use crate::protocol::HostToPico;
use embassy_rp::pio_programs::ws2812::{Grb, PioWs2812};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use smart_leds::RGB8;

pub static LED_COMMAND_CHANNEL: Channel<ThreadModeRawMutex, HostToPico, 4> = Channel::new();

const NUM_LEDS: usize = 13;
type LedBus = PioWs2812<'static, embassy_rp::peripherals::PIO0, 0, 13, Grb>;

#[embassy_executor::task]
pub async fn led_task(mut ws2812: LedBus) {
    let mut colors = [RGB8::default(); NUM_LEDS];

    loop {
        match LED_COMMAND_CHANNEL.receive().await {
            HostToPico::FillAll {
                r,
                g,
                b,
                brightness,
            } => {
                set_all(&mut colors, apply_brightness(r, g, b, brightness));
                ws2812.write(&colors).await;
            }
            HostToPico::SetLed {
                index,
                r,
                g,
                b,
                brightness,
            } => {
                if let Some(led) = colors.get_mut(index as usize) {
                    *led = apply_brightness(r, g, b, brightness);
                    ws2812.write(&colors).await;
                }
            }
            _ => {}
        }
    }
}

fn set_all(colors: &mut [RGB8; NUM_LEDS], color: RGB8) {
    for led in colors.iter_mut() {
        *led = color;
    }
}

fn apply_brightness(r: u8, g: u8, b: u8, brightness: u8) -> RGB8 {
    RGB8 {
        r: scale(r, brightness),
        g: scale(g, brightness),
        b: scale(b, brightness),
    }
}

fn scale(component: u8, brightness: u8) -> u8 {
    ((component as u16 * brightness as u16) / 255) as u8
}
