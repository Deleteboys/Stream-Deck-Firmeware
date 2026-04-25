use crate::protocol::PicoToHost;
use crate::usb::USB_TX_CHANNEL;
use crate::vibration::VIBRATION_TRIGGER_CHANNEL;
use embassy_rp::gpio::Input;
use embassy_time::{Duration, Instant, Timer};

const ENCODER_COUNT: usize = 4;
const ENCODER_BUTTON_COUNT: usize = 4;
const POLL_INTERVAL: Duration = Duration::from_millis(1);
const STEPS_PER_DETENT: i8 = 4;
const BUTTON_DEBOUNCE_TIME: Duration = Duration::from_millis(5);
const ENCODER_BUTTON_IDS: [u8; ENCODER_BUTTON_COUNT] = [8, 11, 14, 17];

pub type EncoderBank = [(Input<'static>, Input<'static>); ENCODER_COUNT];
pub type EncoderButtonBank = [Input<'static>; ENCODER_BUTTON_COUNT];

// Valid quadrature transitions. Index: prev_state << 2 | state
const QUADRATURE_DELTA: [i8; 16] = [0, 1, -1, 0, -1, 0, 0, 1, 1, 0, 0, -1, 0, -1, 1, 0];

#[embassy_executor::task]
pub async fn encoder_task(mut encoders: EncoderBank, mut encoder_buttons: EncoderButtonBank) {
    let mut prev_state = [0u8; ENCODER_COUNT];
    let mut accum = [0i8; ENCODER_COUNT];
    let mut stable_pressed = [false; ENCODER_BUTTON_COUNT];
    let mut last_sample = [false; ENCODER_BUTTON_COUNT];
    let now = Instant::now();
    let mut changed_at = [now; ENCODER_BUTTON_COUNT];

    for (id, (a, b)) in encoders.iter_mut().enumerate() {
        let a_bit = a.is_high() as u8;
        let b_bit = b.is_high() as u8;
        prev_state[id] = (a_bit << 1) | b_bit;
    }

    loop {
        for (id, (a, b)) in encoders.iter_mut().enumerate() {
            let a_bit = a.is_high() as u8;
            let b_bit = b.is_high() as u8;
            let state = (a_bit << 1) | b_bit;

            let transition = ((prev_state[id] << 2) | state) as usize;
            let delta = QUADRATURE_DELTA[transition];
            prev_state[id] = state;

            if delta != 0 {
                accum[id] += delta;

                while accum[id] >= STEPS_PER_DETENT {
                    let _ = USB_TX_CHANNEL.try_send(PicoToHost::EncoderTurned {
                        id: id as u8,
                        // Flip direction so clockwise is reported as +1.
                        delta: -1,
                    });
                    accum[id] -= STEPS_PER_DETENT;
                }

                while accum[id] <= -STEPS_PER_DETENT {
                    let _ = USB_TX_CHANNEL.try_send(PicoToHost::EncoderTurned {
                        id: id as u8,
                        delta: 1,
                    });
                    accum[id] += STEPS_PER_DETENT;
                }
            }
        }

        let now = Instant::now();
        for (index, button) in encoder_buttons.iter_mut().enumerate() {
            let pressed = button.is_low();

            if pressed != last_sample[index] {
                last_sample[index] = pressed;
                changed_at[index] = now;
            }

            if pressed != stable_pressed[index]
                && now.duration_since(changed_at[index]) >= BUTTON_DEBOUNCE_TIME
            {
                stable_pressed[index] = pressed;
                let _ = USB_TX_CHANNEL.try_send(PicoToHost::EncoderChanged {
                    id: ENCODER_BUTTON_IDS[index],
                    pressed,
                });
                if pressed {
                    let _ = VIBRATION_TRIGGER_CHANNEL.try_send(());
                }
            }
        }

        Timer::after(POLL_INTERVAL).await;
    }
}
