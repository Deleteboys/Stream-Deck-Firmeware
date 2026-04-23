
use crate::protocol::PicoToHost;
use crate::usb::USB_TX_CHANNEL;
use embassy_rp::gpio::Input;
use embassy_time::{Duration, Instant, Timer};

const BUTTON_COUNT: usize = 9;
const POLL_INTERVAL: Duration = Duration::from_millis(2);
const DEBOUNCE_TIME: Duration = Duration::from_millis(20);
const BUTTON_IDS: [u8; BUTTON_COUNT] = [0, 1, 2, 3, 4, 5, 6, 7, 8];

pub type ButtonBank = [Input<'static>; BUTTON_COUNT];

#[embassy_executor::task]
pub async fn button_task(mut buttons: ButtonBank) {
    let mut stable_pressed = [false; BUTTON_COUNT];
    let mut last_sample = [false; BUTTON_COUNT];
    let now = Instant::now();
    let mut changed_at = [now; BUTTON_COUNT];

    loop {
        let now = Instant::now();

        for (index, button) in buttons.iter_mut().enumerate() {
            // Button is wired between GPIO and GND -> active low with pull-up.
            let pressed = button.is_low();

            if pressed != last_sample[index] {
                last_sample[index] = pressed;
                changed_at[index] = now;
            }

            if pressed != stable_pressed[index] && now.duration_since(changed_at[index]) >= DEBOUNCE_TIME {
                stable_pressed[index] = pressed;
                let _ = USB_TX_CHANNEL.try_send(PicoToHost::ButtonChanged {
                    id: BUTTON_IDS[index],
                    pressed,
                });
            }
        }

        Timer::after(POLL_INTERVAL).await;
    }
}
