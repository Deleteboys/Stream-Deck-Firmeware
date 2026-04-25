use crate::protocol::{HostToPico, LedEffect};
use embassy_rp::pio_programs::ws2812::{Grb, PioWs2812};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{with_timeout, Duration};
use smart_leds::RGB8;

pub enum LedCommand {
    HostCommand(HostToPico),
    Suspend,
    Resume,
}

pub static LED_COMMAND_CHANNEL: Channel<ThreadModeRawMutex, LedCommand, 4> = Channel::new();

const NUM_LEDS: usize = 13;
const FRAME_TIME: Duration = Duration::from_millis(16);
type LedBus = PioWs2812<'static, embassy_rp::peripherals::PIO0, 0, 13, Grb>;

#[derive(Clone, Copy)]
enum LedMode {
    Manual,
    Effect(LedEffect),
}

#[embassy_executor::task]
pub async fn led_task(mut ws2812: LedBus, initial_effect: LedEffect) {
    let mut colors = [RGB8::default(); NUM_LEDS];
    let mut mode = LedMode::Effect(initial_effect);
    let mut frame: u32 = 0;
    let mut noise_seed: u32 = 0x1234_abcd;
    let mut is_suspended = false;

    render_effect(initial_effect, frame, &mut noise_seed, &mut colors);
    ws2812.write(&colors).await;

    loop {
        match with_timeout(FRAME_TIME, LED_COMMAND_CHANNEL.receive()).await {
            Ok(cmd) => match cmd {
                LedCommand::Suspend => {
                    is_suspended = true;
                    set_all(&mut colors, RGB8::default());
                    ws2812.write(&colors).await;
                }
                LedCommand::Resume => {
                    is_suspended = false;
                    frame = 0;
                }
                LedCommand::HostCommand(msg) => match msg {
                    HostToPico::FillAll { r, g, b, brightness } => {
                        mode = LedMode::Manual;
                        set_all(&mut colors, apply_brightness(r, g, b, brightness));
                        if !is_suspended { ws2812.write(&colors).await; }
                    }
                    HostToPico::SetLed { index, r, g, b, brightness } => {
                        mode = LedMode::Manual;
                        if let Some(led) = colors.get_mut(index as usize) {
                            *led = apply_brightness(r, g, b, brightness);
                            if !is_suspended { ws2812.write(&colors).await; }
                        }
                    }
                    HostToPico::SetEffect { effect } => {
                        mode = LedMode::Effect(effect);
                        frame = 0;
                        render_effect(effect, frame, &mut noise_seed, &mut colors);
                        if !is_suspended { ws2812.write(&colors).await; }
                    }
                    _ => {}
                }
            },
            Err(_) => {
                if !is_suspended {
                    if let LedMode::Effect(effect) = mode {
                        frame = frame.wrapping_add(1);
                        render_effect(effect, frame, &mut noise_seed, &mut colors);
                        ws2812.write(&colors).await;
                    }
                }
            }
        }
    }
}

fn set_all(colors: &mut [RGB8; NUM_LEDS], color: RGB8) {
    for led in colors.iter_mut() {
        *led = color;
    }
}

fn apply_brightness(r: u8, g: u8, b: u8, brightness: u8) -> RGB8 {
    RGB8 {
        r: scale(r, brightness),
        g: scale(g, brightness),
        b: scale(b, brightness),
    }
}

fn scale(component: u8, brightness: u8) -> u8 {
    ((component as u16 * brightness as u16) / 255) as u8
}

