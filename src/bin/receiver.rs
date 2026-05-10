#![no_std]
#![no_main]

use embassy_executor::Spawner;
use esp_backtrace as _;
use esp_hal::{interrupt::software::SoftwareInterruptControl, timer::timg::TimerGroup};
use esp_radio::esp_now::EspNowReceiver;

use esp_alloc as _;

use esp_zerobot_nostd::telemetry;

esp_bootloader_esp_idf::esp_app_desc!();

#[embassy_executor::task]
async fn recv_task(mut receiver: EspNowReceiver<'static>) {
    loop {
        let r = receiver.receive_async().await;
        match telemetry::unpack(r.data()) {
            Some(pkt) => {
                log::info!(
                    "battery={}mV left_duty={} right_duty={} left_pulses={} right_pulses={}",
                    pkt.battery_mv,
                    pkt.left_duty,
                    pkt.right_duty,
                    pkt.left_pulses,
                    pkt.right_pulses,
                );
            }
            None => {
                log::warn!("Unknown packet ({} bytes)", r.data().len());
            }
        }
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    esp_alloc::heap_allocator!(size: 72 * 1024);

    let peripherals = esp_hal::init(esp_hal::Config::default());
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    let (_wifi_ctrl, interfaces) =
        esp_radio::wifi::new(peripherals.WIFI, Default::default()).unwrap();
    let esp_now = interfaces.esp_now;
    let (_manager, _sender, receiver) = esp_now.split();

    spawner.spawn(recv_task(receiver).unwrap());

    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(3600)).await;
    }
}
