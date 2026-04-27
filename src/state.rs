// state.rs

use crate::protocol::IconType;

#[derive(Debug, Clone, Copy)]
pub struct SlotState {
    pub icon: IconType,
    pub volume: u8,
    pub muted: bool
}

pub struct DisplayState {
    pub profile_name: &'static str,
    pub slots: [SlotState; 4],
}

impl Default for DisplayState {
    fn default() -> Self {
        Self {
            profile_name: "Main",
            slots: [
                SlotState { icon: IconType::Master, volume: 255, muted: false },
                SlotState { icon: IconType::Spotify, volume: 255, muted: true },
                SlotState { icon: IconType::Discord, volume: 80, muted: false },
                SlotState { icon: IconType::Browser, volume: 35, muted: false },
            ],
        }
    }
}