use crate::inputs::debouncer::Debouncer;
use crate::protocol::PicoToHost;
use crate::usb::USB_TX_CHANNEL;
use crate::vibration::VibrationPattern::Medium;
use crate::vibration::VIBRATION_TRIGGER_CHANNEL;
use embassy_rp::gpio::Input;
use embassy_time::{Duration, Timer};
use crate::keyboard::KeyboardMapper;

const ENCODER_COUNT: usize = 4;
const ENCODER_BUTTON_COUNT: usize = 4;
const POLL_INTERVAL: Duration = Duration::from_millis(1);
const STEPS_PER_DETENT: i8 = 4;
const BUTTON_DEBOUNCE_TIME: Duration = Duration::from_millis(5);
pub type EncoderBank = [(Input<'static>, Input<'static>); ENCODER_COUNT];
pub type EncoderButtonBank = [Input<'static>; ENCODER_BUTTON_COUNT];

// Valid quadrature transitions. Index: prev_state << 2 | state
const QUADRATURE_DELTA: [i8; 16] = [0, 1, -1, 0, -1, 0, 0, 1, 1, 0, 0, -1, 0, -1, 1, 0];

#[embassy_executor::task]
pub async fn encoder_task(mut encoders: EncoderBank, mut encoder_buttons: EncoderButtonBank) {
    let mut prev_state = [0u8; ENCODER_COUNT];
    let mut accum = [0i8; ENCODER_COUNT];
    let mut debouncers = [(); 4].map(|_| Debouncer::new(BUTTON_DEBOUNCE_TIME));

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
                    if KeyboardMapper::is_active() {
                        KeyboardMapper::send_encoder_turn(id as u8, 1);
                    } else {
                        let _ = USB_TX_CHANNEL.try_send(PicoToHost::EncoderTurned { id: id as u8, delta: -1 });
                    }
                    accum[id] -= STEPS_PER_DETENT;
                }

                while accum[id] <= -STEPS_PER_DETENT {
                    if KeyboardMapper::is_active() {
                        KeyboardMapper::send_encoder_turn(id as u8, -1);
                    } else {
                        let _ = USB_TX_CHANNEL.try_send(PicoToHost::EncoderTurned { id: id as u8, delta: 1 });
                    }
                    accum[id] += STEPS_PER_DETENT;
                }
            }
        }

        for (i, button) in encoder_buttons.iter_mut().enumerate() {
            if let Some(pressed) = debouncers[i].update(button.is_low()) {
                if KeyboardMapper::is_active() {
                    if pressed {
                        KeyboardMapper::send_encoder_push(i as u8);
                    }
                } else {
                    let _ = USB_TX_CHANNEL.try_send(PicoToHost::EncoderChanged {
                        id: i as u8,
                        pressed,
                    });
                }
                if pressed {
                    let _ = VIBRATION_TRIGGER_CHANNEL.try_send(Medium);
                }
            }
        }
        Timer::after(POLL_INTERVAL).await;
    }
}
