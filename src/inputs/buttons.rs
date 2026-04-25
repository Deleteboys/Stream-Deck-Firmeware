
use crate::protocol::PicoToHost;
use crate::usb::USB_TX_CHANNEL;
use crate::vibration::VIBRATION_TRIGGER_CHANNEL;
use embassy_rp::gpio::Input;
use embassy_time::{Duration,Timer};
use crate::inputs::debouncer::Debouncer;
use crate::vibration::VibrationPattern::Short;

const BUTTON_COUNT: usize = 8;
const POLL_INTERVAL: Duration = Duration::from_millis(2);
const DEBOUNCE_TIME: Duration = Duration::from_millis(5);
const BUTTON_IDS: [u8; BUTTON_COUNT] = [0, 1, 2, 3, 4, 5, 6, 7];

pub type ButtonBank = [Input<'static>; BUTTON_COUNT];

#[embassy_executor::task]
pub async fn button_task(mut buttons: ButtonBank) {
    let mut debouncers = [(); BUTTON_COUNT].map(|_| Debouncer::new(DEBOUNCE_TIME));

    loop {
        for (i, button) in buttons.iter_mut().enumerate() {
            if let Some(pressed) = debouncers[i].update(button.is_low()) {
                let _ = USB_TX_CHANNEL.try_send(PicoToHost::ButtonChanged {
                    id: i as u8,
                    pressed,
                });
                if pressed {
                    let _ = VIBRATION_TRIGGER_CHANNEL.try_send(Short);
                }
            }
        }
        Timer::after(POLL_INTERVAL).await;
    }
}