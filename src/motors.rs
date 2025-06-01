use esp_hal::ledc::{self, channel::ChannelIFace};

const ACCEL_TIME: u16 = 500; // ms
const DECEL_TIME_L: u16 = 1000;
const DECEL_TIME_R: u16 = 500;

pub struct Config {
    accel_time: u16,
    decel_time_l: u16,
    decel_time_r: u16,
    left_duty: u8,
    right_duty: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            accel_time: ACCEL_TIME,
            decel_time_l: DECEL_TIME_L,
            decel_time_r: DECEL_TIME_R,
            left_duty: 87,
            right_duty: 81,
        }
    }
}

pub struct Motors<'a> {
    left_1: ledc::channel::Channel<'a, ledc::LowSpeed>,
    left_2: ledc::channel::Channel<'a, ledc::LowSpeed>,
    right_1: ledc::channel::Channel<'a, ledc::LowSpeed>,
    right_2: ledc::channel::Channel<'a, ledc::LowSpeed>,
    config: Config,
    l1: u8,
    l2: u8,
    r1: u8,
    r2: u8,
}

impl<'a> Motors<'a> {
    pub fn init(
        left_1: ledc::channel::Channel<'a, ledc::LowSpeed>,
        left_2: ledc::channel::Channel<'a, ledc::LowSpeed>,
        right_1: ledc::channel::Channel<'a, ledc::LowSpeed>,
        right_2: ledc::channel::Channel<'a, ledc::LowSpeed>,
        config: Config,
    ) -> Self {
        left_1.set_duty(0).unwrap();
        left_2.set_duty(0).unwrap();
        right_1.set_duty(0).unwrap();
        right_2.set_duty(0).unwrap();
        Self {
            left_1,
            left_2,
            right_1,
            right_2,
            config,
            l1: 0,
            l2: 0,
            r1: 0,
            r2: 0,
        }
    }

    pub fn forward(&mut self) -> u16 {
        self.left_1
            .start_duty_fade(0, self.config.left_duty, self.config.accel_time)
            .unwrap();
        self.left_2.set_duty(0).unwrap();
        self.right_1
            .start_duty_fade(0, self.config.right_duty, self.config.accel_time)
            .unwrap();
        self.right_2.set_duty(0).unwrap();

        self.l1 = self.config.left_duty;
        self.l2 = 0;
        self.r1 = self.config.right_duty;
        self.r2 = 0;

        self.config.accel_time
    }

    pub fn backwards(&mut self) -> u16 {
        self.left_2
            .start_duty_fade(0, self.config.left_duty, self.config.accel_time)
            .unwrap();
        self.left_1.set_duty(0).unwrap();
        self.right_2
            .start_duty_fade(0, self.config.right_duty, self.config.accel_time)
            .unwrap();
        self.right_1.set_duty(0).unwrap();

        self.l2 = self.config.left_duty;
        self.l1 = 0;
        self.r2 = self.config.right_duty;
        self.r1 = 0;

        self.config.accel_time
    }

    pub fn right(&mut self) -> u16 {
        self.left_1
            .start_duty_fade(0, self.config.left_duty, self.config.accel_time)
            .unwrap();
        self.left_2.set_duty(0).unwrap();
        self.right_2
            .start_duty_fade(0, self.config.right_duty, self.config.accel_time)
            .unwrap();
        self.right_1.set_duty(0).unwrap();

        self.l1 = self.config.left_duty;
        self.l2 = 0;
        self.r2 = self.config.right_duty;
        self.r1 = 0;

        self.config.accel_time
    }

    pub fn left(&mut self) -> u16 {
        self.left_2
            .start_duty_fade(0, self.config.left_duty, self.config.accel_time)
            .unwrap();
        self.left_1.set_duty(0).unwrap();
        self.right_1
            .start_duty_fade(0, self.config.right_duty, self.config.accel_time)
            .unwrap();
        self.right_2.set_duty(0).unwrap();

        self.l2 = self.config.left_duty;
        self.l1 = 0;
        self.r1 = self.config.right_duty;
        self.r2 = 0;

        self.config.accel_time
    }

    pub fn stop(&mut self) -> u16 {
        if self.l1 > 0 {
            self.left_1
                .start_duty_fade(self.l1, 0, self.config.decel_time_l)
                .unwrap();
        }
        if self.l2 > 0 {
            self.left_2
                .start_duty_fade(self.l2, 0, self.config.decel_time_l)
                .unwrap();
        }
        if self.r1 > 0 {
            self.right_1
                .start_duty_fade(self.r1, 0, self.config.decel_time_r)
                .unwrap();
        }
        if self.r2 > 0 {
            self.right_2
                .start_duty_fade(self.r2, 0, self.config.decel_time_r)
                .unwrap();
        }

        core::cmp::max(self.config.decel_time_l, self.config.decel_time_r)
    }

    // No decelleration
    pub fn emergency_stop(&mut self) -> u16 {
        self.left_1.set_duty(0).unwrap();
        self.left_2.set_duty(0).unwrap();
        self.right_1.set_duty(0).unwrap();
        self.right_2.set_duty(0).unwrap();

        0
    }
}

