use embassy_rp::i2c::{Blocking, I2c};
use embassy_rp::peripherals::I2C0;
use embassy_time::Duration;

const DISPLAY_WIDTH: usize = 128;
const DISPLAY_HEIGHT: usize = 64;
const DISPLAY_PAGES: usize = DISPLAY_HEIGHT / 8;
const FRAME_SIZE: usize = DISPLAY_WIDTH * DISPLAY_PAGES;
// SH1106 commonly maps the visible 128 columns starting at RAM column 2.
const COLUMN_OFFSET: u8 = 2;
const FRAME_TIME: Duration = Duration::from_millis(33);

const ICON_MASTER: [&str; 14] = [
    "              ",
    "       11     ",
    "      111     ",
    "     1111   1 ",
    "   111111   11",
    "   111111  111",
    "   111111  111",
    "   111111  111",
    "   111111  111",
    "   111111   11",
    "     1111   1 ",
    "      111     ",
    "       11     ",
    "              ",
];

const ICON_SPOTIFY: [&str; 14] = [
    "            ",
    "    111111   ",
    "  1111111111 ",
    " 111111111111",
    " 11        11",
    "11111111111111",
    "1111      1111",
    "11111111111111",
    " 1111    1111",
    " 111111111111",
    "  1111111111 ",
    "    111111   ",
    "            ",
    "            ",
];

const ICON_DISCORD: [&str; 14] = [
    "            ",
    "            ",
    "            ",
    "   11  11   ",
    "  11111111  ",
    " 1111111111 ",
    " 11 1111 11 ",
    " 11 1111 11 ",
    " 1111111111 ",
    "  111  111  ",
    "   11  11   ",
    "            ",
    "            ",
    "            ",
];

const ICON_BROWSER: [&str; 14] = [
    "              ",
    "     11111    ",
    "   111   111  ",
    "  111     111 ",
    " 111       111",
    " 111   1111111",
    " 111   1111111",
    " 111          ",
    " 111          ",
    "  111     11  ",
    "   111111111  ",
    "     11111    ",
    "              ",
    "              ",
];

const VOLUMES: [u8; 4] = [50, 65, 80, 35];

#[embassy_executor::task]
pub async fn display_demo_task(mut i2c: I2c<'static, I2C0, Blocking>) {
    let addr = probe_addr(&mut i2c).unwrap_or(0x3c);
    init_sh1106(&mut i2c, addr);

    let mut frame = [0u8; FRAME_SIZE];
    let _ = FRAME_TIME; // keep constant available for later animated mode

    // Static screen to keep input latency low.
    render_mockup(&mut frame, 0, false);
    let _ = write_frame(&mut i2c, addr, &frame);
}

