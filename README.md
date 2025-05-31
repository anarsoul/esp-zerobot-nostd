Firmware for my variation of [Zerobot]("https://hackaday.io/project/25092-zerobot-raspberry-pi-zero-fpv-robot/log/97988-the-new-zerobot-pro")

Strictly speaking, the only part from Zerobot is enclosure with some minor modifications.

Software and hardware are mine, although the hardware is trivial: esp-c3-zero driving 2 DC motors via LM298N-like drivers,
TCS3472 color sensor and HC-SR04 ultrasonic sensor. It doesn't have a camera and doesn't support remote control (yet)

The firmware is written in Rust using esp-hal and embassy. It might look a bit overengineered, but hey - it runs on bare metal!
