#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{PIO0, USB, DMA_CH0};
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_time::Timer;
use embassy_usb::Builder;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::Config;
use static_cell::StaticCell;
use embassy_rp::bootsel::is_bootsel_pressed;
use embassy_rp::rom_data::reset_to_usb_boot;

use embassy_rp::pio::{InterruptHandler as PioInterruptHandler, Pio};
use embassy_rp::pio_programs::ws2812::{PioWs2812, PioWs2812Program};
use embassy_rp::dma::InterruptHandler as DmaInterruptHandler;

// NEU: Importiere die HSV-Funktionen für den Regenbogen
use smart_leds::RGB8;
use smart_leds::hsv::{Hsv, hsv2rgb};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    PIO0_IRQ_0 => PioInterruptHandler<PIO0>;
    DMA_IRQ_0 => DmaInterruptHandler<DMA_CH0>;
});

macro_rules! usb_print {
    ($class:expr, $text:expr) => {
        if $class.dtr() {
            let _ = $class.write_packet($text).await;
        }
    };
}

#[embassy_executor::task]
async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, Driver<'static, USB>>) -> ! {
    usb.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let driver = Driver::new(p.USB, Irqs);

    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Stream Deck Labor");
    config.product = Some("Pico Serial Log");
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

    let mut class = CdcAcmClass::new(&mut builder, STATE.init(State::new()), 64);
    let usb = builder.build();

    spawner.spawn(usb_task(usb).unwrap());

    let mut led = Output::new(p.PIN_25, Level::Low);
    let mut bootsel_pin = p.BOOTSEL;

    // --- WS2812 Setup an PIN 18 ---
    let mut pio = Pio::new(p.PIO0, Irqs);
    let program = PioWs2812Program::new(&mut pio.common);
    let mut ws2812 = PioWs2812::new(&mut pio.common, pio.sm0, p.DMA_CH0, Irqs, p.PIN_26, &program);

    const NUM_LEDS: usize = 13;
    let mut colors = [RGB8::default(); NUM_LEDS];

    // Dieser Wert merkt sich, wo auf dem Farbrad wir gerade sind
    let mut offset: u8 = 0;

    loop {
        // 1. Berechne die Farbe für jede einzelne LED
        for i in 0..NUM_LEDS {
            // Verteile die 256 Farben des Regenbogens gleichmäßig über die Anzahl deiner LEDs
            let hue_step = (i * 256 / NUM_LEDS) as u8;

            // Addiere den aktuellen Offset, damit sich die Farben bewegen.
            // wrapping_add sorgt dafür, dass nach 255 wieder bei 0 angefangen wird (wie bei einem Kreis).
            let hue = offset.wrapping_add(hue_step);

            // Konvertiere HSV zu RGB und speichere es in unserem Array
            colors[i] = hsv2rgb(Hsv {
                hue,
                sat: 255, // Volle Sättigung (kräftige Farben)
                val: 40,  // Helligkeit (40 von 255 ist super hell, aber sicher für USB)
            });
        }

        // 2. Sende die berechneten Farben an den Streifen
        ws2812.write(&colors).await;

        // 3. Verschiebe das Farbrad für den nächsten Durchlauf
        // Je höher diese Zahl (z.B. 2 oder 3), desto schneller "fließt" der Regenbogen
        offset = offset.wrapping_add(2);

        // 4. BOOTSEL Check für Flashing
        if is_bootsel_pressed(bootsel_pin.reborrow()) {
            usb_print!(class, b"BOOTSEL erkannt! Gehe in Flash-Modus...\r\n");
            Timer::after_millis(100).await;
            reset_to_usb_boot(0, 0);
        }

        // 5. Kurze Pause (20ms entsprechen etwa 50 Bildern pro Sekunde, also extrem flüssig)
        Timer::after_millis(20).await;
    }
}