use crate::color::Color;
use crate::comm::SensorMessage;
use crate::motors::MotorsSmCommand;

enum ControlState {
    BatteryLow,
    Blocked,
    Normal,
}

pub struct ControlSm {
    state: ControlState,
    distance_samples_cnt: u32,
    last_turn: bool,
}

const BATTERY_LOW: u16 = 3200; // 3200 mV
const NO_BATTERY: u16 = 200; // 200 mV
const DISTANCE_CLOSE: u16 = 7; // cm
const DISTANCE_SAMPLES: u32 = 3;

const FORWARD_DELAY: u64 = 600;
const LEFT_DELAY: u64 = 180;
const RIGHT_DELAY: u64 = 160;
const _BACKWARDS_DELAY: u64 = 1000;

impl ControlSm {
    pub fn init() -> Self {
        Self {
            state: ControlState::Blocked,
            distance_samples_cnt: 0,
            last_turn: false,
        }
    }

    pub fn process_event(&mut self, message: SensorMessage) -> Option<MotorsSmCommand> {
        match self.state {
            ControlState::BatteryLow => match message {
                SensorMessage::Voltage(v) => {
                    if !(NO_BATTERY..=BATTERY_LOW).contains(&v) {
                        self.state = ControlState::Blocked;
                        self.distance_samples_cnt = 0;
                    }
                    None
                }
                _ => None,
            },
            ControlState::Normal => match message {
                SensorMessage::Voltage(v) => {
                    if (NO_BATTERY..BATTERY_LOW).contains(&v) {
                        self.state = ControlState::BatteryLow;
                        self.distance_samples_cnt = 0;
                        Some(MotorsSmCommand::EmergencyStop)
                    } else {
                        None
                    }
                }
                SensorMessage::Distance(d) => {
                    if d < DISTANCE_CLOSE {
                        self.distance_samples_cnt =
                            (self.distance_samples_cnt + 1).clamp(0, DISTANCE_SAMPLES);
                    } else {
                        self.distance_samples_cnt = 0;
                    }

                    if self.distance_samples_cnt == DISTANCE_SAMPLES {
                        self.distance_samples_cnt = 0;
                        self.state = ControlState::Blocked;
                        Some(MotorsSmCommand::EmergencyStop)
                    } else {
                        None
                    }
                }
                SensorMessage::Color(c) => match c {
                    Color::Magenta => {
                        self.last_turn = false;
                        Some(MotorsSmCommand::Forward(FORWARD_DELAY))
                    }
                    Color::Red | Color::Orange => {
                        if self.last_turn {
                            self.last_turn = false;
                            Some(MotorsSmCommand::Forward(FORWARD_DELAY))
                        } else {
                            self.last_turn = true;
                            Some(MotorsSmCommand::Left(LEFT_DELAY))
                        }
                    }
                    Color::Blue => {
                        if self.last_turn {
                            self.last_turn = false;
                            Some(MotorsSmCommand::Forward(FORWARD_DELAY))
                        } else {
                            self.last_turn = true;
                            Some(MotorsSmCommand::Right(RIGHT_DELAY))
                        }
                    }
                    _ => None,
                },
            },
            ControlState::Blocked => match message {
                SensorMessage::Voltage(v) => {
                    if (NO_BATTERY..BATTERY_LOW).contains(&v) {
                        self.state = ControlState::BatteryLow;
                        self.distance_samples_cnt = 0;
                        Some(MotorsSmCommand::EmergencyStop)
                    } else {
                        None
                    }
                }
                SensorMessage::Distance(d) => {
                    if d > DISTANCE_CLOSE {
                        self.distance_samples_cnt =
                            (self.distance_samples_cnt + 1).clamp(0, DISTANCE_SAMPLES);
                    } else {
                        self.distance_samples_cnt = 0;
                    }

                    if self.distance_samples_cnt == DISTANCE_SAMPLES {
                        self.distance_samples_cnt = 0;
                        self.last_turn = false;
                        self.state = ControlState::Normal;
                    }
                    None
                }
                _ => None,
            },
        }
    }
}
