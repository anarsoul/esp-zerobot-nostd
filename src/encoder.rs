use core::cell::RefCell;

use critical_section::Mutex;
use esp_hal::gpio::{AnyPin, Event, Input, InputConfig, Io, Pull};
use esp_hal::peripherals;
use portable_atomic::{AtomicU32, Ordering};

pub static MOTOR1_PULSES: AtomicU32 = AtomicU32::new(0);
pub static MOTOR2_PULSES: AtomicU32 = AtomicU32::new(0);

static ENC_PINS: Mutex<RefCell<Option<(Input<'static>, Input<'static>)>>> =
    Mutex::new(RefCell::new(None));

#[esp_hal::handler]
fn gpio_interrupt() {
    critical_section::with(|cs| {
        let mut pins = ENC_PINS.borrow(cs).borrow_mut();
        if let Some((enc1, enc2)) = pins.as_mut() {
            if enc1.is_interrupt_set() {
                MOTOR1_PULSES.fetch_add(1, Ordering::Relaxed);
                enc1.clear_interrupt();
            }
            // enc1 generates 2x of the pulses. It is a hardware bug
            if enc2.is_interrupt_set() {
                MOTOR2_PULSES.fetch_add(2, Ordering::Relaxed);
                enc2.clear_interrupt();
            }
        }
    });
}

pub fn init(
    io_mux: peripherals::IO_MUX<'static>,
    mot1_enc: AnyPin<'static>,
    mot2_enc: AnyPin<'static>,
) {
    let mut io = Io::new(io_mux);
    io.set_interrupt_handler(gpio_interrupt);

    let mut enc1 = Input::new(mot1_enc, InputConfig::default().with_pull(Pull::Up));
    let mut enc2 = Input::new(mot2_enc, InputConfig::default().with_pull(Pull::Up));

    // Listen and store atomically: handler can never fire and find None.
    critical_section::with(|cs| {
        enc1.listen(Event::FallingEdge);
        enc2.listen(Event::FallingEdge);
        ENC_PINS.borrow(cs).replace(Some((enc1, enc2)));
    });
}
