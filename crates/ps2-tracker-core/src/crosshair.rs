//! Crosshair library: self-generated default PNGs and validation of custom files.

use std::path::{Path, PathBuf};

pub const MAX_SIDE: u32 = 512;
pub const MAX_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum CrosshairError {
    #[error("file: {0}")]
    Io(#[from] std::io::Error),
    #[error("PNG: {0}")]
    Decode(#[from] png::DecodingError),
    #[error("writing PNG: {0}")]
    Encode(#[from] png::EncodingError),
    #[error("{0}")]
    Invalid(String),
}

/// RGBA image with drawing helpers.
struct Canvas {
    w: u32,
    h: u32,
    px: Vec<u8>,
}

impl Canvas {
    fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            px: vec![0; (w * h * 4) as usize],
        }
    }
    fn set(&mut self, x: i32, y: i32, c: [u8; 4]) {
        if x < 0 || y < 0 || x as u32 >= self.w || y as u32 >= self.h {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        self.px[i..i + 4].copy_from_slice(&c);
    }
    fn alpha(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x as u32 >= self.w || y as u32 >= self.h {
            return 0;
        }
        self.px[((y as u32 * self.w + x as u32) * 4 + 3) as usize]
    }
    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: [u8; 4]) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.set(x, y, c);
            }
        }
    }
    /// Black 1 px outline around all set pixels, visible on light and dark backgrounds.
    fn outline(&mut self) {
        self.outline_alpha(200);
    }
    /// Outline with the given opacity (lighter for the coloured sets).
    fn outline_alpha(&mut self, a: u8) {
        let mut add = Vec::new();
        for y in 0..self.h as i32 {
            for x in 0..self.w as i32 {
                if self.alpha(x, y) == 0 {
                    let near = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                        .iter()
                        .any(|(dx, dy)| self.alpha(x + dx, y + dy) > 0);
                    if near {
                        add.push((x, y));
                    }
                }
            }
        }
        for (x, y) in add {
            self.set(x, y, [0, 0, 0, a]);
        }
    }
    fn png(&self) -> Result<Vec<u8>, CrosshairError> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.w, self.h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header()?;
            w.write_image_data(&self.px)?;
        }
        Ok(out)
    }
}

const WHITE: [u8; 4] = [255, 255, 255, 255];
const GREEN: [u8; 4] = [80, 255, 120, 255];

/// Colour sets of the cross/dot crosshairs (file name suffix, colour).
const COLOURS: [(&str, [u8; 4]); 3] = [
    ("green", [0x00, 0xFF, 0x00, 255]),
    ("magenta", [0xFF, 0x00, 0xFF, 255]),
    ("red", [0xFF, 0x00, 0x00, 255]),
];
const ORANGE: [u8; 4] = [0xFF, 0x66, 0x00, 255];
/// Opacity of the dark outline of the coloured sets (light, only for contrast).
const LIGHT_OUTLINE: u8 = 140;

/// Cross with 2 px bars on an even canvas: the bars sit symmetric around the
/// centre line between the two middle pixels, as the centre of an even monitor.
/// `gap`: free pixels in the middle (even, 0 = solid); `dash`: free pixels
/// in the middle of each arm (0 = solid arm).
fn cross(side: i32, gap: i32, dash: i32, c: [u8; 4]) -> Canvas {
    let mut k = Canvas::new(side as u32, side as u32);
    let m = side / 2; // bars on m-1 and m
    let (outer, inner) = (1, m - 1 - gap / 2); // arm from outer..=inner (top/left side)
    let mut arm = |from: i32, to: i32| {
        k.rect(m - 1, from, m, to, c); // top
        k.rect(m - 1, side - 1 - to, m, side - 1 - from, c); // bottom
        k.rect(from, m - 1, to, m, c); // left
        k.rect(side - 1 - to, m - 1, side - 1 - from, m, c); // right
    };
    if dash > 0 {
        let len = inner - outer + 1;
        let seg = (len - dash) / 2;
        arm(outer, outer + seg - 1);
        arm(inner - seg + 1, inner);
    } else {
        arm(outer, inner);
    }
    k.outline_alpha(LIGHT_OUTLINE);
    k
}

