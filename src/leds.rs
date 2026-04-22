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
            HostToPico::FillAll { r, g, b } => {
                set_all(&mut colors, RGB8 { r, g, b });
                ws2812.write(&colors).await;
            }
            HostToPico::SetLed { index, r, g, b } => {
                if let Some(led) = colors.get_mut(index as usize) {
                    *led = RGB8 { r, g, b };
                    ws2812.write(&colors).await;
                }
            }
            HostToPico::Ping | HostToPico::StartBootloader => {}
        }
    }
}

fn set_all(colors: &mut [RGB8; NUM_LEDS], color: RGB8) {
    for led in colors.iter_mut() {
        *led = color;
    }
}
