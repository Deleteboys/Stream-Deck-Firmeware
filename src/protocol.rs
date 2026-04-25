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
        saturation: u8,
        #[serde(default)]
        reverse: bool,
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
        #[serde(default)]
        reverse: bool,
    },
    Comet {
        r: u8,
        g: u8,
        b: u8,
        brightness: u8,
        speed: u8,
        tail: u8,
        #[serde(default)]
        reverse: bool,
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
        #[serde(default)]
        reverse: bool,
    },
    ColorOrbit {
        hue: u8,
        hue_shift: u8,
        saturation: u8,
        brightness: u8,
        speed: u8,
        #[serde(default)]
        reverse: bool,
    },
    Astolfo {
        brightness: u8,
        speed: u8,
        saturation: u8,
        spread: u8,
        #[serde(default)]
        reverse: bool,
    },
}

impl Default for LedEffect {
    fn default() -> Self {
        LedEffect::Rainbow {
            brightness: 100,
            speed: 96,
            saturation: 255,
            reverse: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct DeviceConfig {
    pub led_effect: LedEffect,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            led_effect: LedEffect::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum HostToPico {
    Ping,
    StartBootloader,
    GetConfig,
    SetConfig {
        config: DeviceConfig,
    },
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
    EncoderChanged { id: u8, pressed: bool},
    ButtonChanged { id: u8, pressed: bool },
    Config { config: DeviceConfig },
    ConfigSaved,
    ConfigSaveFailed,
    Log(heapless::String<64>),
}
