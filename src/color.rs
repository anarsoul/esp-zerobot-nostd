use crate::comm::{SensorMessage, SENSOR_CHANNEL};
use embassy_time::{Duration, Timer};
use esp_hal::i2c;
use smart_leds::RGB;
use tcs3472::{AllChannelMeasurement, Tcs3472};

#[derive(Debug, Clone, Copy)]
pub enum Color {
    Black,
    Blue,
    Red,
    Magenta,
    Green,
    Cyan,
    Yellow,
    White,
    Orange,
    Unknown,
}

fn normalize_measurement(m: AllChannelMeasurement) -> [f32; 3] {
    let mut max = m.red;

    if m.green > max {
        max = m.green;
    }

    // Correct blue level
    let blue = m.blue * 3 / 2;
    if blue > max {
        max = blue;
    }

    // attenuates "strong" signals and weakens "weak"
    let reverse_gamma = |x: f32| -> f32 { x * x };

    [
        reverse_gamma(m.red as f32 / max as f32),
        reverse_gamma(m.green as f32 / max as f32),
        reverse_gamma(blue as f32 / max as f32),
    ]
}

enum LowHigh {
    Low,
    High,
    Range(f32, f32),
}

const CLEAR_THRESHOLD: u16 = 110;
const CHANNEL_HIGH_THRESHOLD: f32 = 0.7;
const CHANNEL_LOW_THRESHOLD: f32 = 0.3;

impl Color {
    pub fn to_rgb(self) -> RGB<u8> {
        match self {
            Color::Black => RGB::new(0, 0, 0),
            Color::Blue => RGB::new(0, 0, 128),
            Color::Red => RGB::new(128, 0, 0),
            Color::Magenta => RGB::new(128, 0, 128),
            Color::Green => RGB::new(0, 128, 0),
            Color::Cyan => RGB::new(0, 128, 128),
            Color::Yellow => RGB::new(128, 128, 0),
            Color::White => RGB::new(128, 128, 128),
            Color::Orange => RGB::new(128, 82, 0),
            Color::Unknown => RGB::new(0, 0, 0),
        }
    }

    fn compare(f: [f32; 3], c: [LowHigh; 3]) -> bool {
        for idx in 0..3 {
            match c[idx] {
                LowHigh::Low => {
                    if f[idx] > CHANNEL_LOW_THRESHOLD {
                        return false;
                    }
                }
                LowHigh::High => {
                    if f[idx] < CHANNEL_HIGH_THRESHOLD {
                        return false;
                    }
                }
                LowHigh::Range(min, max) => {
                    if f[idx] < min || f[idx] > max {
                        return false;
                    }
                }
            }
        }
        true
    }

    pub fn from_measurement(m: AllChannelMeasurement) -> Self {
        let f = normalize_measurement(m);

        let colors = [
            (Color::Blue, [LowHigh::Low, LowHigh::Low, LowHigh::High]),
            //(Color::Red, [LowHigh::High, LowHigh::Low, LowHigh::Low]),
            (Color::Magenta, [LowHigh::High, LowHigh::Low, LowHigh::High]),
            (Color::Green, [LowHigh::Low, LowHigh::High, LowHigh::Low]),
            (Color::Cyan, [LowHigh::Low, LowHigh::High, LowHigh::High]),
            (Color::Yellow, [LowHigh::High, LowHigh::High, LowHigh::Low]),
            (Color::White, [LowHigh::High, LowHigh::High, LowHigh::High]),
            // Custom color for my filaments
            (
                Color::Magenta,
                [
                    LowHigh::Range(0.3, 0.5),
                    LowHigh::Range(0.3, 0.5),
                    LowHigh::High,
                ],
            ),
            (
                Color::Orange,
                [
                    LowHigh::High,
                    LowHigh::Range(0.45, 0.55),
                    LowHigh::Range(0.4, 0.5),
                ],
            ),
            (
                Color::Red,
                [
                    LowHigh::High,
                    LowHigh::Range(0.45, 0.55),
                    LowHigh::Range(0.55, 0.7),
                ],
            ),
        ];

        if m.clear < CLEAR_THRESHOLD {
            return Color::Black;
        }

        for (color, channels) in colors {
            if Self::compare(f, channels) {
                return color;
            }
        }

        Color::Unknown
    }
}

#[embassy_executor::task]
pub async fn color_task(i2c: i2c::master::I2c<'static, esp_hal::Async>) {
    let mut sensor = Tcs3472::new(i2c);

    log::info!("Starting color sensor task");
    sensor.enable().await.unwrap();
    sensor.enable_rgbc().await.unwrap();
    sensor.set_integration_cycles(32).await.unwrap();

    loop {
        if sensor.is_rgbc_status_valid().await.unwrap() {
            let m = sensor.read_all_channels().await.unwrap();
            let color = Color::from_measurement(m);
            log::debug!(
                "Measurement: {:?}, {:?}, {:?}",
                m,
                normalize_measurement(m),
                color
            );
            SENSOR_CHANNEL.send(SensorMessage::Color(color)).await;
        } else {
            log::error!("Measurement is not valid!");
        }
        Timer::after(Duration::from_millis(100)).await;
    }
}
