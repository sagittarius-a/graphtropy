use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use crate::entropy::EntropyData;
use crate::options::ColorTheme;

pub struct ExportConfig {
    pub show_filename: bool,
    pub show_algorithm: bool,
    pub show_block_step: bool,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            show_filename: true,
            show_algorithm: true,
            show_block_step: true,
        }
    }
}

const CHAR_W: f32 = 8.0;
const CHAR_H: f32 = 8.0;

pub fn render_to_png(
    path: &str,
    data: &EntropyData,
    theme: &ColorTheme,
    y_min: f64,
    y_max: f64,
    file_size: u64,
    caption: Option<&str>,
    x_label: &str,
    y_label: &str,
    width: u32,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pixmap = Pixmap::new(width, height).ok_or("Failed to create pixmap")?;
    let bg = theme.bg;
    pixmap.fill(Color::from_rgba8(bg.r(), bg.g(), bg.b(), 255));

    let margin: f32 = 16.0;
    let caption_h: f32 = if caption.is_some() { 18.0 } else { 0.0 };
    let y_label_w: f32 = if y_label.is_empty() { 0.0 } else { CHAR_H + 4.0 };
    let tick_label_left: f32 = CHAR_W * 4.0 + 6.0;
    let x_label_h: f32 = if x_label.is_empty() { 0.0 } else { CHAR_H + 4.0 };
    let tick_label_bottom: f32 = CHAR_H + 4.0;

    let y_pad = (y_max - y_min) * 0.03;
    let padded_y_min = y_min - y_pad;
    let padded_y_max = y_max + y_pad;
    let padded_range = padded_y_max - padded_y_min;

    let plot_left = margin + y_label_w + tick_label_left;
    let plot_right = width as f32 - margin - CHAR_W * 3.0;
    let plot_top = margin + caption_h;
    let plot_bottom = height as f32 - margin - tick_label_bottom - x_label_h;
    let plot_w = plot_right - plot_left;
    let plot_h = plot_bottom - plot_top;

    if plot_w <= 0.0 || plot_h <= 0.0 {
        return Err("Image too small".into());
    }

    let tc = theme.text;
    let label_color = Color::from_rgba8(tc.r(), tc.g(), tc.b(), 255);
    let cc = theme.caption;
    let caption_color = Color::from_rgba8(cc.r(), cc.g(), cc.b(), 255);

    // Caption
    if let Some(text) = caption {
        let tw = text.len() as f32 * CHAR_W;
        let cx = (width as f32 - tw) / 2.0;
        draw_text(&mut pixmap, text, cx, margin, caption_color);
    }

    // Y-axis label (vertical)
    if !y_label.is_empty() {
        let total_h = y_label.len() as f32 * CHAR_W;
        let ly = plot_top + (plot_h - total_h) / 2.0;
        draw_text_vertical(&mut pixmap, y_label, margin, ly, label_color);
    }

    // X-axis label (centered)
    if !x_label.is_empty() {
        let tw = x_label.len() as f32 * CHAR_W;
        let lx = plot_left + (plot_w - tw) / 2.0;
        let ly = height as f32 - margin - x_label_h + 2.0;
        draw_text(&mut pixmap, x_label, lx, ly, label_color);
    }

    // Grid
    let gc = theme.grid;
    let grid_paint = solid_paint(gc.r(), gc.g(), gc.b(), 255);
    let grid_stroke = Stroke { width: 1.0, ..Stroke::default() };

    for i in 0..=5u32 {
        let t = i as f64 / 5.0;
        let val = y_min + t * (y_max - y_min);
        let norm = (val - padded_y_min) / padded_range;
        let py = plot_bottom - (norm as f32 * plot_h);
        if let Some(path) = line_path(plot_left, py, plot_right, py) {
            pixmap.stroke_path(&path, &grid_paint, &grid_stroke, Transform::identity(), None);
        }
        let label = format!("{val:.1}");
        let tw = label.len() as f32 * CHAR_W;
        draw_text(&mut pixmap, &label, plot_left - tw - 4.0, py - CHAR_H / 2.0, label_color);
    }

    let x_max = file_size as f64;
    for i in 0..=5u32 {
        let t = i as f64 / 5.0;
        let px = plot_left + (t as f32 * plot_w);
        if let Some(path) = line_path(px, plot_top, px, plot_bottom) {
            pixmap.stroke_path(&path, &grid_paint, &grid_stroke, Transform::identity(), None);
        }
        let label = format_offset(t * x_max);
        let tw = label.len() as f32 * CHAR_W;
        draw_text(&mut pixmap, &label, px - tw / 2.0, plot_bottom + 4.0, label_color);
    }

    // Entropy line - walk actual data point pairs (matches GUI topology)
    if data.points.len() >= 2 && padded_range > 0.0 {
        let plot_left_i = plot_left.ceil() as i32;
        let plot_right_i = plot_right.floor() as i32;
        let plot_top_i = plot_top.ceil() as i32;
        let plot_bottom_i = plot_bottom.floor() as i32;
        let stride = width as usize;
        let pixels = pixmap.pixels_mut();

        let to_px = |x: f64, y: f64| -> (f32, f32) {
            let px = plot_left as f64 + (x / x_max) * plot_w as f64;
            let py = plot_bottom as f64 - ((y - padded_y_min) / padded_range) * plot_h as f64;
            (px as f32, py as f32)
        };

        let set_pixel = |pixels: &mut [tiny_skia::PremultipliedColorU8], ix: i32, iy: i32, color: tiny_skia::PremultipliedColorU8| {
            if ix >= plot_left_i && ix <= plot_right_i && iy >= plot_top_i && iy <= plot_bottom_i {
                pixels[iy as usize * stride + ix as usize] = color;
            }
        };

        for seg in 0..data.points.len() - 1 {
            let (x0, y0) = data.points[seg];
            let (x1, y1) = data.points[seg + 1];
            let (px0, py0) = to_px(x0, y0);
            let (px1, py1) = to_px(x1, y1);

            let dx = (px1 - px0).abs();
            let dy = (py1 - py0).abs();
            let steps = (dx.max(dy).ceil() as i32).max(1);

            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let px = px0 + (px1 - px0) * t;
                let py = py0 + (py1 - py0) * t;
                let y_val = y0 + (y1 - y0) * t as f64;

                let c = theme.color_for(y_val, y_max);
                let pc = tiny_skia::PremultipliedColorU8::from_rgba(c.r(), c.g(), c.b(), 255).unwrap();
                let ix = px.round() as i32;
                let iy = py.round() as i32;

                for dy in -1i32..=1 {
                    set_pixel(pixels, ix, iy + dy, pc);
                }
            }
        }
    }

    // Border
    let border_r = gc.r().saturating_add(30).min(255);
    let border_g = gc.g().saturating_add(30).min(255);
    let border_b = gc.b().saturating_add(30).min(255);
    let border_paint = solid_paint(border_r, border_g, border_b, 255);
    let border_stroke = Stroke { width: 1.0, ..Stroke::default() };
    if let Some(rect) = Rect::from_ltrb(plot_left, plot_top, plot_right, plot_bottom) {
        let mut pb = PathBuilder::new();
        pb.push_rect(rect);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &border_paint, &border_stroke, Transform::identity(), None);
        }
    }

    pixmap.save_png(path)?;
    Ok(())
}

