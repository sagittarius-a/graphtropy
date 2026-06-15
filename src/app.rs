use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;

use memmap2::Mmap;

use egui::Color32;
use egui_plot::PlotPoint;

use crate::entropy::{Algorithm, EntropyData};
use crate::export::ExportConfig;
use crate::options::Options;

type GradientFn = Arc<dyn Fn(PlotPoint) -> Color32 + Send + Sync>;

pub struct FileInfo {
    pub path: PathBuf,
    pub size: u64,
    pub block_size: usize,
    pub step: usize,
}

struct ComputeResult {
    data: EntropyData,
    block_size: usize,
    step: usize,
    warning: Option<String>,
}

pub struct App {
    mmap: Arc<Mmap>,
    file_info: FileInfo,
    entropy_data: EntropyData,
    cursor_offset: u64,
    hex_first_row: usize,
    last_hex_offset: u64,
    sync_cooldown: u8,
    last_hover_x: Option<f64>,
    hex_focused: bool,
    hex_selection: Option<(usize, usize)>,
    options: Options,
    view_x_min: f64,
    view_x_max: f64,
    goto_open: bool,
    goto_input: String,
    goto_focus: bool,
    warning: Option<String>,
    export_open: bool,
    export_config: ExportConfig,
    export_path: String,
    export_status: Option<String>,
    cached_plot_points: Vec<PlotPoint>,
    cached_gradient: GradientFn,
    cached_theme_index: usize,
    export_width: String,
    export_height: String,
    compute_rx: Option<mpsc::Receiver<ComputeResult>>,
}

fn spawn_compute(
    mmap: &Arc<Mmap>,
    block_size: usize,
    step: usize,
    algorithm: Algorithm,
) -> mpsc::Receiver<ComputeResult> {
    let (tx, rx) = mpsc::channel();
    let mmap = mmap.clone();
    std::thread::spawn(move || {
        let file_size = mmap.len();
        let (block_size, step, adapted) = crate::auto_adapt(file_size, block_size, step);
        let _ = mmap.advise(memmap2::Advice::Sequential);
        let data = crate::entropy::compute(&mmap, block_size, step, algorithm);
        let _ = mmap.advise(memmap2::Advice::Normal);
        let warning = if adapted {
            Some(format!(
                "Large file: auto-adjusted block={block_size} step={step} ({} points)",
                file_size / step,
            ))
        } else {
            None
        };
        let _ = tx.send(ComputeResult { data, block_size, step, warning });
    });
    rx
}

impl App {
    pub fn new(mmap: Arc<Mmap>, file_info: FileInfo) -> Self {
        let options = Options::new(file_info.block_size, file_info.step);
        let x_max = file_info.size as f64;
        let cached_gradient = build_gradient(&options);
        let export_path = file_info.path.with_extension("png")
            .to_string_lossy().to_string();
        let rx = spawn_compute(&mmap, file_info.block_size, file_info.step, Algorithm::Shannon);
        Self {
            mmap,
            file_info,
            entropy_data: EntropyData { points: vec![], min: 0.0, max: 0.0, avg: 0.0 },
            cursor_offset: 0,
            hex_first_row: 0,
            last_hex_offset: 0,
            sync_cooldown: 0,
            last_hover_x: None,
            hex_focused: false,
            hex_selection: None,
            options,
            view_x_min: 0.0,
            view_x_max: x_max,
            goto_open: false,
            goto_input: String::new(),
            goto_focus: false,
            warning: None,
            export_open: false,
            export_config: ExportConfig::default(),
            export_path,
            export_status: None,
            cached_plot_points: vec![],
            cached_gradient,
            cached_theme_index: usize::MAX,
            export_width: "1920".to_string(),
            export_height: "600".to_string(),
            compute_rx: Some(rx),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll background computation
        if let Some(rx) = &self.compute_rx {
            match rx.try_recv() {
                Ok(result) => {
                    self.entropy_data = result.data;
                    self.file_info.block_size = result.block_size;
                    self.file_info.step = result.step;
                    self.warning = result.warning;
                    self.cached_plot_points = build_plot_points(&self.entropy_data, &self.options);
                    self.cached_gradient = build_gradient(&self.options);
                    self.cached_theme_index = self.options.theme_index;
                    self.compute_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint();
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.compute_rx = None;
                }
            }
        }

        let computing = self.compute_rx.is_some();

        // Ctrl+S opens export dialog
        if !computing && ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S)) {
            self.export_open = true;
            self.export_status = None;
        }

