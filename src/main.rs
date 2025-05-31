#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Instant, Timer};
use esp_backtrace as _;
use esp_hal::{
    analog::adc,
    gpio::Pin,
    i2c,
    ledc::{self, channel::ChannelIFace, timer::TimerIFace},
    peripherals, spi,
    time::Rate,
    timer::timg::TimerGroup,
};

use smart_leds::{SmartLedsWrite, RGB};
use ws2812_spi::Ws2812;

use embassy_futures::select::{select, Either};

mod color;
mod comm;
mod distance;
mod motors;

use color::{color_task, Color};
use comm::{SensorMessage, SENSOR_CHANNEL};
use distance::distance_task;
use motors::{Motors, MotorsSm, MotorsSmCommand};

use esp_alloc as _;

const FORWARD_DELAY: u64 = 700;
const LEFT_DELAY: u64 = 200;
const RIGHT_DELAY: u64 = 200;
const _BACKWARDS_DELAY: u64 = 1000;
const BATTERY_LOW: u16 = 3200; // 3200 mV
const NO_BATTERY: u16 = 200; // 200 mV
const DISTANCE_CLOSE: u16 = 10; // cm

#[embassy_executor::task]
async fn battery_task(adc: peripherals::ADC1, pin: esp_hal::gpio::GpioPin<4>) {
    log::info!("Starting battery task");
    let mut adc1_config = adc::AdcConfig::new();
    let mut adc_pin = adc1_config.enable_pin_with_cal::<_, adc::AdcCalCurve<peripherals::ADC1>>(pin, adc::Attenuation::_11dB);
    let mut adc = adc::Adc::new(adc, adc1_config).into_async();

    loop {
        let v = 2 * adc.read_oneshot(&mut adc_pin).await;

        SENSOR_CHANNEL.send(SensorMessage::Voltage(v)).await;
        log::info!("Battery voltage: {}", v);
        Timer::after(Duration::from_millis(200)).await;
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();
    esp_alloc::heap_allocator!(size: 72 * 1024);

    #[allow(unused)]
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    let mosi = peripherals.GPIO10;
    let scl = peripherals.GPIO8;
    let sda = peripherals.GPIO9;

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
    .with_sda(sda)
    .with_scl(scl)
    .into_async();

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
            pin_config: ledc::channel::config::PinConfig::PushPull,
        })
        .unwrap();

    let mut mot1_2 = ledc.channel(ledc::channel::Number::Channel1, peripherals.GPIO2);
    mot1_2
        .configure(ledc::channel::config::Config {
            timer: &lstimer0,
            duty_pct: 0,
            pin_config: ledc::channel::config::PinConfig::PushPull,
        })
        .unwrap();
    let mut mot2_1 = ledc.channel(ledc::channel::Number::Channel2, peripherals.GPIO0);
    mot2_1
        .configure(ledc::channel::config::Config {
            timer: &lstimer1,
            duty_pct: 0,
            pin_config: ledc::channel::config::PinConfig::PushPull,
        })
        .unwrap();
    let mut mot2_2 = ledc.channel(ledc::channel::Number::Channel3, peripherals.GPIO1);
    mot2_2
        .configure(ledc::channel::config::Config {
            timer: &lstimer1,
            duty_pct: 0,
            pin_config: ledc::channel::config::PinConfig::PushPull,
        })
        .unwrap();

    let trigger = peripherals.GPIO7.degrade();
    let echo = peripherals.GPIO6.degrade();

    let motors = Motors::init(mot1_1, mot1_2, mot2_1, mot2_2, motors::Config::default());
    let mut motors_sm = MotorsSm::init(motors);

    let battery_pin = peripherals.GPIO4;
    let adc = peripherals.ADC1;

    spawner.spawn(battery_task(adc, battery_pin)).unwrap();
    spawner.spawn(color_task(i2c)).unwrap();
    spawner.spawn(distance_task(trigger, echo)).unwrap();

    led.write([RGB::new(0, 0, 0)]).unwrap();
    log::info!("Starting main loop");

    let mut wait = 0;
    let mut now: Option<Instant> = None;
    let mut last_turn = false;
    let mut distance_close_cnt = 0;
    loop {
        let timer_delay = if wait > 0 {
            let elapsed = now.unwrap().elapsed().as_millis();
            if elapsed > wait {
                10
            } else {
                wait - elapsed
            }
        } else {
            100
        };
        let res = select(
            Timer::after(Duration::from_millis(timer_delay)),
            SENSOR_CHANNEL.receive(),
        )
        .await;

        if let Either::Second(msg) = res {
            match msg {
                SensorMessage::Color(color) => {
                    led.write([color.to_rgb()]).unwrap();
                    if distance_close_cnt < 3 { match color {
                        Color::Magenta => {
                            if motors_sm
                                .process_cmd(MotorsSmCommand::Forward(FORWARD_DELAY))
                                .is_ok()
                            {
                                last_turn = false;
                            }
                        }
                        Color::Green => {
                            if motors_sm.process_cmd(MotorsSmCommand::Stop).is_ok() {
                                last_turn = false;
                            }
                        }
                        Color::Red | Color::Orange => {
                            if !last_turn {
                                if motors_sm
                                    .process_cmd(MotorsSmCommand::Left(LEFT_DELAY))
                                    .is_ok()
                                {
                                    last_turn = true;
                                }
                            } else if motors_sm
                                .process_cmd(MotorsSmCommand::Forward(FORWARD_DELAY))
                                .is_ok()
                            {
                                last_turn = false;
                            }
                        }
                        Color::Blue => {
                            if !last_turn {
                                if motors_sm
                                    .process_cmd(MotorsSmCommand::Right(RIGHT_DELAY))
                                    .is_ok()
                                {
                                    last_turn = true;
                                }
                            } else if motors_sm
                                .process_cmd(MotorsSmCommand::Forward(FORWARD_DELAY))
                                .is_ok()
                            {
                                last_turn = false;
                            }
                        }
                        _ => {}
                    }}
                }
                SensorMessage::Distance(distance) => {
                    // Do emergency stop if distance is < 10cm
                    if distance <= DISTANCE_CLOSE {
                        if distance_close_cnt == 3 {
                            log::info!("Emergency stop!");
                            motors_sm.process_cmd(MotorsSmCommand::EmergencyStop).unwrap();
                        } else {
                            distance_close_cnt += 1;
                        }
                    } else {
                        distance_close_cnt = 0;
                    }
                }
                SensorMessage::Voltage(v) => {
                    if v > NO_BATTERY && v < BATTERY_LOW {
                        // Break the loop if voltage is lower than 3.2v
                        log::info!("Battery low: {} mV", v * 2);
                        motors_sm.process_cmd(MotorsSmCommand::EmergencyStop).unwrap();
                        break;
                    }
                }
            }
        }

        if wait == 0 || now.is_some_and(|now| now.elapsed().as_millis() >= wait) {
            wait = motors_sm.next();
            if wait > 0 {
                now = Some(Instant::now());
            } else {
                now = None;
            }
        }
    }

    loop {
        led.write([RGB::new(64, 0, 0)]).unwrap();
        Timer::after(Duration::from_millis(500)).await;
        led.write([RGB::new(0, 0, 0)]).unwrap();
        Timer::after(Duration::from_millis(500)).await;
    }
}