/// 3×5 pixel digits 1–9 (rows top to bottom, 3 bits each, MSB left).
const DIGITS: [[u8; 5]; 9] = [
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b110, 0b001, 0b010, 0b100, 0b111],
    [0b110, 0b001, 0b010, 0b001, 0b110],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b110, 0b001, 0b110],
    [0b011, 0b100, 0b110, 0b101, 0b010],
    [0b111, 0b001, 0b010, 0b010, 0b010],
    [0b010, 0b101, 0b010, 0b101, 0b010],
    [0b010, 0b101, 0b011, 0b001, 0b110],
];

/// Geometry of the range ladder (for the tests): side lengths and the centre.
pub struct LadderSize {
    pub width: u32,
    pub height: u32,
    pub centre: (i32, i32),
}

/// Range ladder: horizontal line with alternating small/large dots, a diamond
/// with small arrows in the middle, below it a vertical ladder with ticks and
/// the numbers 1–9 (holdover aid, not calibrated to a weapon). The image is
/// filled up transparently above, so the diamond is the exact image centre.
/// `s`: scale, 1.0 for 1440p, 1.5 for 4K.
fn range_ladder(s: f32) -> (Canvas, LadderSize) {
    let r = |v: f32| (v * s).round() as i32;
    let half_line = r(90.0); // horizontal line: half length
    let step = r(18.0); // dots on the line
    let gap = r(7.0); // free space around the diamond
    let diamond = r(3.0);
    let rung = r(16.0); // distance of the ladder rungs
    let font = r(2.0).max(2); // pixel size of the digits
    let small = r(1.0).max(1);
    let large = r(2.0).max(2);
    // extent from the centre: right/left and down; up is filled symmetrically
    let half_w = (half_line + large + 2).max(r(10.0) + 4 * font + 2);
    let down = 9 * rung + 3 * font + 2;
    let (w, h) = (2 * half_w + 1, 2 * down + 1);
    let (cx, cy) = (half_w, down);
    let mut k = Canvas::new(w as u32, h as u32);
    // horizontal line, left and right of the diamond
    k.rect(cx - half_line, cy, cx - gap, cy, ORANGE);
    k.rect(cx + gap, cy, cx + half_line, cy, ORANGE);
    // dots: odd steps small, even steps large
    let mut n = 1;
    while n * step <= half_line {
        let d = if n % 2 == 0 { large } else { small };
        for x in [cx - n * step, cx + n * step] {
            k.rect(x - d, cy - d, x + d, cy + d, ORANGE);
        }
        n += 1;
    }
    // diamond in the middle
    for dy in -diamond..=diamond {
        let span = diamond - dy.abs();
        k.rect(cx - span, cy + dy, cx + span, cy + dy, ORANGE);
    }
    // small arrows pointing at the diamond (left ">" and right "<")
    let tip = diamond + r(2.0).max(2);
    for i in 0..=r(2.0).max(2) {
        for x in [cx - tip - i, cx + tip + i] {
            k.set(x, cy - i, ORANGE);
            k.set(x, cy + i, ORANGE);
        }
    }
    // vertical ladder below
    k.rect(cx, cy + gap, cx, cy + 9 * rung, ORANGE);
    for n in 1..=9 {
        let y = cy + n * rung;
        let half = if n % 2 == 1 { r(8.0) } else { r(5.0) };
        k.rect(cx - half, y, cx + half, y, ORANGE);
        // number to the right of the rung, vertically centred on it
        let (x0, y0) = (cx + r(10.0), y - (5 * font) / 2);
        for (row, bits) in DIGITS[n as usize - 1].iter().enumerate() {
            for col in 0..3 {
                if bits & (0b100 >> col) != 0 {
                    let (px, py) = (x0 + col * font, y0 + row as i32 * font);
                    k.rect(px, py, px + font - 1, py + font - 1, ORANGE);
                }
            }
        }
    }
    k.outline_alpha(LIGHT_OUTLINE);
    let size = LadderSize {
        width: w as u32,
        height: h as u32,
        centre: (cx, cy),
    };
    (k, size)
}

