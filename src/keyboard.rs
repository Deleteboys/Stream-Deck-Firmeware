use core::sync::atomic::{AtomicBool, Ordering};
use embassy_rp::peripherals::USB;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;
use embassy_usb::class::hid::HidWriter;
use usbd_hid::descriptor::KeyboardReport;

pub static SIMPLE_MODE_ACTIVE: AtomicBool = AtomicBool::new(false);
pub static HID_TX_CHANNEL: Channel<ThreadModeRawMutex, KeyboardReport, 16> = Channel::new();
#[embassy_executor::task]
pub async fn usb_hid_task(mut hid: HidWriter<'static, embassy_rp::usb::Driver<'static, USB>, 8>) {
    loop {
        let report = HID_TX_CHANNEL.receive().await;

        if report.modifier != 0 {
            let mod_only = KeyboardReport {
                modifier: report.modifier,
                reserved: 0,
                leds: 0,
                keycodes: [0; 6],
            };
            let _ = hid.write_serialize(&mod_only).await;

            Timer::after_millis(5).await;
        }
        // -----------------------------------------

        let _ = hid.write_serialize(&report).await;

        Timer::after_millis(20).await;

        let empty_report = KeyboardReport {
            modifier: 0,
            reserved: 0,
            leds: 0,
            keycodes: [0; 6],
        };
        let _ = hid.write_serialize(&empty_report).await;
    }
}
pub struct KeyboardMapper;

impl KeyboardMapper {
    /// Gibt zurück, ob der Simple Mode gerade aktiv ist.
    pub fn is_active() -> bool {
        SIMPLE_MODE_ACTIVE.load(Ordering::Relaxed)
    }

    /// Schaltet den Modus um (Normal <-> Simple) und gibt den neuen Status zurück.
    pub fn toggle() -> bool {
        let current = Self::is_active();
        let new_state = !current;
        SIMPLE_MODE_ACTIVE.store(new_state, Ordering::Relaxed);
        new_state
    }

    /// Mappt die 8 normalen Tasten (IDs 0-7) auf F13 bis F20
    pub fn send_button(id: u8) {
        if id < 8 {
            // Hex 0x68 ist F13 im USB HID Standard
            let keycode = 0x68 + id;
            let _ = HID_TX_CHANNEL.try_send(Self::build_report(0, keycode));
        }
    }

    /// Mappt die 4 Encoder-Klicks (IDs 0-3) auf F21 bis F24
    pub fn send_encoder_push(id: u8) {
        if id < 4 {
            // Hex 0x70 ist F21 im USB HID Standard
            let keycode = 0x70 + id;
            let _ = HID_TX_CHANNEL.try_send(Self::build_report(0, keycode));
        }
    }

    /// Mappt die Encoder-Drehungen auf F13-F16, ABER mit Modifikatoren (Shift/Strg)
    pub fn send_encoder_turn(id: u8, delta: i8) {
        if id < 4 {
            // F13 bis F16 als Basis für die 4 Encoder
            let keycode = 0x68 + id;

            // Wenn nach rechts gedreht (+1) -> STRG (0x01)
            // Wenn nach links gedreht (-1) -> SHIFT (0x02)
            let modifier = if delta > 0 { 0x01 } else { 0x02 };

            let _ = HID_TX_CHANNEL.try_send(Self::build_report(modifier, keycode));
        }
    }
    fn build_report(modifier: u8, keycode: u8) -> KeyboardReport {
        KeyboardReport {
            modifier,
            reserved: 0,
            leds: 0,
            keycodes: [keycode, 0, 0, 0, 0, 0],
        }
    }
}