fn render_effect(effect: LedEffect, frame: u32, seed: &mut u32, colors: &mut [RGB8; NUM_LEDS]) {
    match effect {
        LedEffect::Solid { r, g, b, brightness } => {
            set_all(colors, apply_brightness(r, g, b, brightness));
        }
        LedEffect::Blink { r, g, b, brightness, speed } => {
            let color = apply_brightness(r, g, b, brightness);
            let period = blink_period_frames(speed);
            if (frame / period) % 2 == 0 {
                set_all(colors, color);
            } else {
                set_all(colors, RGB8::default());
            }
        }
        LedEffect::Rainbow { brightness, speed, saturation, reverse } => {
            let step = speed_step(speed);
            let base_hue = (frame.wrapping_mul(step)) as u8;
            for i in 0..NUM_LEDS {
                let virtual_idx = if !reverse { NUM_LEDS - 1 - i } else { i };
                let hue = base_hue.wrapping_add(((virtual_idx * 256) / NUM_LEDS) as u8);
                let raw = hsv_to_rgb(hue, saturation, 255);
                colors[i] = apply_brightness(raw.r, raw.g, raw.b, brightness);
            }
        }
        LedEffect::Breathing { r, g, b, brightness, speed } => {
            let phase = ((frame.wrapping_mul(speed_step(speed) * 2)) % 512) as u16;
            let value = if phase < 256 { phase as u8 } else { (511 - phase) as u8 };
            let hue = rgb_to_hue_approx(r, g, b);
            let sat = rgb_to_sat_approx(r, g, b);
            let raw = hsv_to_rgb(hue, sat, value);
            set_all(colors, apply_brightness(raw.r, raw.g, raw.b, brightness));
        }
        LedEffect::Chase { r, g, b, brightness, speed, size, reverse } => {
            set_all(colors, RGB8::default());
            let speed_factor = (speed as u32 * 10) + 10;
            let total_pos = (frame.wrapping_mul(speed_factor)) >> 4;
            let num_leds_f = (NUM_LEDS << 8) as u32;
            let head_f = total_pos % num_leds_f;
            let block_size = size.max(1) as i32;

            for i in 0..NUM_LEDS {
                let led_pos_f = (i << 8) as i32;
                let head_pos_f = head_f as i32;
                let mut dist = led_pos_f - head_pos_f;
                if dist < -(num_leds_f as i32 / 2) { dist += num_leds_f as i32; }
                if dist > (num_leds_f as i32 / 2) { dist -= num_leds_f as i32; }

                let intensity = if dist >= 0 && dist < (block_size << 8) { 255 }
                else if dist < 0 && dist > -256 { (256 + dist) as u8 }
                else if dist >= (block_size << 8) && dist < ((block_size + 1) << 8) { (256 - (dist - (block_size << 8))) as u8 }
                else { 0 };

                if intensity > 0 {
                    let target_idx = if reverse { NUM_LEDS - 1 - i } else { i };
                    colors[target_idx] = apply_brightness(scale(r, intensity), scale(g, intensity), scale(b, intensity), brightness);
                }
            }
        }
        LedEffect::Comet { r, g, b, brightness, speed, tail, reverse } => {
            set_all(colors, RGB8::default());
            let speed_factor = (speed as u32 * 10) + 20;
            let total_pos = (frame.wrapping_mul(speed_factor)) >> 4;
            let num_leds_f = (NUM_LEDS << 8) as i32;
            let head_f = (total_pos % num_leds_f as u32) as i32;
            let tail_len_f = (tail.max(1) as i32) << 8;

            for i in 0..NUM_LEDS {
                let led_pos_f = (i << 8) as i32;
                let mut dist = head_f - led_pos_f;
                if dist < -(num_leds_f / 2) { dist += num_leds_f; }
                if dist > (num_leds_f / 2) { dist -= num_leds_f; }
                let target_idx = if reverse { NUM_LEDS - 1 - i } else { i };

                if dist >= 0 && dist <= tail_len_f {
                    let intensity = 255 - ((dist * 255) / tail_len_f) as u8;
                    colors[target_idx] = apply_brightness(r, g, b, scale(brightness, intensity));
                } else if dist < 0 && dist > -256 {
                    let intensity = (256 + dist) as u8;
                    colors[target_idx] = apply_brightness(r, g, b, scale(brightness, intensity));
                }
            }
        }
        LedEffect::Sparkle { r, g, b, brightness, speed, density } => {
            let color = apply_brightness(r, g, b, brightness);
            let fade = 5 + ((speed as u16 * 40) / 255) as u8;
            fade_colors(colors, fade);
            let sparks = 1 + ((density as usize * (NUM_LEDS - 1)) / 255);
            for _ in 0..sparks {
                let pos = (next_random(seed) as usize) % NUM_LEDS;
                colors[pos] = color;
            }
        }
        LedEffect::Aurora { brightness, speed, reverse } => {
            let shift = ((frame.wrapping_mul(speed_step(speed))) & 0xff) as u8;
            for i in 0..NUM_LEDS {
                let virtual_idx = if !reverse { NUM_LEDS - 1 - i } else { i };
                let hue = shift.wrapping_add((virtual_idx as u8).wrapping_mul(17));
                let wave = tri_wave(frame as u8, speed, virtual_idx as u8);
                let sat = 180u8.saturating_add(wave / 3);
                let val = 40u8.saturating_add((wave as u16 * 215 / 255) as u8);
                let raw = hsv_to_rgb(hue, sat, val);
                colors[i] = apply_brightness(raw.r, raw.g, raw.b, brightness);
            }
        }
        LedEffect::ColorOrbit { hue, hue_shift, saturation, brightness, speed, reverse } => {
            let period = orbit_period_frames(speed);
            let rot = (frame / period) as u8;
            for i in 0..NUM_LEDS {
                let virtual_idx = if !reverse { NUM_LEDS - 1 - i } else { i };
                let offset = ((virtual_idx * 256) / NUM_LEDS) as u8;
                let phase = rot.wrapping_add(offset);
                let mix = smooth_wave8(phase);
                let current_hue = hue.wrapping_add(scale(hue_shift, mix));
                let raw = hsv_to_rgb(current_hue, saturation, 255);
                colors[i] = apply_brightness(raw.r, raw.g, raw.b, brightness);
            }
        }
        LedEffect::Astolfo { brightness, speed, saturation, spread, reverse } => {
            let period = orbit_period_frames(speed);
            let rot = ((frame.wrapping_mul(3)) / period) as u16;
            let phase_span = astolfo_phase_span(spread);
            for i in 0..NUM_LEDS {
                let virtual_idx = if !reverse { NUM_LEDS - 1 - i } else { i };
                let offset = ((virtual_idx as u16 * phase_span) / NUM_LEDS as u16) as u16;
                let phase = rot.wrapping_add(offset) as u8;
                let mix = smooth_wave8(phase);
                let hue = lerp8(236, 150, mix);
                let pulse = smooth_wave8(phase.wrapping_add(rot as u8));
                let value = 90u8.saturating_add(scale(165, pulse));
                let raw = hsv_to_rgb(hue, saturation, value);
                colors[i] = apply_brightness(raw.r, raw.g, raw.b, brightness);
            }
        }
    }
}

