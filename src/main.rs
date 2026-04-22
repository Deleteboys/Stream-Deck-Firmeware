#![no_std]
#![no_main]

mod inputs;
mod usb;
mod leds;
mod display;
mod protocol;
mod state;

use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::{USB, PIO0, DMA_CH0};
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_usb::{Builder, Config};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use static_cell::StaticCell;

// --- NEU: Imports für die LEDs ---
use embassy_rp::pio::{InterruptHandler as PioInterruptHandler, Pio};
use embassy_rp::pio_programs::ws2812::{PioWs2812, PioWs2812Program};
use embassy_rp::dma::InterruptHandler as DmaInterruptHandler;

// --- NEU: Interrupts für PIO und DMA hinzugefügt ---
bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    PIO0_IRQ_0 => PioInterruptHandler<PIO0>;
    DMA_IRQ_0 => DmaInterruptHandler<DMA_CH0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // ==========================================
    // 1. USB HARDWARE SETUP
    // ==========================================
    let driver = Driver::new(p.USB, Irqs);

    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Stream Deck Labor");
    config.product = Some("Pico Streamdeck");
    config.serial_number = Some("123456");
    config.max_packet_size_0 = 64;

    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    static STATE: StaticCell<State> = StaticCell::new();

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        &mut [],
        CONTROL_BUF.init([0; 64]),
    );

    let class = CdcAcmClass::new(&mut builder, STATE.init(State::new()), 64);
    let usb_device = builder.build();

    // ==========================================
    // 2. LED HARDWARE SETUP (PIO & PIN_26)
    // ==========================================
    let mut pio = Pio::new(p.PIO0, Irqs);
    let program = PioWs2812Program::new(&mut pio.common);

    // HIER WIRD DER PIN DEFINIERT!
    // Falls du auf dem Breadboard einen anderen Pin nutzt, einfach p.PIN_26 abändern.
    let ws2812 = PioWs2812::new(
        &mut pio.common,
        pio.sm0,
        p.DMA_CH0,
        Irqs,
        p.PIN_26,
        &program
    );

    // ==========================================
    // 3. TASKS STARTEN
    // ==========================================
    spawner.spawn(usb::usb_driver_task(usb_device).unwrap());
    spawner.spawn(usb::usb_comm_task(class).unwrap());

    // NEU: Wir starten deinen LED-Task und übergeben ihm das fertige Hardware-Objekt
    spawner.spawn(leds::led_task(ws2812).unwrap());

    // ==========================================
    // 4. MAIN LOOP (hält den Pico wach)
    // ==========================================
    // In deiner main.rs (Pico)
    let mut led = embassy_rp::gpio::Output::new(p.PIN_25, embassy_rp::gpio::Level::Low); // Die Onboard-LED (bei Pico 1)

    loop {
        led.toggle(); // LED blinkt
        embassy_time::Timer::after_millis(500).await;
    }
}