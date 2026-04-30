use embassy_rp::i2c::{Async, I2c};
use embassy_rp::peripherals::I2C0;
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use crate::icons::{ICON_ACTIVE_WINDOW, ICON_BROWSER, ICON_CAMERA, ICON_DISCORD, ICON_LIGHT, ICON_MASTER, ICON_MIC, ICON_PLAY_PAUSE, ICON_SPOTIFY};
use crate::protocol::IconType;
use embedded_hal_async::i2c::I2c as _;

pub enum DisplayCommand {
    Suspend,
    Resume,
    UpdateVolume { slot: u8, volume: u8 },
    UpdateIcon { slot: u8, icon: IconType },
    UpdateMute { slot: u8, muted: bool },
    SetProfileName(&'static str),
    ForceRedraw
}

pub static DISPLAY_COMMAND_CHANNEL: Channel<ThreadModeRawMutex, DisplayCommand, 16> = Channel::new();

const DISPLAY_WIDTH: usize = 128;
const DISPLAY_HEIGHT: usize = 64;
const DISPLAY_PAGES: usize = DISPLAY_HEIGHT / 8;
const FRAME_SIZE: usize = DISPLAY_WIDTH * DISPLAY_PAGES;
const COLUMN_OFFSET: u8 = 2;

#[embassy_executor::task]
pub async fn display_task(mut i2c: I2c<'static, I2C0, Async>) {
    let addr = probe_addr(&mut i2c).await.unwrap_or(0x3c);
    init_sh1106(&mut i2c, addr).await;

    let mut frame = [0u8; FRAME_SIZE];
    let mut state = crate::state::DisplayState::default();

    // Initiales Zeichnen des Bildschirms
    render_screen(&mut frame, &state);
    let _ = write_frame(&mut i2c, addr, &frame).await;

    // BUGFIX: is_suspended wird VOR der Schleife deklariert!
    let mut is_suspended = false;

    loop {
        let mut cmd = DISPLAY_COMMAND_CHANNEL.receive().await;

        loop {
            // State updaten (OHNE direkt zu rendern!)
            match cmd {
                DisplayCommand::UpdateVolume { slot, volume } => {
                    if slot < 4 { state.slots[slot as usize].volume = volume; }
                }
                DisplayCommand::UpdateIcon { slot, icon } => {
                    if slot < 4 { state.slots[slot as usize].icon = icon; }
                }
                DisplayCommand::UpdateMute { slot, muted } => {
                    if slot < 4 { state.slots[slot as usize].muted = muted; }
                }
                DisplayCommand::SetProfileName(name) => {
                    state.profile_name = name;
                }
                DisplayCommand::Suspend => {
                    is_suspended = true;
                }
                DisplayCommand::Resume => {
                    is_suspended = false;
                }
                DisplayCommand::ForceRedraw => {}
            }

            match DISPLAY_COMMAND_CHANNEL.try_receive() {
                Ok(next_cmd) => {
                    cmd = next_cmd;
                }
                Err(_) => {
                    break;
                }
            }
        }

        if is_suspended {
            fill(&mut frame, false);
        } else {
            render_screen(&mut frame, &state);
        }

        let _ = write_frame(&mut i2c, addr, &frame).await;
    }
}

fn render_screen(frame: &mut [u8; FRAME_SIZE], state: &crate::state::DisplayState) {
    // 1. Framebuffer leeren (Hintergrund schwarz)
    fill(frame, false);

    if crate::keyboard::KeyboardMapper::is_active() {
        draw_text_large_centered(frame, "SIMPLE MODE", true);
        return;
    }

    draw_text_centered(frame, 0, state.profile_name, true);

    // 2. Eine horizontale Trennlinie ziehen (unter dem leeren Header-Bereich)
    draw_dashed_hline(frame, 10, 0, DISPLAY_WIDTH - 1, 2, true);

    let segment_width = DISPLAY_WIDTH / 4;

    for i in 0..4 {
        let x_start = i * segment_width;
        let slot = &state.slots[i];

        if i > 0 {
            draw_dashed_vline(frame, x_start, 15, DISPLAY_HEIGHT - 1, 1, 2, true);
        }

        // 4. Icon auswählen und zeichnen
        let icon_data = match slot.icon {
            IconType::Master => &ICON_MASTER,
            IconType::Spotify => &ICON_SPOTIFY,
            IconType::Discord => &ICON_DISCORD,
            IconType::Browser => &ICON_BROWSER,
            IconType::Mic => &ICON_MIC,
            IconType::Camera => &ICON_CAMERA,
            IconType::PlayPause => &ICON_PLAY_PAUSE,
            IconType::Light => &ICON_LIGHT,
            IconType::ActiveWindow => &ICON_ACTIVE_WINDOW,
            IconType::None => &["              "; 14],
        };


        let icon_width = icon_data[0].len();
        let icon_x = x_start + (segment_width - icon_width) / 2;
        let icon_y = 20;

        // 3. Zeichnen
        if i > 0 {
            draw_dashed_vline(frame, x_start, 15, DISPLAY_HEIGHT - 1, 1, 2, true);
        }
        draw_icon(frame, icon_x, icon_y, icon_data, true);

        // 5. Mute-X zeichnen (falls gemutet)
        if slot.muted {
            // Wir zeichnen ein massives X über das 14x14 Icon-Feld
            for d in 0..14 {
                // --- Diagonale von oben-links nach unten-rechts (\) ---
                put_pixel(frame, icon_x + d, icon_y + d, true); // Die mittlere Linie
                if d > 0 {
                    put_pixel(frame, icon_x + d - 1, icon_y + d, true); // Pixel links daneben
                }
                if d < 13 {
                    put_pixel(frame, icon_x + d + 1, icon_y + d, true); // Pixel rechts daneben
                }

                // --- Diagonale von oben-rechts nach unten-links (/) ---
                put_pixel(frame, icon_x + 13 - d, icon_y + d, true); // Die mittlere Linie
                if d > 0 {
                    put_pixel(frame, icon_x + 13 - d + 1, icon_y + d, true); // Pixel rechts daneben
                }
                if d < 13 {
                    put_pixel(frame, icon_x + 13 - d - 1, icon_y + d, true); // Pixel links daneben
                }
            }
        }

        // 6. Lautstärketext oder "---" zeichnen
        let mut vol_buf = [0u8; 4];
        if slot.volume == 255 {
            // Platzhalter, wenn keine Daten vom PC vorliegen
            draw_text_centered_in_range(
                frame,
                6, // Page 6 (unterer Bereich)
                b"---",
                x_start,
                x_start + segment_width - 1,
                true,
            );
        } else {
            // Normalen Prozentwert umwandeln und anzeigen
            let vol_len = volume_to_ascii(slot.volume, &mut vol_buf);
            draw_text_centered_in_range(
                frame,
                6,
                &vol_buf[..vol_len],
                x_start,
                x_start + segment_width - 1,
                true,
            );
        }
    }
}

fn fill(frame: &mut [u8; FRAME_SIZE], on: bool) {
    frame.fill(if on { 0xff } else { 0x00 });
}

fn draw_icon(frame: &mut [u8; FRAME_SIZE], x: usize, y: usize, icon: &[&str; 14], on: bool) {
    for (row, line) in icon.iter().enumerate() {
        for (col, b) in line.as_bytes().iter().enumerate() {
            if *b == b'1' {
                put_pixel(frame, x + col, y + row, on);
            }
        }
    }
}

fn draw_dashed_hline(
    frame: &mut [u8; FRAME_SIZE],
    y: usize,
    x_start: usize,
    x_end: usize,
    dash_len: usize,
    on: bool,
) {
    let mut x = x_start;
    while x <= x_end {
        for d in 0..dash_len {
            let xx = x + d;
            if xx <= x_end {
                put_pixel(frame, xx, y, on);
            }
        }
        x += dash_len * 2;
    }
}

fn draw_dashed_vline(
    frame: &mut [u8; FRAME_SIZE],
    x: usize,
    y_start: usize,
    y_end: usize,
    dash_len: usize,
    gap_len: usize,
    on: bool,
) {
    let mut y = y_start;
    while y <= y_end {
        for d in 0..dash_len {
            let yy = y + d;
            if yy <= y_end {
                put_pixel(frame, x, yy, on);
            }
        }
        y += dash_len + gap_len;
    }
}

fn draw_text_centered(frame: &mut [u8; FRAME_SIZE], page: usize, text: &str, on: bool) {
    draw_text_centered_in_range(frame, page, text.as_bytes(), 0, DISPLAY_WIDTH - 1, on);
}

fn draw_text_centered_in_range(
    frame: &mut [u8; FRAME_SIZE],
    page: usize,
    text: &[u8],
    x_min: usize,
    x_max: usize,
    on: bool,
) {
    let glyph_w = 6;
    let text_w = text.len() * glyph_w;
    let range_w = x_max.saturating_sub(x_min) + 1;
    let start_x = x_min + range_w.saturating_sub(text_w) / 2;
    draw_text(frame, page, start_x, text, on);
}

fn draw_text(frame: &mut [u8; FRAME_SIZE], page: usize, col: usize, text: &[u8], on: bool) {
    if page >= DISPLAY_PAGES || col >= DISPLAY_WIDTH {
        return;
    }
    let mut cursor = col;
    for &ch in text {
        if cursor + 6 > DISPLAY_WIDTH {
            break;
        }
        let glyph = font_5x7(ch.to_ascii_uppercase());
        for (dx, bits) in glyph.iter().enumerate() {
            for dy in 0..7 {
                if (bits >> dy) & 1 != 0 {
                    put_pixel(frame, cursor + dx, page * 8 + dy, on);
                }
            }
        }
        cursor += 6;
    }
}

fn volume_to_ascii(volume: u8, out: &mut [u8; 4]) -> usize {
    let h = volume / 100;
    let t = (volume % 100) / 10;
    let o = volume % 10;
    let mut idx = 0;
    if h > 0 {
        out[idx] = b'0' + h;
        idx += 1;
    }
    if idx > 0 || t > 0 {
        out[idx] = b'0' + t;
        idx += 1;
    }
    out[idx] = b'0' + o;
    idx += 1;
    out[idx] = b'%';
    idx += 1;
    idx
}

fn put_pixel(frame: &mut [u8; FRAME_SIZE], x: usize, y: usize, on: bool) {
    if x >= DISPLAY_WIDTH || y >= DISPLAY_HEIGHT {
        return;
    }
    let page = y / 8;
    let bit = y % 8;
    let idx = page * DISPLAY_WIDTH + x;
    let mask = 1u8 << bit;
    if on {
        frame[idx] |= mask;
    } else {
        frame[idx] &= !mask;
    }
}

async fn probe_addr(i2c: &mut I2c<'_, I2C0, Async>) -> Option<u16> {
    for addr in [0x3c_u16, 0x3d_u16] {
        if i2c.write(addr, &[0x00]).await.is_ok() {
            return Some(addr);
        }
    }
    None
}

async fn init_sh1106(i2c: &mut I2c<'_, I2C0, Async>, addr: u16) {
    let _ = write_cmd(i2c, addr, 0xae).await;
    let _ = write_cmd2(i2c, addr, 0xd5, 0x80).await;
    let _ = write_cmd2(i2c, addr, 0xa8, 0x3f).await;
    let _ = write_cmd2(i2c, addr, 0xd3, 0x00).await;
    let _ = write_cmd(i2c, addr, 0x40).await;
    let _ = write_cmd2(i2c, addr, 0x8d, 0x14).await;
    let _ = write_cmd2(i2c, addr, 0x20, 0x02).await;
    let _ = write_cmd(i2c, addr, 0xa1).await;
    let _ = write_cmd(i2c, addr, 0xc8).await;
    let _ = write_cmd2(i2c, addr, 0xda, 0x12).await;
    let _ = write_cmd2(i2c, addr, 0x81, 0x05).await;
    let _ = write_cmd2(i2c, addr, 0xd9, 0xf1).await;
    let _ = write_cmd2(i2c, addr, 0xdb, 0x40).await;
    let _ = write_cmd(i2c, addr, 0xa4).await;
    let _ = write_cmd(i2c, addr, 0xa6).await;
    let _ = write_cmd(i2c, addr, 0xaf).await;
}

async fn write_frame(
    i2c: &mut I2c<'_, I2C0, Async>,
    addr: u16,
    frame: &[u8; FRAME_SIZE],
) -> Result<(), embassy_rp::i2c::Error> {
    let mut payload = [0u8; DISPLAY_WIDTH + 1];
    payload[0] = 0x40;
    for page in 0..DISPLAY_PAGES {
        let col = COLUMN_OFFSET;
        write_cmd(i2c, addr, 0xb0 | (page as u8)).await?;
        write_cmd(i2c, addr, col & 0x0f).await?;
        write_cmd(i2c, addr, 0x10 | ((col >> 4) & 0x0f)).await?;
        let start = page * DISPLAY_WIDTH;
        payload[1..].copy_from_slice(&frame[start..start + DISPLAY_WIDTH]);
        i2c.write(addr, &payload).await?;
    }
    Ok(())
}

async fn write_cmd(
    i2c: &mut I2c<'_, I2C0, Async>,
    addr: u16,
    cmd: u8,
) -> Result<(), embassy_rp::i2c::Error> {
    i2c.write(addr, &[0x00, cmd]).await
}

async fn write_cmd2(
    i2c: &mut I2c<'_, I2C0, Async>,
    addr: u16,
    cmd: u8,
    val: u8,
) -> Result<(), embassy_rp::i2c::Error> {
    i2c.write(addr, &[0x00, cmd, val]).await
}

fn font_5x7(c: u8) -> [u8; 5] {
    match c {
        b' ' => [0x00, 0x00, 0x00, 0x00, 0x00],
        b'%' => [0x23, 0x13, 0x08, 0x64, 0x62],
        b'-' => [0x08, 0x08, 0x08, 0x08, 0x08],
        b':' => [0x00, 0x00, 0x24, 0x00, 0x00],

        b'0' => [0x3e, 0x51, 0x49, 0x45, 0x3e],
        b'1' => [0x00, 0x42, 0x7f, 0x40, 0x00],
        b'2' => [0x42, 0x61, 0x51, 0x49, 0x46],
        b'3' => [0x21, 0x41, 0x45, 0x4b, 0x31],
        b'4' => [0x18, 0x14, 0x12, 0x7f, 0x10],
        b'5' => [0x27, 0x45, 0x45, 0x45, 0x39],
        b'6' => [0x3c, 0x4a, 0x49, 0x49, 0x30],
        b'7' => [0x01, 0x71, 0x09, 0x05, 0x03],
        b'8' => [0x36, 0x49, 0x49, 0x49, 0x36],
        b'9' => [0x06, 0x49, 0x49, 0x29, 0x1e],

        b'A' => [0x7e, 0x11, 0x11, 0x11, 0x7e],
        b'B' => [0x7f, 0x49, 0x49, 0x49, 0x36],
        b'C' => [0x3e, 0x41, 0x41, 0x41, 0x22],
        b'D' => [0x7f, 0x41, 0x41, 0x22, 0x1c],
        b'E' => [0x7f, 0x49, 0x49, 0x49, 0x41],
        b'F' => [0x7f, 0x09, 0x09, 0x09, 0x01],
        b'G' => [0x3e, 0x41, 0x49, 0x49, 0x7a],
        b'H' => [0x7f, 0x08, 0x08, 0x08, 0x7f],
        b'I' => [0x00, 0x41, 0x7f, 0x41, 0x00],
        b'J' => [0x20, 0x40, 0x41, 0x3f, 0x01],
        b'K' => [0x7f, 0x08, 0x14, 0x22, 0x41],
        b'L' => [0x7f, 0x40, 0x40, 0x40, 0x40],
        b'M' => [0x7f, 0x02, 0x04, 0x02, 0x7f],
        b'N' => [0x7f, 0x04, 0x08, 0x10, 0x7f],
        b'O' => [0x3e, 0x41, 0x41, 0x41, 0x3e],
        b'P' => [0x7f, 0x09, 0x09, 0x09, 0x06],
        b'Q' => [0x3e, 0x41, 0x51, 0x21, 0x5e],
        b'R' => [0x7f, 0x09, 0x19, 0x29, 0x46],
        b'S' => [0x46, 0x49, 0x49, 0x49, 0x31],
        b'T' => [0x01, 0x01, 0x7f, 0x01, 0x01],
        b'U' => [0x3f, 0x40, 0x40, 0x40, 0x3f],
        b'V' => [0x1f, 0x20, 0x40, 0x20, 0x1f],
        b'W' => [0x3f, 0x40, 0x38, 0x40, 0x3f],
        b'X' => [0x63, 0x14, 0x08, 0x14, 0x63],
        b'Y' => [0x03, 0x04, 0x78, 0x04, 0x03],
        b'Z' => [0x61, 0x51, 0x49, 0x45, 0x43],

        _ => [0x00, 0x00, 0x5f, 0x00, 0x00], // Fallback für alle anderen Zeichen
    }
}

fn draw_text_large_centered(frame: &mut [u8; FRAME_SIZE], text: &str, on: bool) {
    let scale = 2; // 2x2 Pixel pro originalem Pixel
    let glyph_w = 6 * scale; // Ein Buchstabe ist jetzt 12 Pixel breit
    let text_w = text.len() * glyph_w;

    // Berechne die absolute Mitte des Displays
    let start_x = (DISPLAY_WIDTH.saturating_sub(text_w)) / 2;
    let start_y = (DISPLAY_HEIGHT.saturating_sub(7 * scale)) / 2;

    let mut cursor = start_x;
    for &ch in text.as_bytes() {
        if cursor + glyph_w > DISPLAY_WIDTH {
            break;
        }
        let glyph = font_5x7(ch.to_ascii_uppercase());

        // Jeden Pixel verdoppeln
        for (dx, bits) in glyph.iter().enumerate() {
            for dy in 0..7 {
                if (bits >> dy) & 1 != 0 {
                    // Zeichne einen 2x2 Block für jeden Pixel
                    for sx in 0..scale {
                        for sy in 0..scale {
                            put_pixel(frame, cursor + dx * scale + sx, start_y + dy * scale + sy, on);
                        }
                    }
                }
            }
        }
        cursor += glyph_w;
    }
}