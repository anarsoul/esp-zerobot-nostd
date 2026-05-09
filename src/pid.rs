// Fixed-point PID. Gains are scaled by SCALE (256), so kp=128 means Kp=0.5.
// Output is a signed integer in the same units as the setpoint/measurement
// (e.g. duty-percent adjustment points).
const SCALE: i32 = 256;

pub struct Pid {
    kp: i32,
    ki: i32,
    kd: i32,
    integral: i32,
    integral_limit: i32,
    prev_error: i32,
}

impl Pid {
    pub const fn new(kp: i32, ki: i32, kd: i32, integral_limit: i32) -> Self {
        Self {
            kp,
            ki,
            kd,
            integral: 0,
            integral_limit,
            prev_error: 0,
        }
    }

    pub fn update(&mut self, setpoint: i32, measurement: i32) -> i32 {
        let error = setpoint - measurement;
        self.integral = (self.integral + error).clamp(-self.integral_limit, self.integral_limit);
        let derivative = error - self.prev_error;
        self.prev_error = error;
        (self.kp * error + self.ki * self.integral + self.kd * derivative) / SCALE
    }

    pub fn reset(&mut self) {
        self.integral = 0;
        self.prev_error = 0;
    }
}
