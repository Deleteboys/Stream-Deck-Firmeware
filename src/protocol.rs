use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum HostToPico {
    Ping,
    StartBootloader,
    FillAll {
        r: u8,
        g: u8,
        b: u8,
        brightness: u8,
    },
    SetLed {
        index: u8,
        r: u8,
        g: u8,
        b: u8,
        brightness: u8,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PicoToHost {
    Hello,
    EncoderTurned { id: u8, delta: i8 },
    ButtonPressed(u8),
    Log(heapless::String<64>),
}
