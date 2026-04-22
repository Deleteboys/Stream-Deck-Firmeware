use crate::protocol::{HostToPico, PicoToHost};
use embassy_rp::peripherals::USB;
use embassy_rp::rom_data::reset_to_usb_boot;
use embassy_rp::usb::Driver;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embassy_usb::class::cdc_acm::CdcAcmClass;
use embassy_usb::UsbDevice;

// Über diesen Channel können alle Tasks Nachrichten in Richtung PC werfen
pub static USB_TX_CHANNEL: Channel<ThreadModeRawMutex, PicoToHost, 16> = Channel::new();

// Der Hardware-Task, der den USB-Bus am Leben hält
#[embassy_executor::task]
pub async fn usb_driver_task(mut usb: UsbDevice<'static, Driver<'static, USB>>) -> ! {
    usb.run().await
}

// Der Task, der liest und schreibt
// In src/usb.rs
use embassy_time::{Duration, with_timeout};

#[embassy_executor::task]
pub async fn usb_comm_task(mut class: CdcAcmClass<'static, Driver<'static, USB>>) -> ! {
    let mut buf = [0; 64];

    loop {
        class.wait_connection().await;
        let _ = send_packet(&mut class, &to_log("Pico Online")).await;

        loop {
            // "with_timeout" bricht das Warten nach 2 Sekunden ab
            let select_fut = embassy_futures::select::select(
                class.read_packet(&mut buf),
                USB_TX_CHANNEL.receive()
            );

            match with_timeout(Duration::from_secs(2), select_fut).await {
                // Ein Event ist innerhalb der 2 Sek eingetroffen
                Ok(either) => {
                    match either {
                        embassy_futures::select::Either::First(result) => {
                            match result {
                                Ok(len) => {
                                    if let Ok(msg) = postcard::from_bytes::<HostToPico>(&buf[..len]) {
                                        handle_host_command(msg, &mut class).await;
                                    }
                                }
                                Err(_) => break, // Verbindung weg
                            }
                        }
                        embassy_futures::select::Either::Second(msg_to_send) => {
                            if send_packet(&mut class, &msg_to_send).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                // TIMEOUT: 2 Sekunden lang ist nichts passiert
                Err(_) => {
                    // Schicke ein Lebenszeichen an den PC
                    // let _ = send_packet(&mut class, &to_log("Heartbeat...")).await;
                }
            }
        }
    }
}

async fn handle_host_command(
    msg: HostToPico,
    class: &mut CdcAcmClass<'static, Driver<'static, USB>>,
) {
    match msg {
        HostToPico::StartBootloader => {
            let _ = send_packet(class, &to_log("Rebooting to Bootloader...")).await;
            embassy_time::Timer::after_millis(200).await;
            reset_to_usb_boot(0, 0);
        }

        HostToPico::FillAll { r, g, b } => {
            let _ = send_packet(class, &to_log("LED: Fill All")).await;
            let _ = crate::leds::LED_COMMAND_CHANNEL.try_send(msg);
        }

        // HIER IST DER NEUE TEIL:
        HostToPico::SetLed { index, r, g, b } => {
            let _ = send_packet(class, &to_log("LED: Set Single")).await;
            // Wir leiten den Befehl einfach an den LED-Task weiter
            let _ = crate::leds::LED_COMMAND_CHANNEL.try_send(msg);
        }

        HostToPico::Ping => {
            let _ = send_packet(class, &to_log("Pong!")).await;
        }
    }
}

// Hilfsfunktion zum Serialisieren und Senden
// Ändere die Zeile 62 (ungefähr) in src/usb.rs zu:
async fn send_packet(
    class: &mut CdcAcmClass<'static, Driver<'static, USB>>,
    msg: &PicoToHost,
) -> Result<(), embassy_usb::driver::EndpointError> {
    // <--- Hier das .driver. einfügen
    let mut buf = [0u8; 64];
    if let Ok(slice) = postcard::to_slice(msg, &mut buf) {
        class.write_packet(slice).await?;
    }
    Ok(())
}


pub async fn log_to_pc(text: &str) {
    let mut s = heapless::String::new();
    let _ = s.push_str(text);
    let _ = crate::usb::USB_TX_CHANNEL.send(crate::protocol::PicoToHost::Log(s)).await;
}

fn to_log(s: &str) -> PicoToHost {
    let mut h_string = heapless::String::<64>::new();
    let _ = h_string.push_str(s); // Wir ignorieren Fehler, da wir wissen, dass unsere Texte < 64 Zeichen sind
    PicoToHost::Log(h_string)
}