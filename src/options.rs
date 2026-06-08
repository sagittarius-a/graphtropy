use std::fs;
use std::path::PathBuf;

use egui::Color32;
use serde::Deserialize;

use crate::entropy::{Algorithm, EntropyData, ALL_ALGORITHMS};

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

#[derive(Clone)]
pub struct ColorTheme {
    pub name: String,
    pub bands: Vec<(f64, Color32)>,
    pub bg: Color32,
    pub text: Color32,
    pub grid: Color32,
    pub caption: Color32,
    pub dark: bool,
}

impl ColorTheme {
    pub fn color_for(&self, value: f64, y_max: f64) -> Color32 {
        let t = (value / y_max).clamp(0.0, 1.0);
        if self.bands.is_empty() {
            return Color32::WHITE;
        }
        if t <= self.bands[0].0 || self.bands.len() == 1 {
            return self.bands[0].1;
        }
        if t >= self.bands.last().unwrap().0 {
            return self.bands.last().unwrap().1;
        }
        for i in 1..self.bands.len() {
            if t <= self.bands[i].0 {
                let (t0, c0) = self.bands[i - 1];
                let (t1, c1) = self.bands[i];
                let f = ((t - t0) / (t1 - t0)) as f32;
                return lerp_color(c0, c1, f);
            }
        }
        self.bands.last().unwrap().1
    }

    fn dark() -> Self {
        ColorTheme {
            name: "Dark".to_string(),
            bands: vec![
                (0.0, Color32::from_rgb(33, 150, 243)),
                (0.25, Color32::from_rgb(76, 175, 80)),
                (0.625, Color32::from_rgb(255, 152, 0)),
                (0.875, Color32::from_rgb(244, 67, 54)),
            ],
            bg: Color32::from_rgb(30, 30, 30),
            text: Color32::from_rgb(160, 160, 160),
            grid: Color32::from_rgb(50, 50, 50),
            caption: Color32::from_rgb(200, 200, 200),
            dark: true,
        }
    }

    fn light() -> Self {
        ColorTheme {
            name: "Light".to_string(),
            bands: vec![
                (0.0, Color32::from_rgb(21, 101, 192)),
                (0.25, Color32::from_rgb(0, 121, 107)),
                (0.625, Color32::from_rgb(230, 126, 34)),
                (0.875, Color32::from_rgb(183, 28, 28)),
            ],
            bg: Color32::from_rgb(245, 245, 245),
            text: Color32::from_rgb(60, 60, 60),
            grid: Color32::from_rgb(200, 200, 200),
            caption: Color32::from_rgb(40, 40, 40),
            dark: false,
        }
    }
}

#[derive(Deserialize)]
struct ThemeFile {
    name: String,
    bands: Vec<BandEntry>,
}

#[derive(Deserialize)]
struct BandEntry {
    threshold: f64,
    color: String,
}

fn parse_hex_color(s: &str) -> Option<Color32> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(Color32::from_rgb(r, g, b))
}

fn themes_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("graphtropy").join("themes"))
}

fn ensure_default_themes() {
    let Some(dir) = themes_dir() else { return };
    let _ = fs::create_dir_all(&dir);

    let dark_path = dir.join("dark.toml");
    if !dark_path.exists() {
        let _ = fs::write(
            dark_path,
            r##"name = "Dark"

[[bands]]
threshold = 0.0
color = "#2196F3"

[[bands]]
threshold = 0.25
color = "#4CAF50"

[[bands]]
threshold = 0.625
color = "#FF9800"

[[bands]]
threshold = 0.875
color = "#F44336"
"##,
        );
    }

    let light_path = dir.join("light.toml");
    if !light_path.exists() {
        let _ = fs::write(
            light_path,
            r##"name = "Light"

[[bands]]
threshold = 0.0
color = "#1565C0"

[[bands]]
threshold = 0.25
color = "#00796B"

[[bands]]
threshold = 0.625
color = "#E67E22"

[[bands]]
threshold = 0.875
color = "#B71C1C"
"##,
        );
    }

    // Remove old "classic.toml" if it exists
    let classic_path = dir.join("classic.toml");
    if classic_path.exists() {
        let _ = fs::remove_file(classic_path);
    }
}