/// Default crosshairs: (file name, PNG). Odd sides have a true centre pixel;
/// the 2 px crosses use even sides (centre between the two middle pixels).
pub fn standard_crosshairs() -> Result<Vec<(String, Vec<u8>)>, CrosshairError> {
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    for (name, bytes) in classic()? {
        out.push((name.to_owned(), bytes));
    }
    for (col, c) in COLOURS {
        out.push((format!("cross-{col}.png"), cross(32, 6, 0, c).png()?));
        out.push((format!("cross-small-{col}.png"), cross(24, 4, 0, c).png()?));
        out.push((format!("cross-tiny-{col}.png"), cross(12, 0, 0, c).png()?));
        out.push((format!("cross-dashed-{col}.png"), cross(32, 6, 2, c).png()?));
        let mut d = Canvas::new(5, 5);
        d.rect(1, 1, 3, 3, c);
        d.outline_alpha(220);
        out.push((format!("dot-{col}.png"), d.png()?));
    }
    out.push(("range-ladder-1440p.png".into(), range_ladder(1.0).0.png()?));
    out.push(("range-ladder-4k.png".into(), range_ladder(1.5).0.png()?));
    Ok(out)
}

/// The first four defaults (unchanged file names, existing users have them).
fn classic() -> Result<Vec<(&'static str, Vec<u8>)>, CrosshairError> {
    let mut out = Vec::new();

    let mut c = Canvas::new(9, 9);
    c.rect(3, 3, 5, 5, WHITE);
    c.outline();
    out.push(("punkt.png", c.png()?));

    let mut c = Canvas::new(23, 23);
    c.rect(11, 2, 11, 8, WHITE);
    c.rect(11, 14, 11, 20, WHITE);
    c.rect(2, 11, 8, 11, WHITE);
    c.rect(14, 11, 20, 11, WHITE);
    c.outline();
    out.push(("kreuz.png", c.png()?));

    let mut c = Canvas::new(23, 23);
    c.rect(11, 3, 11, 8, GREEN);
    c.rect(11, 14, 11, 19, GREEN);
    c.rect(3, 11, 8, 11, GREEN);
    c.rect(14, 11, 19, 11, GREEN);
    c.rect(11, 11, 11, 11, GREEN);
    c.outline();
    out.push(("kreuz-punkt-gruen.png", c.png()?));

    let mut c = Canvas::new(25, 25);
    let (cx, cy, r) = (12.0f32, 12.0f32, 9.0f32);
    for y in 0..25 {
        for x in 0..25 {
            let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
            if (d - r).abs() < 0.75 {
                c.set(x, y, WHITE);
            }
        }
    }
    c.set(12, 12, WHITE);
    c.outline();
    out.push(("kreis.png", c.png()?));

    Ok(out)
}

/// Installs the default crosshairs into the library (existing files are kept).
pub fn install_standard(dir: &Path) -> Result<(), CrosshairError> {
    std::fs::create_dir_all(dir)?;
    for (name, bytes) in standard_crosshairs()? {
        let p = dir.join(name);
        if !p.exists() {
            std::fs::write(p, bytes)?;
        }
    }
    Ok(())
}

