use embassy_rp::i2c::{Blocking, I2c};
use embassy_rp::peripherals::I2C0;
use embassy_time::{Duration, Timer};

const DISPLAY_WIDTH: usize = 128;
const DISPLAY_HEIGHT: usize = 64;
const DISPLAY_PAGES: usize = DISPLAY_HEIGHT / 8;
const FRAME_SIZE: usize = DISPLAY_WIDTH * DISPLAY_PAGES;
// SH1106 commonly maps the visible 128 columns starting at RAM column 2.
const COLUMN_OFFSET: u8 = 2;
const FRAME_TIME: Duration = Duration::from_millis(33);

#[embassy_executor::task]
pub async fn display_demo_task(mut i2c: I2c<'static, I2C0, Blocking>) {
    let addr = probe_addr(&mut i2c).unwrap_or(0x3c);
    init_sh1106(&mut i2c, addr);

    let mut frame = [0u8; FRAME_SIZE];
    let mut t: u16 = 0;

    loop {
        render_demo_frame(&mut frame, t);
        let _ = write_frame(&mut i2c, addr, &frame);
        t = t.wrapping_add(1);
        Timer::after(FRAME_TIME).await;
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
    let _ = write_cmd2(i2c, addr, 0x81, 0xbf);
    let _ = write_cmd2(i2c, addr, 0xd9, 0xf1);
    let _ = write_cmd2(i2c, addr, 0xdb, 0x40);
    let _ = write_cmd(i2c, addr, 0xa4);
    let _ = write_cmd(i2c, addr, 0xa6);
    let _ = write_cmd(i2c, addr, 0xaf);
}

fn render_demo_frame(frame: &mut [u8; FRAME_SIZE], t: u16) {
    frame.fill(0);

    // Starfield.
    for i in 0..48u16 {
        let x = ((i * 29 + t * 3) % DISPLAY_WIDTH as u16) as usize;
        let y = ((i * 41 + t * ((i % 3) + 1)) % DISPLAY_HEIGHT as u16) as usize;
        set_pixel(frame, x, y);
    }

    // Three flowing wave lines.
    for x in 0..DISPLAY_WIDTH {
        let phase = (x as u8).wrapping_mul(4);
        let y1 = 10 + ((tri8(phase.wrapping_add((t * 3) as u8)) as usize * 18) / 255);
        let y2 = 20 + ((tri8(phase.wrapping_add((t * 5 + 80) as u8)) as usize * 20) / 255);
        let y3 = 6 + ((tri8(phase.wrapping_add((t * 2 + 170) as u8)) as usize * 16) / 255);
        set_pixel(frame, x, y1.min(DISPLAY_HEIGHT - 1));
        set_pixel(frame, x, y2.min(DISPLAY_HEIGHT - 1));
        set_pixel(frame, x, y3.min(DISPLAY_HEIGHT - 1));
    }

    // Bouncing square.
    let box_x = ping_pong((t as usize) * 2, DISPLAY_WIDTH - 10);
    let box_y = ping_pong((t as usize) * 3, DISPLAY_HEIGHT - 10);
    draw_rect(frame, box_x, box_y, 10, 10);

    // Small "PICO" tag in top-left.
    draw_text(frame, 0, 0, "PICO DEMO");
}

fn tri8(x: u8) -> u8 {
    if x < 128 {
        x.saturating_mul(2)
    } else {
        (255 - x).saturating_mul(2)
    }
}

fn ping_pong(t: usize, max: usize) -> usize {
    if max == 0 {
        return 0;
    }
    let span = max * 2;
    let p = t % span;
    if p <= max {
        p
    } else {
        span - p
    }
}

fn draw_rect(frame: &mut [u8; FRAME_SIZE], x: usize, y: usize, w: usize, h: usize) {
    for dx in 0..w {
        set_pixel(frame, x + dx, y);
        set_pixel(frame, x + dx, y + h - 1);
    }
    for dy in 0..h {
        set_pixel(frame, x, y + dy);
        set_pixel(frame, x + w - 1, y + dy);
    }
}

fn draw_text(frame: &mut [u8; FRAME_SIZE], page: usize, col: usize, text: &str) {
    if page >= DISPLAY_PAGES || col >= DISPLAY_WIDTH {
        return;
    }

    let mut cursor = col;
    for ch in text.bytes() {
        if cursor + 6 > DISPLAY_WIDTH {
            break;
        }
        let glyph = font_5x7(ch);
        let base = page * DISPLAY_WIDTH + cursor;
        frame[base..base + 5].copy_from_slice(&glyph);
        frame[base + 5] = 0x00;
        cursor += 6;
    }
}

fn set_pixel(frame: &mut [u8; FRAME_SIZE], x: usize, y: usize) {
    if x >= DISPLAY_WIDTH || y >= DISPLAY_HEIGHT {
        return;
    }
    let page = y / 8;
    let bit = y % 8;
    let idx = page * DISPLAY_WIDTH + x;
    frame[idx] |= 1 << bit;
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
        b'A' => [0x7e, 0x11, 0x11, 0x11, 0x7e],
        b'C' => [0x3e, 0x41, 0x41, 0x41, 0x22],
        b'D' => [0x7f, 0x41, 0x41, 0x22, 0x1c],
        b'E' => [0x7f, 0x49, 0x49, 0x49, 0x41],
        b'I' => [0x00, 0x41, 0x7f, 0x41, 0x00],
        b'M' => [0x7f, 0x02, 0x04, 0x02, 0x7f],
        b'O' => [0x3e, 0x41, 0x41, 0x41, 0x3e],
        b'P' => [0x7f, 0x09, 0x09, 0x09, 0x06],
        _ => [0x00, 0x00, 0x5f, 0x00, 0x00],
    }
}