pub fn load_themes() -> Vec<ColorTheme> {
    ensure_default_themes();
    let mut themes = vec![ColorTheme::dark(), ColorTheme::light()];

    let Some(dir) = themes_dir() else {
        return themes;
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return themes;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "toml") {
            if let Ok(contents) = fs::read_to_string(&path) {
                if let Ok(tf) = toml::from_str::<ThemeFile>(&contents) {
                    if tf.name == "Dark" || tf.name == "Light" {
                        continue;
                    }
                    let bands: Vec<(f64, Color32)> = tf
                        .bands
                        .iter()
                        .filter_map(|b| parse_hex_color(&b.color).map(|c| (b.threshold, c)))
                        .collect();
                    if !bands.is_empty() {
                        themes.push(ColorTheme {
                            name: tf.name,
                            bands,
                            ..ColorTheme::dark()
                        });
                    }
                }
            }
        }
    }

    themes
}

pub struct Options {
    pub algorithm: Algorithm,
    pub block_size: usize,
    pub step: usize,
    pub theme_index: usize,
    pub needs_recompute: bool,
    pub custom_block_input: String,
    pub custom_step_input: String,
    pub themes: Vec<ColorTheme>,
}

impl Options {
    pub fn new(block_size: usize, step: usize) -> Self {
        Self {
            algorithm: Algorithm::Shannon,
            block_size,
            step,
            theme_index: 0,
            needs_recompute: false,
            custom_block_input: String::new(),
            custom_step_input: String::new(),
            themes: load_themes(),
        }
    }

    pub fn theme(&self) -> &ColorTheme {
        &self.themes[self.theme_index]
    }

    pub fn render_panel(&mut self, ui: &mut egui::Ui, data: &EntropyData) {
        ui.heading("Options");
        ui.separator();

        ui.label("Algorithm");
        for alg in &ALL_ALGORITHMS {
            if ui
                .selectable_label(self.algorithm == *alg, alg.label())
                .clicked()
                && self.algorithm != *alg
            {
                self.algorithm = *alg;
                self.needs_recompute = true;
            }
        }

        ui.separator();
        ui.label("Block Size");
        let standard_sizes = [64, 128, 256, 512, 1024, 2048, 4096, 8192];
        let block_is_standard = standard_sizes.contains(&self.block_size);
        for &bs in &standard_sizes {
            if ui
                .selectable_label(self.block_size == bs, format!("{bs}"))
                .clicked()
                && self.block_size != bs
            {
                self.block_size = bs;
                self.custom_block_input.clear();
                self.needs_recompute = true;
            }
        }
        ui.horizontal(|ui| {
            ui.label("Custom:");
            let re = ui.add(
                egui::TextEdit::singleline(&mut self.custom_block_input)
                    .desired_width(60.0)
                    .hint_text(if !block_is_standard {
                        format!("{}", self.block_size)
                    } else {
                        String::new()
                    }),
            );
            if re.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Ok(v) = self.custom_block_input.trim().parse::<usize>() {
                    if v > 0 && v != self.block_size {
                        self.block_size = v;
                        self.needs_recompute = true;
                    }
                }
            }
        });

        ui.separator();
        ui.label("Step Size");
        let step_is_standard = standard_sizes.contains(&self.step);
        for &s in &standard_sizes {
            if ui
                .selectable_label(self.step == s, format!("{s}"))
                .clicked()
                && self.step != s
            {
                self.step = s;
                self.custom_step_input.clear();
                self.needs_recompute = true;
            }
        }
        ui.horizontal(|ui| {
            ui.label("Custom:");
            let re = ui.add(
                egui::TextEdit::singleline(&mut self.custom_step_input)
                    .desired_width(60.0)
                    .hint_text(if !step_is_standard {
                        format!("{}", self.step)
                    } else {
                        String::new()
                    }),
            );
            if re.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if let Ok(v) = self.custom_step_input.trim().parse::<usize>() {
                    if v > 0 && v != self.step {
                        self.step = v;
                        self.needs_recompute = true;
                    }
                }
            }
        });

        ui.separator();
        ui.label("Theme");
        let theme_count = self.themes.len();
        for i in 0..theme_count {
            let selected = self.theme_index == i;
            let name = self.themes[i].name.clone();
            if ui.selectable_label(selected, &name).clicked() {
                self.theme_index = i;
            }
            if selected {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for &(_, color) in &self.themes[i].bands {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(16.0, 12.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, color);
                    }
                });
            }
        }
        if let Some(dir) = themes_dir() {
            ui.label(
                egui::RichText::new(format!("Themes: {}", dir.display()))
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        }

        ui.separator();
        ui.heading("Stats");
        ui.label(format!("Points: {}", data.points.len()));
        ui.label(format!("Min: {:.3}", data.min));
        ui.label(format!("Max: {:.3}", data.max));
        ui.label(format!("Avg: {:.3}", data.avg));
    }
}
