# Project Description

This is a project for ESP32-C3 written in Rust `no_std`, using `esp-hal`. It uses Embassy as the async runtime.

This is software for a small robot car. It is driven using 2 DC motors with motor drivers connected to
GPIO0, GPIO1 and GPIO2, GPIO3. Motor speed is controlled using PWM with the LEDC peripheral.

The robot has 2 sensors:

- Ultrasonic distance sensor (HCSR04): GPIO7 for trigger, GPIO6 for echo
- Color sensor (TCS3472) on the bottom: connected to I2C, SCL: GPIO9, SDA: GPIO5

The "path" for the robot is built using color pads, and the robot senses the color of the pad to
determine direction:

- Purple: forward
- Red or orange: forward and then left
- Blue: forward and then right

If the robot determines that it is blocked, it issues an emergency stop.

## Project Structure

The software is event-driven, with color and distance tasks issuing events for the "control" state machine.

| File              | Purpose                                      |
|-------------------|----------------------------------------------|
| `src/main.rs`     | Entry point, glue for all components and main loop |
| `src/color.rs`    | Color sensor task                            |
| `src/distance.rs` | Distance task                                |
| `src/control.rs`  | Control state machine                        |
| `src/motors.rs`   | Motors state machine                         |

Sensor tasks send events to `SENSOR_CHANNEL`. The main loop consumes events from `SENSOR_CHANNEL`
and sends them to `control_sm`, which produces a command for `motors_sm`.

# Rust `no_std` Guidelines

## General Rules

- ALWAYS use `#[no_std]` in all lib/main files.
- NEVER use `std::*`. Use `core::*` or `alloc::*` (if alloc is allowed).
- Define `#[panic_handler]` in the final binary crate.
- Prefer `heapless` structures (queues, vectors) over `std` collections.
- Ensure all code is suitable for embedded/bare-metal environments.

## Development Constraints

- **Allocation:** Only use the `alloc` crate if a heap allocator is explicitly initialized.
- **Floating Point:** Avoid `f32`/`f64` if the target is a soft-float architecture.
- **Error Handling:** Use `Result` without `std::error::Error`.

## Commands

- **Build:** `cargo build`
- **Clippy:** `cargo clippy --no-default-features`
- **Check:** `cargo check`
