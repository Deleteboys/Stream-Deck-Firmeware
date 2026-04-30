#![no_std]
#![no_main]

mod config;
mod display;
pub mod icons;
mod inputs;
mod keyboard;
mod leds;
mod protocol;
mod state;
mod usb;
mod vibration;

use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::dma::InterruptHandler as DmaInterruptHandler;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::i2c::InterruptHandler as I2cInterruptHandler;
use embassy_rp::i2c::{Config as I2cConfig, I2c};
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1, I2C0, PIO0, USB};
use embassy_rp::pio::{InterruptHandler as PioInterruptHandler, Pio};
use embassy_rp::pio_programs::ws2812::{PioWs2812, PioWs2812Program};
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embassy_usb::class::hid::{Config as HidConfig, HidBootProtocol, HidSubclass, HidWriter, State as HidState};
use embassy_usb::{Builder, Config as UsbConfig};
use panic_probe as _;
use static_cell::StaticCell;
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};

const USB_EP0_PACKET_SIZE: u8 = 64;
const CDC_PACKET_SIZE: u16 = 64;
const ONBOARD_BLINK_MS: u64 = 500;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    PIO0_IRQ_0 => PioInterruptHandler<PIO0>;
    DMA_IRQ_0 => DmaInterruptHandler<DMA_CH0>, DmaInterruptHandler<DMA_CH1>;
    I2C0_IRQ => I2cInterruptHandler<I2C0>; // <-- NEU: Interrupt für I2C0
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let driver = Driver::new(p.USB, Irqs);
    let mut config = UsbConfig::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Stream Deck Labor");
    config.product = Some("Pico Streamdeck");
    config.serial_number = Some("123456");
    config.max_packet_size_0 = USB_EP0_PACKET_SIZE;

    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    static STATE: StaticCell<State> = StaticCell::new();

    static POWER_HANDLER: StaticCell<usb::MyPowerHandler> = StaticCell::new();
    let power_handler = POWER_HANDLER.init(usb::MyPowerHandler);

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        &mut [],
        CONTROL_BUF.init([0; 64]),
    );

    builder.handler(power_handler);

    static HID_STATE: StaticCell<HidState> = StaticCell::new();
    let hid_config = HidConfig {
        report_descriptor: KeyboardReport::desc(),
        request_handler: None,
        poll_ms: 10,
        max_packet_size: 8,
        hid_subclass: HidSubclass::No,
        hid_boot_protocol: HidBootProtocol::None,
    };
    let hid_writer = HidWriter::new(&mut builder, HID_STATE.init(HidState::new()), hid_config);

    let class = CdcAcmClass::new(&mut builder, STATE.init(State::new()), CDC_PACKET_SIZE);
    let usb_device = builder.build();

    let mut pio = Pio::new(p.PIO0, Irqs);
    let program = PioWs2812Program::new(&mut pio.common);
    let ws2812 = PioWs2812::new(
        &mut pio.common,
        pio.sm0,
        p.DMA_CH0,
        Irqs,
        p.PIN_26,
        &program,
    );

    let flash =
        embassy_rp::flash::Flash::<_, _, { 2 * 1024 * 1024 }>::new(p.FLASH, p.DMA_CH1, Irqs);
    let mut config_storage = config::new_storage(flash);
    let device_config = config::load_config(&mut config_storage).await;

    spawner.spawn(usb::usb_driver_task(usb_device).unwrap());
    spawner.spawn(usb::usb_comm_task(class).unwrap());
    spawner.spawn(config::config_task(config_storage, device_config).unwrap());
    spawner.spawn(keyboard::usb_hid_task(hid_writer).unwrap());
    spawner.spawn(leds::led_task(ws2812, device_config.led_effect).unwrap());

    let mut i2c_config = I2cConfig::default();
    i2c_config.frequency = 400_000;
    let i2c_display = I2c::new_async(p.I2C0, p.PIN_21, p.PIN_20, Irqs, i2c_config);
    spawner.spawn(display::display_task(i2c_display).unwrap()); // Angepasster Name

    let buttons = [
        Input::new(p.PIN_0, Pull::Up),
        Input::new(p.PIN_1, Pull::Up),
        Input::new(p.PIN_2, Pull::Up),
        Input::new(p.PIN_3, Pull::Up),
        Input::new(p.PIN_4, Pull::Up),
        Input::new(p.PIN_5, Pull::Up),
        Input::new(p.PIN_6, Pull::Up),
        Input::new(p.PIN_7, Pull::Up),
    ];
    spawner.spawn(inputs::buttons::button_task(buttons).unwrap());

    let encoders = [
        (
            Input::new(p.PIN_9, Pull::Up),
            Input::new(p.PIN_10, Pull::Up),
        ),
        (
            Input::new(p.PIN_12, Pull::Up),
            Input::new(p.PIN_13, Pull::Up),
        ),
        (
            Input::new(p.PIN_15, Pull::Up),
            Input::new(p.PIN_16, Pull::Up),
        ),
        (
            Input::new(p.PIN_18, Pull::Up),
            Input::new(p.PIN_19, Pull::Up),
        ),
    ];
    let encoder_buttons = [
        Input::new(p.PIN_8, Pull::Up),
        Input::new(p.PIN_11, Pull::Up),
        Input::new(p.PIN_14, Pull::Up),
        Input::new(p.PIN_17, Pull::Up),
    ];
    spawner.spawn(inputs::encoders::encoder_task(encoders, encoder_buttons).unwrap());

    let vibration_motor = Output::new(p.PIN_22, Level::Low);
    spawner.spawn(vibration::vibration_task(vibration_motor).unwrap());

    let mut led = Output::new(p.PIN_25, Level::Low);
    loop {
        led.toggle();
        embassy_time::Timer::after_millis(ONBOARD_BLINK_MS).await;
    }
}
