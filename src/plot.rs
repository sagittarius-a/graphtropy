use std::sync::Arc;

use egui::Color32;
use egui_plot::{Line, Plot, PlotBounds, PlotPoint, PlotPoints, VLine};

use crate::options::{ColorTheme, Options};

const ZOOM_FACTOR: f64 = 1.3;

pub fn render(
    ui: &mut egui::Ui,
    plot_points: &[PlotPoint],
    gradient: &Arc<dyn Fn(PlotPoint) -> Color32 + Send + Sync>,
    options: &Options,
    cursor_offset: u64,
    file_size: u64,
    last_hover_x: &mut Option<f64>,
    view_x_min: &mut f64,
    view_x_max: &mut f64,
) -> Option<u64> {
    let (y_min, y_max) = options.algorithm.y_range();
    let x_max = file_size as f64;

    let mut clicked_offset = None;
    let dark_mode = ui.visuals().dark_mode;

    let vx_min = *view_x_min;
    let vx_max = *view_x_max;

    // Only pass visible points (with 1 extra on each side for edge continuity)
    let start = plot_points.partition_point(|p| p.x < vx_min).saturating_sub(1);
    let end = (plot_points.partition_point(|p| p.x <= vx_max) + 1).min(plot_points.len());
    let visible_points = &plot_points[start..end];

    let response = Plot::new("entropy_plot")
        .height(ui.available_height())
        .width(ui.available_width())
        .x_axis_label("Offset")
        .y_axis_label(options.algorithm.y_label())
        .x_axis_formatter(|mark, _range| format_offset(mark.value))
        .y_axis_formatter(|mark, _range| format!("{:.1}", mark.value))
        .label_formatter(|_name, point| {
            let offset = point.x as u64;
            format!(
                "Offset: 0x{:X} ({})\nValue: {:.4}",
                offset,
                format_offset_human(offset),
                point.y,
            )
        })
        .set_margin_fraction(egui::vec2(0.02, 0.05))
        .allow_zoom(false)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .show(ui, |plot_ui| {
            let y_pad = (y_max - y_min) * 0.03;
            plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                [vx_min, y_min - y_pad],
                [vx_max, y_max + y_pad],
            ));

            plot_ui.line(
                Line::new("entropy", PlotPoints::Borrowed(visible_points))
                    .width(1.5)
                    .gradient_color(gradient.clone(), false),
            );
            let cursor_color = if dark_mode {
                Color32::from_rgba_premultiplied(255, 255, 255, 180)
            } else {
                Color32::from_rgba_premultiplied(0, 0, 0, 160)
            };
            plot_ui.vline(
                VLine::new("cursor", cursor_offset as f64)
                    .color(cursor_color)
                    .width(1.0),
            );

            plot_ui.pointer_coordinate()
        });

    if let Some(coord) = response.inner {
        *last_hover_x = Some(coord.x);
    }

    let plot_rect = response.response.rect;
    let pointer_in_rect = ui.input(|i| {
        i.pointer
            .latest_pos()
            .is_some_and(|pos| plot_rect.contains(pos))
    });

    if pointer_in_rect {
        let scroll_y = ui.input(|i| i.raw_scroll_delta.y);

        if scroll_y != 0.0 {
            let hover_x = last_hover_x.unwrap_or((*view_x_min + *view_x_max) / 2.0);
            let factor = if scroll_y > 0.0 { 1.0 / ZOOM_FACTOR } else { ZOOM_FACTOR };

            let left = hover_x - *view_x_min;
            let right = *view_x_max - hover_x;
            let mut new_min = hover_x - left * factor;
            let mut new_max = hover_x + right * factor;

            // Clamp to data bounds
            if new_max - new_min >= x_max {
                new_min = 0.0;
                new_max = x_max;
            } else {
                if new_min < 0.0 {
                    new_max -= new_min;
                    new_min = 0.0;
                }
                if new_max > x_max {
                    new_min -= new_max - x_max;
                    new_max = x_max;
                }
                new_min = new_min.max(0.0);
            }

            *view_x_min = new_min;
            *view_x_max = new_max;
        }

        // Pan with middle mouse drag or Zoom mode drag
        let drag_delta = ui.input(|i| i.pointer.delta());
        let primary_down = ui.input(|i| i.pointer.primary_down());
        let middle_down = ui.input(|i| i.pointer.middle_down());

        if middle_down || primary_down {
            let width = plot_rect.width() as f64;
            let dx = -drag_delta.x as f64 * (*view_x_max - *view_x_min) / width;
            let mut new_min = *view_x_min + dx;
            let mut new_max = *view_x_max + dx;
            if new_min < 0.0 {
                new_max -= new_min;
                new_min = 0.0;
            }
            if new_max > x_max {
                new_min -= new_max - x_max;
                new_max = x_max;
            }
            *view_x_min = new_min.max(0.0);
            *view_x_max = new_max.min(x_max);
        }

        if ui.input(|i| i.pointer.any_click()) {
            if let Some(x) = *last_hover_x {
                let offset = (x as u64).min(file_size.saturating_sub(1));
                clicked_offset = Some(offset);
            }
        }
    }

    clicked_offset
}

pub fn subdivide_at_bands(points: &[[f64; 2]], theme: &ColorTheme, y_max: f64) -> Vec<[f64; 2]> {
    if points.len() < 2 {
        return points.to_vec();
    }

    let band_ys: Vec<f64> = theme.bands.iter().map(|(t, _)| t * y_max).collect();

    let mut result = Vec::with_capacity(points.len() * 2);
    result.push(points[0]);

    for i in 0..points.len() - 1 {
        let [x0, y0] = points[i];
        let [x1, y1] = points[i + 1];
        let (lo, hi) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };

        let mut crossings: Vec<f64> = band_ys
            .iter()
            .filter(|&&by| by > lo && by < hi)
            .copied()
            .collect();

        if y0 <= y1 {
            crossings.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
        } else {
            crossings.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap());
        }

        for cy in crossings {
            let t = (cy - y0) / (y1 - y0);
            let cx = x0 + (x1 - x0) * t;
            result.push([cx, cy]);
        }

        result.push(points[i + 1]);
    }

    result
}

fn format_offset(value: f64) -> String {
    if value <= 0.0 {
        return "0".to_string();
    }
    format_offset_human(value as u64)
}

fn format_offset_human(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("0x{bytes:X}")
    }
}
