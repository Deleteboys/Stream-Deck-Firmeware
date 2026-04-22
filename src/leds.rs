use crate::protocol::HostToPico;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
// HIER NEU: Grb importiert
use embassy_rp::pio_programs::ws2812::{PioWs2812, Grb};
use smart_leds::RGB8;

pub static LED_COMMAND_CHANNEL: Channel<ThreadModeRawMutex, HostToPico, 4> = Channel::new();

#[embassy_executor::task]
pub async fn led_task(
    // HIER NEU: Grb ist nun das vierte Argument in der Liste!
    mut ws2812: PioWs2812<'static, embassy_rp::peripherals::PIO0, 0, 13, Grb>
) {
    const NUM_LEDS: usize = 13;
    let mut colors = [RGB8::default(); NUM_LEDS];

    loop {
        let msg = LED_COMMAND_CHANNEL.receive().await;

        match msg {
            HostToPico::FillAll { r, g, b } => {
                for i in 0..NUM_LEDS {
                    colors[i] = RGB8 { r, g, b };
                }
                ws2812.write(&colors).await;
            }
            _ => {}
        }
    }
}