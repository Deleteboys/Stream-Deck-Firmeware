pub struct Debouncer {
    stable_pressed: bool,
    last_sample: bool,
    changed_at: embassy_time::Instant,
    debounce_time: embassy_time::Duration,
}

impl Debouncer {
    pub fn new(debounce_time: embassy_time::Duration) -> Self {
        Self {
            stable_pressed: false,
            last_sample: false,
            changed_at: embassy_time::Instant::now(),
            debounce_time,
        }
    }

    pub fn update(&mut self, is_low: bool) -> Option<bool> {
        let now = embassy_time::Instant::now();
        let pressed = is_low;
        if pressed != self.last_sample {
            self.last_sample = pressed;
            self.changed_at = now;
        }
        if pressed != self.stable_pressed && now.duration_since(self.changed_at) >= self.debounce_time {
            self.stable_pressed = pressed;
            return Some(pressed);
        }
        None
    }
}