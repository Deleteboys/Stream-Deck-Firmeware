use embassy_rp::gpio::Output;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::Timer;

pub enum VibrationPattern {
    Short,
    Medium,
    Long,
    Custom(u64),
}

pub static VIBRATION_TRIGGER_CHANNEL: Channel<ThreadModeRawMutex, VibrationPattern, 2> = Channel::new();

#[embassy_executor::task]
pub async fn vibration_task(mut motor: Output<'static>) {
    loop {
        let pattern = VIBRATION_TRIGGER_CHANNEL.receive().await;

        let ms = match pattern {
            VibrationPattern::Short => 100,
            VibrationPattern::Medium => 150,
            VibrationPattern::Long => 350,
            VibrationPattern::Custom(custom_ms) => custom_ms,
        };

        motor.set_high();
        Timer::after_millis(ms).await;
        motor.set_low();

        Timer::after_millis(20).await;
    }
}
