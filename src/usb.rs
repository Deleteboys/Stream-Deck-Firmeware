use crate::protocol::{HostToPico, PicoToHost};
use embassy_rp::peripherals::USB;
use embassy_rp::rom_data::reset_to_usb_boot;
use embassy_rp::usb::Driver;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{with_timeout, Duration};
use embassy_usb::class::cdc_acm::CdcAcmClass;
use embassy_usb::UsbDevice;

pub static USB_TX_CHANNEL: Channel<ThreadModeRawMutex, PicoToHost, 16> = Channel::new();

type UsbDriver = Driver<'static, USB>;
type UsbClass = CdcAcmClass<'static, UsbDriver>;

const USB_PACKET_SIZE: usize = 64;
const CONNECTION_IDLE_TIMEOUT: Duration = Duration::from_secs(2);

#[embassy_executor::task]
pub async fn usb_driver_task(mut usb: UsbDevice<'static, UsbDriver>) -> ! {
    usb.run().await
}

#[embassy_executor::task]
pub async fn usb_comm_task(mut class: UsbClass) -> ! {
    let mut buf = [0; USB_PACKET_SIZE];

    loop {
        class.wait_connection().await;
        let _ = send_packet(&mut class, &to_log("Pico Online")).await;

        loop {
            let select_fut = embassy_futures::select::select(
                class.read_packet(&mut buf),
                USB_TX_CHANNEL.receive(),
            );

            match with_timeout(CONNECTION_IDLE_TIMEOUT, select_fut).await {
                Ok(embassy_futures::select::Either::First(Ok(len))) => {
                    if let Ok(msg) = postcard::from_bytes::<HostToPico>(&buf[..len]) {
                        handle_host_command(msg, &mut class).await;
                    }
                }
                Ok(embassy_futures::select::Either::First(Err(_))) => break,
                Ok(embassy_futures::select::Either::Second(msg_to_send)) => {
                    if send_packet(&mut class, &msg_to_send).await.is_err() {
                        break;
                    }
                }
                Err(_) => {}
            }
        }
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
            let _ = crate::leds::LED_COMMAND_CHANNEL.try_send(msg);
        }
        HostToPico::Ping => {
            let _ = send_packet(class, &to_log("Pong!")).await;
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

pub async fn log_to_pc(text: &str) {
    let mut s = heapless::String::new();
    let _ = s.push_str(text);
    let _ = USB_TX_CHANNEL.send(PicoToHost::Log(s)).await;
}

fn to_log(s: &str) -> PicoToHost {
    let mut h_string = heapless::String::<64>::new();
    let _ = h_string.push_str(s);
    PicoToHost::Log(h_string)
}
