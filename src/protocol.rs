use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum LedEffect {
    Solid {
        r: u8,
        g: u8,
        b: u8,
        brightness: u8,
    },
    Blink {
        r: u8,
        g: u8,
        b: u8,
        brightness: u8,
        speed: u8,
    },
    Rainbow {
        brightness: u8,
        speed: u8,
    },
    Breathing {
        r: u8,
        g: u8,
        b: u8,
        brightness: u8,
        speed: u8,
    },
    Chase {
        r: u8,
        g: u8,
        b: u8,
        brightness: u8,
        speed: u8,
        size: u8,
    },
    Comet {
        r: u8,
        g: u8,
        b: u8,
        brightness: u8,
        speed: u8,
        tail: u8,
    },
    Sparkle {
        r: u8,
        g: u8,
        b: u8,
        brightness: u8,
        speed: u8,
        density: u8,
    },
    Aurora {
        brightness: u8,
        speed: u8,
    },
    ColorOrbit {
        hue: u8,
        hue_shift: u8,
        saturation: u8,
        brightness: u8,
        speed: u8,
    },
    Astolfo {
        brightness: u8,
        speed: u8,
        saturation: u8,
        spread: u8,
    },
}

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
    SetEffect {
        effect: LedEffect,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PicoToHost {
    Hello,
    EncoderTurned { id: u8, delta: i8 },
    ButtonChanged { id: u8, pressed: bool },
    Log(heapless::String<64>),
}
