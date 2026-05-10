#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Instant, Timer};
use esp_backtrace as _;
use esp_hal::{
    analog::adc,
    gpio::{DriveMode, Pin},
    i2c,
    ledc::{self, channel::ChannelIFace, timer::TimerIFace},
    peripherals, spi,
    time::Rate,
    timer::timg::TimerGroup,
};
use esp_radio::esp_now::{BROADCAST_ADDRESS, EspNowSender};

use smart_leds::{RGB, SmartLedsWrite};
use ws2812_spi::Ws2812;

use embassy_futures::select::{Either, select};

use esp_zerobot_nostd::color::color_task;
use esp_zerobot_nostd::comm::{SENSOR_CHANNEL, SensorMessage, TELEMETRY_CHANNEL};
use esp_zerobot_nostd::control::ControlSm;
use esp_zerobot_nostd::distance::distance_task;
use esp_zerobot_nostd::motors::{Motors, MotorsSm};
use esp_zerobot_nostd::telemetry;
use esp_zerobot_nostd::{encoder, motors};

use esp_alloc as _;

#[embassy_executor::task]
async fn telemetry_task(mut sender: EspNowSender<'static>) {
    loop {
        let pkt = TELEMETRY_CHANNEL.receive().await;
        let buf = telemetry::pack(&pkt);
        if let Err(e) = sender.send_async(&BROADCAST_ADDRESS, &buf).await {
            log::warn!("ESP-NOW send error: {:?}", e);
        }
    }
}

