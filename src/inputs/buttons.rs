
use crate::protocol::PicoToHost;
use crate::usb::USB_TX_CHANNEL;
use crate::vibration::{VibrationPattern, VIBRATION_TRIGGER_CHANNEL};
use embassy_rp::gpio::Input;
use embassy_time::{Duration,Timer};
use crate::inputs::debouncer::Debouncer;
use crate::keyboard::KeyboardMapper;
use crate::vibration::VibrationPattern::Medium;

const BUTTON_COUNT: usize = 8;
const POLL_INTERVAL: Duration = Duration::from_millis(2);
const DEBOUNCE_TIME: Duration = Duration::from_millis(5);
pub type ButtonBank = [Input<'static>; BUTTON_COUNT];

#[embassy_executor::task]
pub async fn button_task(mut buttons: ButtonBank) {
    // Erstelle 8 Debouncer-Instanzen
    let mut debouncers = [(); BUTTON_COUNT].map(|_| Debouncer::new(DEBOUNCE_TIME));

    // Merkt sich den physischen Zustand (gedrückt oder nicht) jeder Taste
    let mut button_states = [false; BUTTON_COUNT];

    // Verhindert, dass die Combo mehrfach auslöst, wenn man die Tasten gedrückt hält
    let mut combo_handled = false;

    loop {
        for (id, button) in buttons.iter_mut().enumerate() {
            // Wir lesen den Pin (is_low() = gedrückt bei Pull-Up) und geben ihn an deinen Debouncer
            if let Some(pressed) = debouncers[id].update(button.is_low()) {
                button_states[id] = pressed;

                // --- NORMALE TASTEN SENDEN ---
                // Sende die Taste NUR, wenn wir nicht gerade die Modus-Wechsel-Combo drücken!
                if !(button_states[0] && button_states[7]) {
                    if KeyboardMapper::is_active() {
                        // Simple Mode: Nur den Tastendruck senden (Loslassen macht der USB HID Task automatisch)
                        if pressed {
                            KeyboardMapper::send_button(id as u8);
                        }
                    } else {
                        // Normaler CDC Modus: An die PC Software senden
                        let _ = USB_TX_CHANNEL.try_send(PicoToHost::ButtonChanged {
                            id: id as u8,
                            pressed,
                        });
                    }
                }
            }
        }

        // --- COMBO: MODUS WECHSEL (Tasten 0 und 7) ---
        if button_states[0] && button_states[7] {
            // Nur 1x umschalten, solange die Tasten gehalten werden
            if !combo_handled {
                let _ = KeyboardMapper::toggle();

                let _ = VIBRATION_TRIGGER_CHANNEL.try_send(VibrationPattern::Long);

                let _ = crate::display::DISPLAY_COMMAND_CHANNEL.try_send(crate::display::DisplayCommand::ForceRedraw);


                combo_handled = true;
            }
        } else {
            combo_handled = false;
        }

        Timer::after(POLL_INTERVAL).await;
    }
}