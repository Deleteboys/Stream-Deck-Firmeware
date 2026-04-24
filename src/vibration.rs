use embassy_rp::gpio::Output;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;

const VIBRATION_PULSE_MS: u64 = 120;

pub static VIBRATION_TRIGGER_CHANNEL: Channel<ThreadModeRawMutex, (), 8> = Channel::new();

#[embassy_executor::task]
pub async fn vibration_task(mut motor: Output<'static>) {
    loop {
        let _ = VIBRATION_TRIGGER_CHANNEL.receive().await;
        motor.set_high();
        Timer::after_millis(VIBRATION_PULSE_MS).await;
        motor.set_low();
    }
}
