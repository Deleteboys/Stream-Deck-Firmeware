use serde::{Deserialize, Serialize};

/// Das schickt der PC an den Pico
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum HostToPico {
    Ping,                    // Index 0
    StartBootloader,         // Index 1
    FillAll { r: u8, g: u8, b: u8 }, // Index 2
    SetLed { index: u8, r: u8, g: u8, b: u8 }, // Index 3
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PicoToHost {
    Hello,                   // Index 0
    EncoderTurned { id: u8, delta: i8 }, // Index 1
    ButtonPressed(u8),       // Index 2
    Log(heapless::String<64>), // Index 3
}