fn solid_paint(r: u8, g: u8, b: u8, a: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(r, g, b, a);
    p.anti_alias = true;
    p
}

fn line_path(x1: f32, y1: f32, x2: f32, y2: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    pb.finish()
}

fn format_offset(value: f64) -> String {
    if value <= 0.0 {
        return "0".to_string();
    }
    let bytes = value as u64;
    if bytes >= 1_073_741_824 {
        format!("{:.1}GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1}MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("0x{bytes:X}")
    }
}

fn draw_text(pixmap: &mut Pixmap, text: &str, x: f32, y: f32, color: Color) {
    use font8x8::UnicodeFonts;
    let mut paint = Paint::default();
    paint.set_color(color);

    for (i, ch) in text.chars().enumerate() {
        let Some(glyph) = font8x8::BASIC_FONTS.get(ch) else { continue };
        let cx = x + i as f32 * CHAR_W;
        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..8u32 {
                if bits & (1 << col) != 0 {
                    let rx = cx + col as f32;
                    let ry = y + row as f32;
                    if let Some(rect) = Rect::from_xywh(rx, ry, 1.0, 1.0) {
                        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
                    }
                }
            }
        }
    }
}

fn draw_text_vertical(pixmap: &mut Pixmap, text: &str, x: f32, y: f32, color: Color) {
    use font8x8::UnicodeFonts;
    let mut paint = Paint::default();
    paint.set_color(color);

    let len = text.chars().count();
    for (i, ch) in text.chars().enumerate() {
        let Some(glyph) = font8x8::BASIC_FONTS.get(ch) else { continue };
        let cy = y + (len - 1 - i) as f32 * CHAR_W;
        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..8u32 {
                if bits & (1 << col) != 0 {
                    let rx = x + row as f32;
                    let ry = cy + (7 - col) as f32;
                    if let Some(rect) = Rect::from_xywh(rx, ry, 1.0, 1.0) {
                        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
                    }
                }
            }
        }
    }
}
