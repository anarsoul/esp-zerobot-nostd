use embassy_time::{Delay, Duration, Instant, Timer};
use esp_hal::gpio::{AnyPin, Input, InputConfig, Level, Output, OutputConfig};

use crate::comm::{SensorMessage, SENSOR_CHANNEL};

use hcsr04_async::{Config, DistanceUnit, Hcsr04, TemperatureUnit};

#[embassy_executor::task]
pub async fn distance_task(trigger: AnyPin, echo: AnyPin) {
    struct EmbassyClock;
    impl hcsr04_async::Now for EmbassyClock {
        fn now_micros(&self) -> u64 {
            Instant::now().as_micros()
        }
    }

    let config = Config {
        distance_unit: DistanceUnit::Centimeters,
        temperature_unit: TemperatureUnit::Celsius,
    };

    let clock = EmbassyClock;
    let delay = Delay;

    let trigger = Output::new(trigger, Level::Low, OutputConfig::default());
    let echo = Input::new(echo, InputConfig::default());
    let mut sensor = Hcsr04::new(trigger, echo, config, clock, delay);

    let temp = 22.0;
    loop {
        let distance = sensor.measure(temp).await;
        if let Ok(res) = distance {
            log::info!("Distance: {}", res);
            SENSOR_CHANNEL
                .send(SensorMessage::Distance(res as u16))
                .await;
        } else {
            log::error!("Couldn't measure distance");
        }
        Timer::after(Duration::from_millis(100)).await;
    }
}
