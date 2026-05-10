use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

use crate::color::Color;
use crate::telemetry::TelemetryPacket;

pub static SENSOR_CHANNEL: Channel<CriticalSectionRawMutex, SensorMessage, 4> = Channel::new();
pub static TELEMETRY_CHANNEL: Channel<CriticalSectionRawMutex, TelemetryPacket, 4> = Channel::new();

#[derive(Debug, Clone, Copy)]
pub enum SensorMessage {
    Color(Color),
    Distance(u16),
    Voltage(u16),
}