fn render_mockup(frame: &mut [u8; FRAME_SIZE], selected_idx: usize, simple_mode: bool) {
    let (bg, fg, profile) = if simple_mode {
        (true, false, "PROFIL: BASIC")
    } else {
        (false, true, "PROFIL: MAIN")
    };

    fill(frame, bg);

    draw_text_centered(frame, 1, profile, fg);
    draw_dashed_hline(frame, 16, 0, DISPLAY_WIDTH - 1, 2, fg);

    let segment_width = DISPLAY_WIDTH / 4;
    let icons = [ICON_MASTER, ICON_SPOTIFY, ICON_DISCORD, ICON_BROWSER];

    for i in 0..4 {
        let x_start = i * segment_width;
        let x_center = x_start + (segment_width / 2);

        if i > 0 {
            draw_dashed_vline(frame, x_start, 20, DISPLAY_HEIGHT - 1, 1, 2, fg);
        }

        let mut icon_color = fg;
        let mut text_color = fg;
        if i == selected_idx {
            draw_filled_rect(frame, x_start + 2, 19, segment_width - 3, 44, fg);
            icon_color = bg;
            text_color = bg;
        }

        draw_icon(frame, x_center.saturating_sub(6), 22, &icons[i], icon_color);

        let mut vol = [0u8; 4];
        let vol_len = volume_to_ascii(VOLUMES[i], &mut vol);
        draw_text_centered_in_range(
            frame,
            6,
            &vol[..vol_len],
            x_start,
            x_start + segment_width - 1,
            text_color,
        );
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

fn draw_filled_rect(frame: &mut [u8; FRAME_SIZE], x: usize, y: usize, w: usize, h: usize, on: bool) {
    if w == 0 || h == 0 {
        return;
    }
    for yy in y..(y + h) {
        for xx in x..(x + w) {
            put_pixel(frame, xx, yy, on);
        }
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
        let glyph = font_5x7(ch);
        for (dx, bits) in glyph.iter().enumerate() {
            for dy in 0..7 {
                let pixel_on = (bits >> dy) & 1 != 0;
                if pixel_on {
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

fn probe_addr(i2c: &mut I2c<'_, I2C0, Blocking>) -> Option<u16> {
    for addr in [0x3c_u16, 0x3d_u16] {
        if i2c.blocking_write(addr, &[0x00]).is_ok() {
            return Some(addr);
        }
    }
    None
}

fn init_sh1106(i2c: &mut I2c<'_, I2C0, Blocking>, addr: u16) {
    let _ = write_cmd(i2c, addr, 0xae);
    let _ = write_cmd2(i2c, addr, 0xd5, 0x80);
    let _ = write_cmd2(i2c, addr, 0xa8, 0x3f);
    let _ = write_cmd2(i2c, addr, 0xd3, 0x00);
    let _ = write_cmd(i2c, addr, 0x40);
    let _ = write_cmd2(i2c, addr, 0x8d, 0x14);
    let _ = write_cmd2(i2c, addr, 0x20, 0x02); // Page addressing mode
    let _ = write_cmd(i2c, addr, 0xa1);
    let _ = write_cmd(i2c, addr, 0xc8);
    let _ = write_cmd2(i2c, addr, 0xda, 0x12);
    let _ = write_cmd2(i2c, addr, 0x81, 0x05);
    let _ = write_cmd2(i2c, addr, 0xd9, 0xf1);
    let _ = write_cmd2(i2c, addr, 0xdb, 0x40);
    let _ = write_cmd(i2c, addr, 0xa4);
    let _ = write_cmd(i2c, addr, 0xa6);
    let _ = write_cmd(i2c, addr, 0xaf);
}

fn write_frame(
    i2c: &mut I2c<'_, I2C0, Blocking>,
    addr: u16,
    frame: &[u8; FRAME_SIZE],
) -> Result<(), embassy_rp::i2c::Error> {
    let mut payload = [0u8; DISPLAY_WIDTH + 1];
    payload[0] = 0x40;

    for page in 0..DISPLAY_PAGES {
        let page_cmd = 0xb0 | (page as u8);
        let col = COLUMN_OFFSET;
        let col_low = col & 0x0f;
        let col_high = 0x10 | ((col >> 4) & 0x0f);

        write_cmd(i2c, addr, page_cmd)?;
        write_cmd(i2c, addr, col_low)?;
        write_cmd(i2c, addr, col_high)?;

        let start = page * DISPLAY_WIDTH;
        let end = start + DISPLAY_WIDTH;
        payload[1..].copy_from_slice(&frame[start..end]);
        i2c.blocking_write(addr, &payload)?;
    }
    Ok(())
}

fn write_cmd(i2c: &mut I2c<'_, I2C0, Blocking>, addr: u16, cmd: u8) -> Result<(), embassy_rp::i2c::Error> {
    i2c.blocking_write(addr, &[0x00, cmd])
}

fn write_cmd2(
    i2c: &mut I2c<'_, I2C0, Blocking>,
    addr: u16,
    cmd: u8,
    value: u8,
) -> Result<(), embassy_rp::i2c::Error> {
    i2c.blocking_write(addr, &[0x00, cmd, value])
}

fn font_5x7(c: u8) -> [u8; 5] {
    match c {
        b' ' => [0x00, 0x00, 0x00, 0x00, 0x00],
        b'%' => [0x23, 0x13, 0x08, 0x64, 0x62],
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
        b':' => [0x00, 0x36, 0x36, 0x00, 0x00],
        b'A' => [0x7e, 0x11, 0x11, 0x11, 0x7e],
        b'B' => [0x7f, 0x49, 0x49, 0x49, 0x36],
        b'C' => [0x3e, 0x41, 0x41, 0x41, 0x22],
        b'F' => [0x7f, 0x09, 0x09, 0x09, 0x01],
        b'I' => [0x00, 0x41, 0x7f, 0x41, 0x00],
        b'L' => [0x7f, 0x40, 0x40, 0x40, 0x40],
        b'M' => [0x7f, 0x02, 0x04, 0x02, 0x7f],
        b'N' => [0x7f, 0x04, 0x08, 0x10, 0x7f],
        b'O' => [0x3e, 0x41, 0x41, 0x41, 0x3e],
        b'P' => [0x7f, 0x09, 0x09, 0x09, 0x06],
        b'R' => [0x7f, 0x09, 0x19, 0x29, 0x46],
        b'S' => [0x46, 0x49, 0x49, 0x49, 0x31],
        b'K' => [0x7f, 0x08, 0x14, 0x22, 0x41],
        _ => [0x00, 0x00, 0x5f, 0x00, 0x00],
    }
}