/// Validates a PNG (format, size) and returns width/height.
pub fn inspect(bytes: &[u8]) -> Result<(u32, u32), CrosshairError> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err(CrosshairError::Invalid("file too large (max. 2 MB)".into()));
    }
    let dec = png::Decoder::new(std::io::Cursor::new(bytes));
    let reader = dec.read_info()?;
    let info = reader.info();
    if info.width == 0 || info.height == 0 || info.width > MAX_SIDE || info.height > MAX_SIDE {
        return Err(CrosshairError::Invalid(format!(
            "size {}x{} not allowed (max. {MAX_SIDE}x{MAX_SIDE})",
            info.width, info.height
        )));
    }
    Ok((info.width, info.height))
}

/// PNG files of the library, sorted.
pub fn list(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.extension()
                            .and_then(|e| e.to_str())
                            .is_some_and(|e| e.eq_ignore_ascii_case("png"))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Copies a custom PNG into the library (after validation) and returns the file name.
pub fn import(src: &Path, dir: &Path) -> Result<String, CrosshairError> {
    let bytes = std::fs::read(src)?;
    inspect(&bytes)?;
    let stem = src
        .file_stem()
        .map(|s| crate::voicepack::slug(&s.to_string_lossy()))
        .unwrap_or_else(|| "crosshair".into());
    std::fs::create_dir_all(dir)?;
    let mut name = format!("{stem}.png");
    let mut i = 2;
    while dir.join(&name).exists() {
        name = format!("{stem}-{i}.png");
        i += 1;
    }
    std::fs::write(dir.join(&name), bytes)?;
    Ok(name)
}

/// Window position (physical pixels) so the image sits exactly centred on the monitor.
/// Odd remainders are rounded down; `offset` corrects games that centre 1 px off.
pub fn centered_position(
    monitor_pos: (i32, i32),
    monitor_size: (u32, u32),
    window_size: (u32, u32),
    offset: (i32, i32),
) -> (i32, i32) {
    let x = monitor_pos.0 + (monitor_size.0 as i32 - window_size.0 as i32).div_euclid(2) + offset.0;
    let y = monitor_pos.1 + (monitor_size.1 as i32 - window_size.1 as i32).div_euclid(2) + offset.1;
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut r = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let mut buf = vec![0; r.output_buffer_size().unwrap()];
        let info = r.next_frame(&mut buf).unwrap();
        (info.width, info.height, buf)
    }

    fn alpha(px: &[u8], w: u32, x: u32, y: u32) -> u8 {
        px[((y * w + x) * 4 + 3) as usize]
    }

    #[test]
    fn standard_pngs_are_valid_and_centered() {
        let all = standard_crosshairs().unwrap();
        assert_eq!(all.len(), 4 + 3 * 5 + 2);
        for (name, bytes) in &all {
            let (w, h, px) = decode(bytes);
            inspect(bytes).unwrap();
            if name.starts_with("range-ladder") {
                continue; // own test (numbers only on the right)
            }
            assert_eq!(w, h, "{name}: square");
            // mirror symmetric in both axes: the centre is the image centre
            // (a pixel for odd sides, the line between two for even sides)
            for y in 0..h {
                for x in 0..w {
                    let a = alpha(&px, w, x, y);
                    assert_eq!(a, alpha(&px, w, w - 1 - x, y), "{name}: {x},{y} left/right");
                    assert_eq!(a, alpha(&px, w, x, h - 1 - y), "{name}: {x},{y} up/down");
                }
            }
        }
        let size = |n: &str| {
            let (w, h, _) = decode(&all.iter().find(|(f, _)| f == n).unwrap().1);
            (w, h)
        };
        assert_eq!(size("cross-green.png"), (32, 32));
        assert_eq!(size("cross-small-red.png"), (24, 24));
        assert_eq!(size("cross-tiny-magenta.png"), (12, 12));
        assert_eq!(size("cross-dashed-green.png"), (32, 32));
        assert_eq!(size("dot-red.png"), (5, 5));
        assert_eq!(size("punkt.png").0 % 2, 1);
    }

    #[test]
    fn cross_shapes() {
        let (w, _, px) = decode(&cross(32, 6, 0, COLOURS[0].1).png().unwrap());
        let colour = |x, y| {
            let i = ((y * w + x) * 4) as usize;
            px[i..i + 4] == [0, 255, 0, 255]
        };
        // 2 px bars on columns 15/16, gap of 6 px in the middle (rows 13..=18)
        assert!(colour(15, 5) && colour(16, 5) && !colour(14, 5) && !colour(17, 5));
        assert!(colour(15, 12) && !colour(15, 13) && !colour(15, 18) && colour(15, 19));
        // tiny: no gap
        let (w, _, px) = decode(&cross(12, 0, 0, COLOURS[2].1).png().unwrap());
        assert_eq!(&px[((5 * w + 5) * 4) as usize..][..4], &[255, 0, 0, 255]);
        // dashed: a free part in the middle of each arm
        let (w, _, px) = decode(&cross(32, 6, 2, COLOURS[1].1).png().unwrap());
        let row: Vec<bool> = (1..=12).map(|y| alpha(&px, w, 15, y) == 255).collect();
        assert!(row.contains(&false) && row[0] && row[11], "{row:?}");
    }

    #[test]
    fn range_ladder_centre() {
        for (s, file) in [
            (1.0, "range-ladder-1440p.png"),
            (1.5, "range-ladder-4k.png"),
        ] {
            let (canvas, size) = range_ladder(s);
            let (w, h, px) = decode(&canvas.png().unwrap());
            assert_eq!((w, h), (size.width, size.height), "{file}");
            assert_eq!(w % 2, 1, "{file}: odd width");
            assert_eq!(h % 2, 1, "{file}: odd height");
            // the diamond is the exact image centre
            assert_eq!(size.centre, ((w / 2) as i32, (h / 2) as i32), "{file}");
            let (cx, cy) = (w / 2, h / 2);
            let i = ((cy * w + cx) * 4) as usize;
            assert_eq!(&px[i..i + 4], &ORANGE, "{file}: centre pixel");
            // above the line only transparent filling (beyond the diamond/arrows)
            let top = cy - 8 * s as u32;
            assert!(
                (0..top).all(|y| (0..w).all(|x| alpha(&px, w, x, y) == 0)),
                "{file}"
            );
            // the horizontal line is symmetric around the centre
            for x in 0..w {
                assert_eq!(
                    alpha(&px, w, x, cy),
                    alpha(&px, w, w - 1 - x, cy),
                    "{file}: {x}"
                );
            }
            // ladder: something drawn below at the 9th rung
            let rung = (16.0 * s).round() as u32;
            assert_eq!(alpha(&px, w, cx, cy + 9 * rung), 255, "{file}");
            assert!(w <= MAX_SIDE && h <= MAX_SIDE);
        }
    }

    #[test]
    fn rejects_non_png_and_huge() {
        assert!(inspect(b"not a png").is_err());
        let mut c = Canvas::new(600, 4);
        c.set(0, 0, WHITE);
        assert!(inspect(&c.png().unwrap()).is_err());
    }

    #[test]
    fn centering_math() {
        // 2560x1440 on a second monitor to the right, crosshair 23x23
        assert_eq!(
            centered_position((1920, 0), (2560, 1440), (23, 23), (0, 0)),
            (1920 + 1268, 708)
        );
        assert_eq!(
            centered_position((0, 0), (1920, 1080), (9, 9), (1, -1)),
            (955 + 1, 535 - 1)
        );
    }

    #[test]
    fn import_and_list() {
        let dir = std::env::temp_dir().join(format!("ps2t-xh-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        install_standard(&dir).unwrap();
        assert_eq!(list(&dir).len(), standard_crosshairs().unwrap().len());
        let src = dir.join("punkt.png");
        let name = import(&src, &dir).unwrap();
        assert_eq!(name, "punkt-2.png");
        let _ = std::fs::remove_dir_all(dir);
    }
}
