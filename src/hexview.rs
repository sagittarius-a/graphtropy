use egui::{Color32, Rect, Sense, TextStyle, pos2, vec2};

const BYTES_PER_ROW: usize = 16;

fn hex_str(b: u8) -> &'static str {
    static TABLE: std::sync::OnceLock<[String; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| std::array::from_fn(|i| format!("{i:02X}")));
    &table[b as usize]
}

pub fn render(
    ui: &mut egui::Ui,
    data: &[u8],
    cursor_offset: u64,
    first_row: &mut usize,
    focused: &mut bool,
    selection: &mut Option<(usize, usize)>,
) -> u64 {
    let total_rows = data.len().div_ceil(BYTES_PER_ROW);
    let font_size = ui.text_style_height(&TextStyle::Monospace);
    let font_id = egui::FontId::monospace(font_size);
    let row_height = font_size + 2.0;
    let char_w = ui.fonts_mut(|f| f.glyph_width(&font_id, '0'));
    let available_height = ui.available_height();
    let visible_rows = (available_height / row_height) as usize;
    let max_first_row = total_rows.saturating_sub(visible_rows);

    let hex_rect = ui.available_rect_before_wrap();
    let scrollbar_w = 12.0;
    let content_rect = Rect::from_min_max(
        hex_rect.min,
        pos2(hex_rect.max.x - scrollbar_w - 4.0, hex_rect.max.y),
    );
    let scrollbar_rect = Rect::from_min_max(
        pos2(hex_rect.max.x - scrollbar_w, hex_rect.min.y),
        hex_rect.max,
    );

    // Claim the full area (hover only - interaction handled below)

    // Layout positions
    let offset_x = content_rect.left();
    let hex_x = offset_x + 10.0 * char_w;
    let ascii_x = hex_x + 49.0 * char_w;

    let byte_hex_x = |col: usize| -> f32 {
        hex_x + col as f32 * 3.0 * char_w + if col >= 8 { char_w } else { 0.0 }
    };

    // Focus
    let pointer_in_hex = ui.input(|i| {
        i.pointer.latest_pos().is_some_and(|pos| hex_rect.contains(pos))
    });
    if ui.input(|i| i.pointer.any_pressed()) {
        *focused = pointer_in_hex;
    }

    // Scroll wheel
    if pointer_in_hex {
        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if scroll_delta != 0.0 {
            let rows_delta = (-scroll_delta / row_height).round() as isize;
            let new_row = (*first_row as isize + rows_delta).clamp(0, max_first_row as isize);
            *first_row = new_row as usize;
        }
    }

    // Page Up / Page Down
    if *focused {
        let page = visible_rows.max(1);
        if ui.input(|i| i.key_pressed(egui::Key::PageDown)) {
            *first_row = (*first_row + page).min(max_first_row);
        }
        if ui.input(|i| i.key_pressed(egui::Key::PageUp)) {
            *first_row = first_row.saturating_sub(page);
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            *selection = None;
        }
    }

    *first_row = (*first_row).min(max_first_row);
    let cursor_row = cursor_offset as usize / BYTES_PER_ROW;
    let end_row = (*first_row + visible_rows).min(total_rows);

    // Interaction
    let content_response = ui.allocate_rect(content_rect, Sense::click_and_drag());

    let byte_from_pos = |pos: egui::Pos2| -> Option<usize> {
        let row_f = (pos.y - content_rect.top()) / row_height;
        if row_f < 0.0 {
            return None;
        }
        let row = *first_row + row_f as usize;
        if row >= total_rows {
            return None;
        }

        let rel_x = pos.x - hex_x;
        let col = if rel_x < 0.0 {
            return None;
        } else if rel_x < 24.0 * char_w {
            (rel_x / (3.0 * char_w)) as usize
        } else if rel_x < 25.0 * char_w {
            7
        } else if rel_x < 49.0 * char_w {
            let adj = rel_x - 25.0 * char_w;
            (8 + (adj / (3.0 * char_w)) as usize).min(15)
        } else {
            return None;
        };

        let byte_idx = row * BYTES_PER_ROW + col;
        if byte_idx < data.len() {
            Some(byte_idx)
        } else {
            None
        }
    };

    // Selection: click to place, shift+click to extend, drag to range
    let shift_held = ui.input(|i| i.modifiers.shift);

    if content_response.clicked_by(egui::PointerButton::Primary) {
        if let Some(pos) = content_response.interact_pointer_pos() {
            if let Some(idx) = byte_from_pos(pos) {
                if shift_held {
                    if let Some((anchor, _)) = *selection {
                        *selection = Some((anchor, idx));
                    } else {
                        *selection = Some((idx, idx));
                    }
                } else {
                    *selection = Some((idx, idx));
                }
            }
        }
    }
    if content_response.drag_started_by(egui::PointerButton::Primary) {
        if let Some(pos) = content_response.interact_pointer_pos() {
            if let Some(idx) = byte_from_pos(pos) {
                *selection = Some((idx, idx));
            }
        }
    }
    if content_response.dragged_by(egui::PointerButton::Primary) {
        if let Some(pos) = content_response.interact_pointer_pos() {
            if let Some(idx) = byte_from_pos(pos) {
                if let Some((anchor, _)) = *selection {
                    *selection = Some((anchor, idx));
                }
            }
        }
    }

    if content_response.secondary_clicked() {
        if let Some(pos) = content_response.interact_pointer_pos() {
            if let Some(idx) = byte_from_pos(pos) {
                if selection.is_none() {
                    *selection = Some((idx, idx));
                }
            }
        }
    }

    let sel_range = selection.map(|(a, b)| (a.min(b), a.max(b)));

    // Colors
    let strong = ui.visuals().strong_text_color();
    let weak = ui.visuals().weak_text_color();
    let highlight = ui.visuals().selection.stroke.color;
    let sel_bg = ui.visuals().selection.bg_fill;

    let painter = ui.painter_at(content_rect);

    for row_idx in *first_row..end_row {
        let offset = row_idx * BYTES_PER_ROW;
        let end = (offset + BYTES_PER_ROW).min(data.len());
        let row_data = &data[offset..end];
        let is_cursor_row = row_idx == cursor_row;
        let y = content_rect.top() + (row_idx - *first_row) as f32 * row_height;

        let offset_color = if is_cursor_row { highlight } else { weak };
        painter.text(
            pos2(offset_x, y),
            egui::Align2::LEFT_TOP,
            format!("{offset:08X}  "),
            font_id.clone(),
            offset_color,
        );

        for (i, &byte) in row_data.iter().enumerate() {
            let global_idx = offset + i;
            let bx = byte_hex_x(i);
            let is_selected =
                sel_range.is_some_and(|(lo, hi)| global_idx >= lo && global_idx <= hi);

            if is_selected {
                painter.rect_filled(
                    Rect::from_min_size(pos2(bx - 1.0, y), vec2(2.0 * char_w + 2.0, row_height)),
                    2.0,
                    sel_bg,
                );
            }

            let color = if is_selected {
                strong
            } else if is_cursor_row {
                strong
            } else {
                weak
            };
            painter.text(pos2(bx, y), egui::Align2::LEFT_TOP, hex_str(byte), font_id.clone(), color);
        }

        // ASCII column
        for (i, &byte) in row_data.iter().enumerate() {
            let global_idx = offset + i;
            let ax = ascii_x + (1 + i) as f32 * char_w;
            let is_selected =
                sel_range.is_some_and(|(lo, hi)| global_idx >= lo && global_idx <= hi);

            if is_selected {
                painter.rect_filled(
                    Rect::from_min_size(pos2(ax, y), vec2(char_w, row_height)),
                    0.0,
                    sel_bg,
                );
            }

            let ch = if byte.is_ascii_graphic() || byte == b' ' {
                byte as char
            } else {
                '.'
            };
            let color = if is_selected {
                strong
            } else if is_cursor_row {
                highlight
            } else {
                weak
            };
            painter.text(pos2(ax, y), egui::Align2::LEFT_TOP, ch.to_string(), font_id.clone(), color);
        }

        // ASCII brackets
        let bracket_color = if is_cursor_row { highlight } else { weak };
        painter.text(
            pos2(ascii_x, y),
            egui::Align2::LEFT_TOP,
            "|",
            font_id.clone(),
            bracket_color,
        );
        painter.text(
            pos2(ascii_x + (1 + BYTES_PER_ROW) as f32 * char_w, y),
            egui::Align2::LEFT_TOP,
            "|",
            font_id.clone(),
            bracket_color,
        );
    }

    // Context menu
    content_response.context_menu(|ui| {
        if let Some((lo, hi)) = sel_range {
            let hi = hi.min(data.len().saturating_sub(1));
            let selected = &data[lo..=hi];
            let count = selected.len();
            ui.label(format!("{count} byte{}", if count == 1 { "" } else { "s" }));
            ui.separator();
            for (label, fmt) in [
                ("Hex (spaced)", format_hex_spaced as fn(&[u8]) -> String),
                ("Hex (compact)", format_hex_compact),
                ("C array", format_c_array),
                ("Rust slice", format_rust_slice),
                ("Python bytes", format_python_bytes),
                ("Base64", format_base64),
                ("UTF-8 (lossy)", format_utf8_lossy),
            ] {
                if ui.button(label).clicked() {
                    ui.ctx().copy_text(fmt(selected));
                    ui.close();
                }
            }
        }
    });

    // Scrollbar
    if total_rows > 0 {
        let sb_painter = ui.painter_at(scrollbar_rect);
        let track = scrollbar_rect.shrink2(vec2(2.0, 0.0));

        let track_color = if ui.visuals().dark_mode {
            Color32::from_gray(40)
        } else {
            Color32::from_gray(210)
        };
        sb_painter.rect_filled(track, 3.0, track_color);

        let frac_visible = (visible_rows as f32 / total_rows as f32).min(1.0);
        let frac_start = *first_row as f32 / total_rows as f32;
        let thumb_h = (frac_visible * track.height()).max(16.0);
        let thumb_max_y = track.height() - thumb_h;
        let thumb_y = track.min.y + frac_start * thumb_max_y / (1.0 - frac_visible).max(0.001);
        let thumb_rect = Rect::from_min_size(
            pos2(track.min.x, thumb_y.clamp(track.min.y, track.max.y - thumb_h)),
            vec2(track.width(), thumb_h),
        );

        let thumb_color = if ui.visuals().dark_mode {
            Color32::from_gray(100)
        } else {
            Color32::from_gray(150)
        };
        sb_painter.rect_filled(thumb_rect, 3.0, thumb_color);

        let sb_response = ui.allocate_rect(scrollbar_rect, Sense::click_and_drag());
        if sb_response.dragged() || sb_response.clicked() {
            if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
                let rel = (pos.y - track.min.y - thumb_h / 2.0) / (track.height() - thumb_h);
                let row = (rel.clamp(0.0, 1.0) * max_first_row as f32).round() as usize;
                *first_row = row.min(max_first_row);
            }
        }
    }

    (*first_row * BYTES_PER_ROW) as u64
}

fn format_hex_spaced(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

fn format_hex_compact(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

fn format_c_array(bytes: &[u8]) -> String {
    let items: Vec<_> = bytes.iter().map(|b| format!("0x{b:02X}")).collect();
    format!("{{{}}}", items.join(", "))
}

fn format_rust_slice(bytes: &[u8]) -> String {
    let items: Vec<_> = bytes.iter().map(|b| format!("0x{b:02X}")).collect();
    format!("[{}]", items.join(", "))
}

fn format_python_bytes(bytes: &[u8]) -> String {
    let inner: String = bytes.iter().map(|b| format!("\\x{b:02x}")).collect();
    format!("b'{inner}'")
}

fn format_base64(bytes: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 { CHARS[((n >> 6) & 0x3F) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { CHARS[(n & 0x3F) as usize] as char } else { '=' });
    }
    out
}

fn format_utf8_lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
