use crate::protocol::{HostToPico, PicoToHost};
use embassy_rp::peripherals::USB;
use embassy_rp::rom_data::reset_to_usb_boot;
use embassy_rp::usb::Driver;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embassy_usb::class::cdc_acm::CdcAcmClass;
use embassy_usb::UsbDevice;
use embassy_usb::Handler;

pub static USB_TX_CHANNEL: Channel<ThreadModeRawMutex, PicoToHost, 16> = Channel::new();

type UsbDriver = Driver<'static, USB>;
type UsbClass = CdcAcmClass<'static, UsbDriver>;

const USB_PACKET_SIZE: usize = 64;

#[embassy_executor::task]
pub async fn usb_driver_task(mut usb: UsbDevice<'static, UsbDriver>) -> ! {
    usb.run().await
}

#[embassy_executor::task]
pub async fn usb_comm_task(mut class: UsbClass) -> ! {
    let mut buf = [0; USB_PACKET_SIZE];

    loop {
        class.wait_connection().await;

        // PC verbunden -> Aufwachen signalisieren
        let _ = crate::leds::LED_COMMAND_CHANNEL.try_send(crate::leds::LedCommand::Resume);
        let _ = crate::display::DISPLAY_COMMAND_CHANNEL.try_send(crate::display::DisplayCommand::Resume);

        let _ = send_packet(&mut class, &to_log("Pico Online")).await;

        loop {
            let select_fut = embassy_futures::select::select(
                class.read_packet(&mut buf),
                USB_TX_CHANNEL.receive(),
            ).await;

            match select_fut {
                embassy_futures::select::Either::First(Ok(len)) => {
                    if let Ok(msg) = postcard::from_bytes::<HostToPico>(&buf[..len]) {
                        handle_host_command(msg, &mut class).await;
                    }
                }
                embassy_futures::select::Either::First(Err(_)) => break, // Verbindung getrennt
                embassy_futures::select::Either::Second(msg_to_send) => {
                    if send_packet(&mut class, &msg_to_send).await.is_err() {
                        break; // Senden fehlgeschlagen, Verbindung wohl weg
                    }
                }
            }
        }

        // Verbindung verloren -> Schlafen signalisieren
        let _ = crate::leds::LED_COMMAND_CHANNEL.try_send(crate::leds::LedCommand::Suspend);
        let _ = crate::display::DISPLAY_COMMAND_CHANNEL.try_send(crate::display::DisplayCommand::Suspend);
    }
}

async fn handle_host_command(msg: HostToPico, class: &mut UsbClass) {
    match msg {
        HostToPico::StartBootloader => {
            let _ = send_packet(class, &to_log("Rebooting to Bootloader...")).await;
            embassy_time::Timer::after_millis(200).await;
            reset_to_usb_boot(0, 0);
        }
        HostToPico::FillAll { .. } | HostToPico::SetLed { .. } => {
            crate::leds::LED_COMMAND_CHANNEL.send(crate::leds::LedCommand::HostCommand(msg)).await;
        }
        HostToPico::SetEffect { effect } => {
            crate::leds::LED_COMMAND_CHANNEL
                .send(crate::leds::LedCommand::HostCommand(HostToPico::SetEffect { effect }))
                .await;
            crate::config::CONFIG_COMMAND_CHANNEL
                .send(crate::config::ConfigCommand::SaveLedEffect(effect))
                .await;
        }
        HostToPico::GetConfig => {
            crate::config::CONFIG_COMMAND_CHANNEL
                .send(crate::config::ConfigCommand::SendConfigToHost)
                .await;
        }
        HostToPico::SetConfig { config } => {
            crate::leds::LED_COMMAND_CHANNEL
                .send(crate::leds::LedCommand::HostCommand(HostToPico::SetEffect {
                    effect: config.led_effect,
                }))
                .await;
            crate::config::CONFIG_COMMAND_CHANNEL
                .send(crate::config::ConfigCommand::SetConfig(config))
                .await;
        }
        HostToPico::Ping => {
            let _ = send_packet(class, &to_log("Pong!")).await;
        }
    }
}

pub struct MyPowerHandler;

impl Handler for MyPowerHandler {
    fn enabled(&mut self, enabled: bool) {
        if !enabled {
            // Strom wurde komplett entfernt
            let _ = crate::leds::LED_COMMAND_CHANNEL.try_send(crate::leds::LedCommand::Suspend);
            let _ = crate::display::DISPLAY_COMMAND_CHANNEL.try_send(crate::display::DisplayCommand::Suspend);
        }
    }

    fn suspended(&mut self, suspended: bool) {
        if suspended {
            // Der USB-Bus ist im Suspend-Modus (PC schläft)
            let _ = crate::leds::LED_COMMAND_CHANNEL.try_send(crate::leds::LedCommand::Suspend);
            let _ = crate::display::DISPLAY_COMMAND_CHANNEL.try_send(crate::display::DisplayCommand::Suspend);
        } else {
            // Der USB-Bus ist wieder aktiv (PC wacht auf)
            let _ = crate::leds::LED_COMMAND_CHANNEL.try_send(crate::leds::LedCommand::Resume);
            let _ = crate::display::DISPLAY_COMMAND_CHANNEL.try_send(crate::display::DisplayCommand::Resume);
        }
    }
}

async fn send_packet(
    class: &mut UsbClass,
    msg: &PicoToHost,
) -> Result<(), embassy_usb::driver::EndpointError> {
    let mut buf = [0u8; USB_PACKET_SIZE];
    if let Ok(slice) = postcard::to_slice(msg, &mut buf) {
        class.write_packet(slice).await?;
    }
    Ok(())
}

fn to_log(s: &str) -> PicoToHost {
    let mut h_string = heapless::String::<64>::new();
    let _ = h_string.push_str(s);
    PicoToHost::Log(h_string)
}