        if self.export_open {
            let mut do_export = false;
            let mut close = false;

            egui::Window::new("Export graph")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Output path:");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.export_path);
                        if ui.button("Browse...").clicked() {
                            let mut dialog = rfd::FileDialog::new()
                                .add_filter("PNG", &["png"]);
                            if let Some(parent) = std::path::Path::new(&self.export_path).parent() {
                                dialog = dialog.set_directory(parent);
                            }
                            if let Some(name) = std::path::Path::new(&self.export_path).file_name() {
                                dialog = dialog.set_file_name(name.to_string_lossy());
                            }
                            if let Some(path) = dialog.save_file() {
                                self.export_path = path.to_string_lossy().to_string();
                            }
                        }
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Width:");
                        ui.add(egui::TextEdit::singleline(&mut self.export_width).desired_width(50.0));
                        ui.label("Height:");
                        ui.add(egui::TextEdit::singleline(&mut self.export_height).desired_width(50.0));
                    });

                    ui.add_space(4.0);
                    ui.label("Caption:");
                    ui.checkbox(&mut self.export_config.show_filename, "File name");
                    ui.checkbox(&mut self.export_config.show_algorithm, "Algorithm");
                    ui.checkbox(&mut self.export_config.show_block_step, "Block / Step size");

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button("Export").clicked() {
                            do_export = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });

                    if let Some(status) = &self.export_status {
                        let color = if status.starts_with("Error") {
                            if ui.visuals().dark_mode {
                                egui::Color32::RED
                            } else {
                                egui::Color32::from_rgb(180, 0, 0)
                            }
                        } else if ui.visuals().dark_mode {
                            egui::Color32::LIGHT_GREEN
                        } else {
                            egui::Color32::from_rgb(0, 120, 0)
                        };
                        ui.label(egui::RichText::new(status).color(color));
                    }
                });

            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
            if do_export {
                let caption = build_caption(&self.export_config, &self.file_info, &self.options);
                let (y_min, y_max) = self.options.algorithm.y_range();
                let w = self.export_width.parse().unwrap_or(1920);
                let h = self.export_height.parse().unwrap_or(600);
                match crate::export::render_to_png(
                    &self.export_path,
                    &self.entropy_data,
                    self.options.theme(),
                    y_min, y_max,
                    self.file_info.size,
                    caption.as_deref(),
                    "Offset",
                    self.options.algorithm.y_label(),
                    w, h,
                ) {
                    Ok(()) => {
                        self.export_status = Some(format!("Saved to {}", self.export_path));
                    }
                    Err(e) => {
                        self.export_status = Some(format!("Error: {e}"));
                    }
                }
            }
            if close {
                self.export_open = false;
            }
        }

        // Ctrl+G opens "Go to offset" dialog
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::G)) {
            self.goto_open = true;
            self.goto_input.clear();
            self.goto_focus = true;
        }

        // "Go to offset" modal window
        if self.goto_open {
            let mut jump_to = None;
            let mut close = false;

            egui::Window::new("Go to offset")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Enter offset (hex: 0x... or plain decimal):");
                    let re = ui.text_edit_singleline(&mut self.goto_input);
                    if self.goto_focus {
                        re.request_focus();
                        self.goto_focus = false;
                    }
                    if re.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        jump_to = parse_offset(&self.goto_input);
                        close = true;
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Go").clicked() {
                            jump_to = parse_offset(&self.goto_input);
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });

            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
            if let Some(offset) = jump_to {
                let offset = offset.min(self.file_info.size.saturating_sub(1));
                self.cursor_offset = offset;
                self.hex_first_row = offset as usize / 16;
                self.sync_cooldown = 5;
            }
            if close {
                self.goto_open = false;
            }
        }

        if self.options.needs_recompute && !computing {
            self.options.needs_recompute = false;
            self.compute_rx = Some(spawn_compute(
                &self.mmap,
                self.options.block_size,
                self.options.step,
                self.options.algorithm,
            ));
        }

        if self.options.theme_index != self.cached_theme_index {
            if !computing {
                self.cached_gradient = build_gradient(&self.options);
                self.cached_plot_points = build_plot_points(&self.entropy_data, &self.options);
            }
            self.cached_theme_index = self.options.theme_index;
            ctx.set_visuals(self.options.theme().to_visuals());
        }

        // Toolbar (top)
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Reset zoom").clicked() {
                    self.view_x_min = 0.0;
                    self.view_x_max = self.file_info.size as f64;
                }
                if ui.button("Go to (Ctrl+G)").clicked() {
                    self.goto_open = true;
                    self.goto_input.clear();
                    self.goto_focus = true;
                }
                if ui.button("Export (Ctrl+S)").clicked() {
                    self.export_open = true;
                    self.export_status = None;
                }
                if let Some(warn) = &self.warning {
                    let warn_color = if ui.visuals().dark_mode {
                        egui::Color32::YELLOW
                    } else {
                        egui::Color32::from_rgb(180, 100, 0)
                    };
                    ui.label(
                        egui::RichText::new(warn)
                            .color(warn_color),
                    );
                }
            });
        });

        // Status bar (bottom)
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let name = self.file_info.path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                let mut status = format!(
                    "{name} | {} | Cursor: 0x{:X}",
                    format_size(self.file_info.size),
                    self.cursor_offset,
                );
                if let Some((a, b)) = self.hex_selection {
                    let lo = a.min(b);
                    let hi = a.max(b);
                    let count = hi - lo + 1;
                    status.push_str(&format!(
                        " | Sel: 0x{lo:X}..0x{hi:X} ({count} byte{})",
                        if count == 1 { "" } else { "s" },
                    ));
                }
                ui.label(status);
            });
        });

        // Options panel (right side)
        egui::SidePanel::right("options_panel")
            .default_width(160.0)
            .resizable(true)
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(
                egui::Margin { left: 8, right: 16, top: 8, bottom: 8 },
            ))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.options.render_panel(ui, &self.entropy_data);
                });
            });

        // Main area: plot on top, hex viewer below
        egui::CentralPanel::default().show(ctx, |ui| {
            if computing {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() / 2.0 - 20.0);
                    ui.spinner();
                    ui.label("Computing entropy...");
                });
                return;
            }

            let available = ui.available_height();
            let plot_height = available * 0.45;

            // Entropy plot
            ui.add_space(4.0);
            let plot_w = ui.available_width() - 16.0;
            let plot_response = ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.allocate_ui(egui::vec2(plot_w, plot_height), |ui| {
                    crate::plot::render(
                        ui,
                        &self.cached_plot_points,
                        &self.cached_gradient,
                        &self.options,
                        self.cursor_offset,
                        self.file_info.size,
                        &mut self.last_hover_x,
                        &mut self.view_x_min,
                        &mut self.view_x_max,
                    )
                }).inner
            });
            if let Some(clicked_offset) = plot_response.inner {
                self.cursor_offset = clicked_offset;
                self.hex_first_row = clicked_offset as usize / 16;
                self.sync_cooldown = 5;
            }

            ui.separator();

            // Hex viewer (fills remaining space)
            let hex_offset = crate::hexview::render(
                ui,
                &self.mmap,
                self.cursor_offset,
                &mut self.hex_first_row,
                &mut self.hex_focused,
                &mut self.hex_selection,
            );

            if self.sync_cooldown > 0 {
                self.sync_cooldown -= 1;
            } else if hex_offset != self.last_hex_offset {
                self.cursor_offset = hex_offset;
            }
            self.last_hex_offset = hex_offset;
        });
    }
}

fn build_plot_points(data: &EntropyData, options: &Options) -> Vec<PlotPoint> {
    let raw: Vec<[f64; 2]> = data.points.iter().map(|&(x, y)| [x, y]).collect();
    let (_, y_max) = options.algorithm.y_range();
    crate::plot::subdivide_at_bands(&raw, options.theme(), y_max)
        .into_iter()
        .map(|p| PlotPoint::new(p[0], p[1]))
        .collect()
}

fn build_gradient(options: &Options) -> Arc<dyn Fn(PlotPoint) -> Color32 + Send + Sync> {
    let theme = options.theme().clone();
    let (_, y_max) = options.algorithm.y_range();
    Arc::new(move |pt| theme.color_for(pt.y, y_max))
}

fn parse_offset(input: &str) -> Option<u64> {
    let s = input.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u64>().ok()
    }
}

fn build_caption(config: &ExportConfig, file_info: &FileInfo, options: &Options) -> Option<String> {
    let mut parts = Vec::new();
    if config.show_filename {
        let name = file_info.path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        parts.push(name);
    }
    if config.show_algorithm {
        parts.push(options.algorithm.label().to_string());
    }
    if config.show_block_step {
        parts.push(format!("block={} step={}", options.block_size, options.step));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}