fn blink_period_frames(speed: u8) -> u32 { 6 + (((255 - speed) as u32 * 54) / 255) }
fn speed_step(speed: u8) -> u32 { 1 + ((speed as u32 * 7) / 255) }
fn orbit_period_frames(speed: u8) -> u32 { 1 + (((255 - speed) as u32 * 18) / 255) }
fn astolfo_phase_span(spread: u8) -> u16 { 64 + ((spread as u32 * 320) / 255) as u16 }

fn hsv_to_rgb(h: u8, s: u8, v: u8) -> RGB8 {
    if s == 0 { return RGB8 { r: v, g: v, b: v }; }
    let region = h / 43;
    let remainder = ((h as u16 - (region as u16 * 43)) * 6) as u8;
    let p = ((v as u16 * (255 - s) as u16) / 255) as u8;
    let q = ((v as u16 * (255 - ((s as u16 * remainder as u16) / 255)) as u16) / 255) as u8;
    let t = ((v as u16 * (255 - ((s as u16 * (255 - remainder as u16)) / 255)) as u16) / 255) as u8;
    match region {
        0 => RGB8 { r: v, g: t, b: p },
        1 => RGB8 { r: q, g: v, b: p },
        2 => RGB8 { r: p, g: v, b: t },
        3 => RGB8 { r: p, g: q, b: v },
        4 => RGB8 { r: t, g: p, b: v },
        _ => RGB8 { r: v, g: p, b: q },
    }
}

fn rgb_to_hue_approx(r: u8, g: u8, b: u8) -> u8 {
    if r >= g && r >= b { 0 } else if g >= r && g >= b { 85 } else { 170 }
}

fn rgb_to_sat_approx(r: u8, g: u8, b: u8) -> u8 {
    if r == g && g == b { 0 } else { 255 }
}

fn fade_colors(colors: &mut [RGB8; NUM_LEDS], amount: u8) {
    for led in colors.iter_mut() {
        led.r = led.r.saturating_sub(amount);
        led.g = led.g.saturating_sub(amount);
        led.b = led.b.saturating_sub(amount);
    }
}

fn next_random(seed: &mut u32) -> u32 {
    let mut x = *seed;
    x ^= x << 13; x ^= x >> 17; x ^= x << 5;
    *seed = x; x
}

fn tri_wave(frame: u8, speed: u8, offset: u8) -> u8 {
    let step = 1 + (speed / 32);
    let phase = frame.wrapping_mul(step).wrapping_add(offset.wrapping_mul(11));
    let p = (phase as u16) & 0x01ff;
    if p < 256 { p as u8 } else { (511 - p) as u8 }
}

fn smooth_wave8(phase: u8) -> u8 {
    let tri = if phase < 128 { phase.saturating_mul(2) } else { (255 - phase).saturating_mul(2) };
    smoothstep8(tri)
}

fn smoothstep8(x: u8) -> u8 {
    let x = x as u32;
    ((x * x * (765 - 2 * x)) / 65025) as u8
}

fn lerp8(a: u8, b: u8, t: u8) -> u8 {
    let a = a as i16; let b = b as i16; let t = t as i16;
    (a + (((b - a) * t) / 255)) as u8
}