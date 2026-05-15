# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Bare-metal ESP32-C3 firmware for a tiny autonomous robot car. The robot follows color mats (Purple=forward, Blue=right, Orange/Red=left) using a TCS3472 color sensor and avoids obstacles with an HC-SR04 ultrasonic sensor. Telemetry is broadcast over ESP-NOW.

Hardware: ESP32-C3-Zero, LM298N motor drivers, TCS3472 I2C color sensor, HC-SR04 ultrasonic sensor, WS2812 RGB LED.

## Commands

```bash
# Build
cargo build --release

# Flash and monitor (requires espflash: cargo install espflash)
cargo run --release

# Build/run the telemetry receiver binary
cargo run --release --bin receiver

# Lint (CI treats warnings as errors)
cargo clippy --all-features -- -D warnings

# Format
cargo fmt --all

# Build without PID feature
cargo build --release --no-default-features
```

## Architecture

The project is a `no_std` crate targeting `riscv32imc-unknown-none-elf` (ESP32-C3). There are two binaries:

- **`src/bin/zerobot.rs`** — Main robot firmware. Initializes all peripherals (GPIO, I2C, SPI, LEDC PWM, ADC), spawns async Embassy tasks, and runs the main event loop that processes sensor messages and drives the motor state machine.
- **`src/bin/receiver.rs`** — Standalone telemetry monitor that receives ESP-NOW packets and logs robot state.

Core modules in `src/`:

- **`control.rs`** — Finite state machine: maps `SensorReading` events (color, distance) to `MotorCommand` outputs.
- **`motors.rs`** — Motor control state machine with acceleration/deceleration ramps and optional PID-based speed synchronization between left/right motors.
- **`pid.rs`** — Fixed-point PID controller used by `motors.rs` (enabled via default feature `pid`).
- **`encoder.rs`** — GPIO interrupt handler counting motor encoder pulses. Note: Motor1 hardware generates 2× pulses vs Motor2, compensated in code.
- **`color.rs`** — TCS3472 driver wrapper with color classification logic.
- **`distance.rs`** — Async HC-SR04 ultrasonic sensor task.
- **`comm.rs`** — Embassy `Channel` definitions shared between tasks (`SENSOR_CHANNEL`, `TELEMETRY_CHANNEL`).
- **`telemetry.rs`** — Packet format serialized over ESP-NOW.

### Task / Concurrency Model

Uses `esp-rtos` + Embassy executor. Tasks communicate via `embassy-sync` channels defined in `comm.rs`. The main loop in `zerobot.rs` receives `SensorReading` messages and dispatches `MotorCommand` to `motors.rs`.

### Key Constraints

- `no_std`, `no_main` — entry point via `esp_rtos::entry` macro.
- Heap: 72 KB via `esp-alloc`.
- Profile: `opt-level = "s"` (size) in both dev and release; release adds LTO fat.
- Toolchain: stable Rust with `rust-src` component (needed for `-Z build-std`). Defined in `rust-toolchain.toml`.