#[derive(Debug, Copy, Clone)]
pub enum MotorsSmCommand {
    Forward(u64),
    #[allow(dead_code)]
    Backwards(u64),
    #[allow(dead_code)]
    Stop,
    EmergencyStop,
    Left(u64),
    Right(u64),
}

#[derive(Debug, Copy, Clone)]
enum MotorSmState {
    Stopped,
    WaitAccel,
    WaitDecel,
    Forward,
    Backwards,
    Left,
    Right,
}

#[derive(Debug, Copy, Clone)]
pub enum MotorsSmError {
    Busy,
}

pub struct MotorsSm<'a> {
    current_cmd: Option<MotorsSmCommand>,
    state: MotorSmState,
    motors: Motors<'a>,
}

impl<'a> MotorsSm<'a> {
    pub fn init(motors: Motors<'a>) -> Self {
        Self {
            current_cmd: None,
            state: MotorSmState::Stopped,
            motors,
        }
    }

    pub fn next(&mut self) -> u64 {
        log::debug!("from: {:?}", self.state);
        let res = match self.state {
            MotorSmState::Stopped => {
                if let Some(cmd) = self.current_cmd {
                    match cmd {
                        MotorsSmCommand::Forward(_) => {
                            self.state = MotorSmState::WaitAccel;
                            self.motors.forward() as u64
                        }
                        MotorsSmCommand::Backwards(_) => {
                            self.state = MotorSmState::WaitAccel;
                            self.motors.backwards() as u64
                        }
                        MotorsSmCommand::Left(_) => {
                            self.state = MotorSmState::WaitAccel;
                            self.motors.left() as u64
                        }
                        MotorsSmCommand::Right(_) => {
                            self.state = MotorSmState::WaitAccel;
                            self.motors.right() as u64
                        }
                        MotorsSmCommand::EmergencyStop => {
                            self.state = MotorSmState::Stopped;
                            self.motors.emergency_stop();
                            self.current_cmd = None;
                            0
                        }
                        MotorsSmCommand::Stop => {
                            self.state = MotorSmState::Stopped;
                            self.current_cmd = None;
                            0
                        }
                    }
                } else {
                    // Stopped and no command. Do nothing
                    0
                }
            }
            MotorSmState::WaitAccel => {
                if let Some(cmd) = self.current_cmd {
                    match cmd {
                        MotorsSmCommand::Forward(delay) => {
                            self.state = MotorSmState::Forward;
                            delay
                        }
                        MotorsSmCommand::Backwards(delay) => {
                            self.state = MotorSmState::Backwards;
                            delay
                        }
                        MotorsSmCommand::Left(left) => {
                            self.state = MotorSmState::Left;
                            left
                        }
                        MotorsSmCommand::Right(right) => {
                            self.state = MotorSmState::Right;
                            right
                        }
                        MotorsSmCommand::EmergencyStop => {
                            self.state = MotorSmState::Stopped;
                            self.motors.emergency_stop();
                            self.current_cmd = None;
                            0
                        }
                        _ => {
                            log::info!(
                                "{:?} state with {:?} command. Stopping motors",
                                self.state,
                                cmd
                            );
                            self.state = MotorSmState::WaitDecel;
                            self.motors.stop() as u64
                        }
                    }
                } else {
                    log::info!("{:?} state with no command. Stopping motors", self.state);
                    self.state = MotorSmState::WaitDecel;
                    self.motors.stop() as u64
                }
            }
            MotorSmState::Forward
            | MotorSmState::Backwards
            | MotorSmState::Left
            | MotorSmState::Right => {
                if self
                    .current_cmd
                    .is_some_and(|c| matches!(c, MotorsSmCommand::EmergencyStop))
                {
                    self.state = MotorSmState::Stopped;
                    self.motors.emergency_stop();
                    self.current_cmd = None;
                    0
                } else {
                    self.state = MotorSmState::WaitDecel;
                    self.motors.stop() as u64
                }
            }
            MotorSmState::WaitDecel => {
                self.state = MotorSmState::Stopped;
                self.current_cmd = None;
                0
            }
        };
        log::debug!("to: {:?}, delay: {}", self.state, res);
        res
    }

    pub fn process_cmd(&mut self, new_cmd: MotorsSmCommand) -> Result<(), MotorsSmError> {
        if let MotorsSmCommand::EmergencyStop = new_cmd {
            self.current_cmd = Some(new_cmd);
            Ok(())
        } else {
            match self.state {
                MotorSmState::Stopped => {
                    if self.current_cmd.is_some() {
                        // A command is already accepted, but hasn't been processed yet, so SM is busy
                        Err(MotorsSmError::Busy)
                    } else {
                        self.current_cmd = Some(new_cmd);
                        Ok(())
                    }
                }
                MotorSmState::Forward
                | MotorSmState::Backwards
                | MotorSmState::Left
                | MotorSmState::Right => {
                    if let MotorsSmCommand::Stop = new_cmd {
                        // Accept command. Next state is WaitDecel
                        Ok(())
                    } else {
                        Err(MotorsSmError::Busy)
                    }
                }
                MotorSmState::WaitAccel | MotorSmState::WaitDecel => {
                    // Busy. Retry later
                    Err(MotorsSmError::Busy)
                }
            }
        }
    }
}