#[embassy_executor::task]
async fn battery_task(adc: peripherals::ADC1<'static>, pin: peripherals::GPIO4<'static>) {
    log::info!("Starting battery task");
    let mut adc1_config = adc::AdcConfig::new();
    let mut adc_pin = adc1_config.enable_pin_with_cal::<_, adc::AdcCalCurve<peripherals::ADC1>>(
        pin,
        adc::Attenuation::_11dB,
    );
    let mut adc = adc::Adc::new(adc, adc1_config).into_async();

    loop {
        let v = 2 * adc.read_oneshot(&mut adc_pin).await;

        SENSOR_CHANNEL.send(SensorMessage::Voltage(v)).await;
        log::debug!("Battery voltage: {}", v);
        Timer::after(Duration::from_millis(1000)).await;
    }
}

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    esp_alloc::heap_allocator!(size: 72 * 1024);

    #[allow(unused)]
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    // Wait for 1s before proceeding with initialization
    Timer::after(Duration::from_millis(1000)).await;

    let mot1_enc = peripherals.GPIO8.degrade();
    let mot2_enc = peripherals.GPIO20.degrade();

    encoder::init(peripherals.IO_MUX, mot1_enc, mot2_enc);

    let mosi = peripherals.GPIO10;
    let scl = peripherals.GPIO9;
    let sda = peripherals.GPIO5;

    let spi = spi::master::Spi::new(
        peripherals.SPI2,
        spi::master::Config::default()
            .with_frequency(Rate::from_khz(3800))
            .with_mode(spi::Mode::_0),
    )
    .unwrap()
    .with_mosi(mosi)
    .into_async();

    let mut led = Ws2812::new(spi);

    let i2c = i2c::master::I2c::new(
        peripherals.I2C0,
        i2c::master::Config::default().with_frequency(Rate::from_khz(100)),
    )
    .unwrap()
    .into_async()
    .with_sda(sda)
    .with_scl(scl);

    let mut ledc = ledc::Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(ledc::LSGlobalClkSource::APBClk);
    let mut lstimer0 = ledc.timer::<ledc::LowSpeed>(ledc::timer::Number::Timer0);
    lstimer0
        .configure(ledc::timer::config::Config {
            duty: ledc::timer::config::Duty::Duty7Bit,
            clock_source: ledc::timer::LSClockSource::APBClk,
            frequency: Rate::from_khz(24),
        })
        .unwrap();

    let mut lstimer1 = ledc.timer::<ledc::LowSpeed>(ledc::timer::Number::Timer1);
    lstimer1
        .configure(ledc::timer::config::Config {
            duty: ledc::timer::config::Duty::Duty7Bit,
            clock_source: ledc::timer::LSClockSource::APBClk,
            frequency: Rate::from_khz(24),
        })
        .unwrap();

    let mut mot1_1 = ledc.channel(ledc::channel::Number::Channel0, peripherals.GPIO3);
    mot1_1
        .configure(ledc::channel::config::Config {
            timer: &lstimer0,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        })
        .unwrap();

    let mut mot1_2 = ledc.channel(ledc::channel::Number::Channel1, peripherals.GPIO2);
    mot1_2
        .configure(ledc::channel::config::Config {
            timer: &lstimer0,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        })
        .unwrap();
    let mut mot2_1 = ledc.channel(ledc::channel::Number::Channel2, peripherals.GPIO0);
    mot2_1
        .configure(ledc::channel::config::Config {
            timer: &lstimer1,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        })
        .unwrap();
    let mut mot2_2 = ledc.channel(ledc::channel::Number::Channel3, peripherals.GPIO1);
    mot2_2
        .configure(ledc::channel::config::Config {
            timer: &lstimer1,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        })
        .unwrap();

    let trigger = peripherals.GPIO7.degrade();
    let echo = peripherals.GPIO6.degrade();

    let motors = Motors::init(mot1_1, mot1_2, mot2_1, mot2_2, motors::Config::default());
    let mut motors_sm = MotorsSm::init(motors);
    let mut control_sm = ControlSm::init();

    let battery_pin = peripherals.GPIO4;
    let adc = peripherals.ADC1;

    spawner.spawn(battery_task(adc, battery_pin).unwrap());
    spawner.spawn(color_task(i2c).unwrap());
    spawner.spawn(distance_task(trigger, echo).unwrap());

    let (_wifi_ctrl, interfaces) =
        esp_radio::wifi::new(peripherals.WIFI, Default::default()).unwrap();
    let esp_now = interfaces.esp_now;
    let (_manager, sender, _receiver) = esp_now.split();
    spawner.spawn(telemetry_task(sender).unwrap());

    led.write([RGB::new(0, 0, 0)]).unwrap();
    log::info!("Starting main loop");

    let mut wait = 0;
    let mut now: Option<Instant> = None;
    loop {
        let timer_delay = if wait > 0 {
            let elapsed = now.unwrap().elapsed().as_millis();
            if elapsed > wait { 10 } else { wait - elapsed }
        } else {
            100
        };
        let res = select(
            Timer::after(Duration::from_millis(timer_delay)),
            SENSOR_CHANNEL.receive(),
        )
        .await;

        if let Either::Second(msg) = res {
            if let SensorMessage::Voltage(v) = msg {
                let (left_duty, right_duty) = motors_sm.current_duties();
                let (left_pulses, right_pulses) = motors_sm.last_pulse_counts();
                let pkt = telemetry::TelemetryPacket {
                    battery_mv: v,
                    left_duty,
                    right_duty,
                    left_pulses,
                    right_pulses,
                };
                TELEMETRY_CHANNEL.try_send(pkt).ok();
            }

            let mut is_color = false;
            if let SensorMessage::Color(color) = msg {
                led.write([color.to_rgb()]).unwrap();
                is_color = true;
            }

            // Supress color messages when motor_sm is busy
            // Otherwise it may clear "last_turn" flag, but motors_sm will reject
            // the cmd, since it's busy
            //
            // Other events should be passed, since it may result in emergency stop
            if !motors_sm.busy() || !is_color {
                let cmd = control_sm.process_event(msg);
                if let Some(cmd) = cmd {
                    match motors_sm.process_cmd(cmd) {
                        Ok(()) => {}
                        Err(x) => {
                            log::info!("motors_sm.process_cmd returned {:?}", x);
                        }
                    }
                }
            }
        }

        if wait == 0 || now.is_some_and(|now| now.elapsed().as_millis() >= wait) {
            wait = motors_sm.process();
            if wait > 0 {
                now = Some(Instant::now());
            } else {
                now = None;
            }
        }
    }
}
