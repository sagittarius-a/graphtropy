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

fn lighten(c: Color32, amount: u8) -> Color32 {
    Color32::from_rgb(
        c.r().saturating_add(amount),
        c.g().saturating_add(amount),
        c.b().saturating_add(amount),
    )
}

fn darken(c: Color32, amount: u8) -> Color32 {
    Color32::from_rgb(
        c.r().saturating_sub(amount),
        c.g().saturating_sub(amount),
        c.b().saturating_sub(amount),
    )
}

#[derive(Clone)]
pub struct HexPalette {
    pub null: Color32,
    pub whitespace: Color32,
    pub printable: Color32,
    pub control: Color32,
    pub non_ascii: Color32,
}

#[derive(Clone)]
pub struct ColorTheme {
    pub name: String,
    pub bands: Vec<(f64, Color32)>,
    pub bg: Color32,
    pub text: Color32,
    pub grid: Color32,
    pub caption: Color32,
    pub error: Color32,
    pub dark: bool,
    pub hex: HexPalette,
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

    pub fn to_visuals(&self) -> egui::Visuals {
        let mut v = if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        v.panel_fill = self.bg;
        v.window_fill = self.bg;
        v.override_text_color = Some(self.text);

        if self.dark {
            v.extreme_bg_color = darken(self.bg, 10);
            v.faint_bg_color = lighten(self.bg, 5);
            v.widgets.inactive.bg_fill = lighten(self.bg, 20);
            v.widgets.hovered.bg_fill = lighten(self.bg, 35);
            v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, lighten(self.grid, 20));
            v.widgets.active.bg_fill = lighten(self.bg, 45);
        } else {
            v.extreme_bg_color = lighten(self.bg, 10);
            v.faint_bg_color = darken(self.bg, 5);
            v.widgets.inactive.bg_fill = darken(self.bg, 15);
            v.widgets.hovered.bg_fill = darken(self.bg, 25);
            v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, darken(self.grid, 20));
            v.widgets.active.bg_fill = darken(self.bg, 35);
        }

        v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, self.grid);
        v.widgets.noninteractive.bg_fill = self.bg;
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, self.grid);
        v.window_stroke = egui::Stroke::new(1.0, self.grid);

        v
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
            error: Color32::from_rgb(255, 80, 80),
            dark: true,
            hex: HexPalette {
                null: Color32::from_rgb(90, 90, 90),
                whitespace: Color32::from_rgb(80, 170, 255),
                printable: Color32::from_rgb(95, 220, 95),
                control: Color32::from_rgb(190, 115, 255),
                non_ascii: Color32::from_rgb(255, 120, 80),
            },
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
            error: Color32::from_rgb(200, 30, 30),
            dark: false,
            hex: HexPalette {
                null: Color32::from_rgb(140, 140, 140),
                whitespace: Color32::from_rgb(20, 95, 190),
                printable: Color32::from_rgb(20, 135, 45),
                control: Color32::from_rgb(125, 70, 185),
                non_ascii: Color32::from_rgb(190, 65, 30),
            },
        }
    }

    fn gruvbox_dark() -> Self {
        ColorTheme {
            name: "Gruvbox Dark".to_string(),
            bands: vec![
                (0.0, Color32::from_rgb(69, 133, 136)),
                (0.25, Color32::from_rgb(152, 151, 26)),
                (0.625, Color32::from_rgb(214, 93, 14)),
                (0.875, Color32::from_rgb(204, 36, 29)),
            ],
            bg: Color32::from_rgb(40, 40, 40),
            text: Color32::from_rgb(189, 174, 147),
            grid: Color32::from_rgb(80, 73, 69),
            caption: Color32::from_rgb(235, 219, 178),
            error: Color32::from_rgb(251, 73, 52),
            dark: true,
            hex: HexPalette {
                null: Color32::from_rgb(102, 92, 84),
                whitespace: Color32::from_rgb(131, 165, 152),
                printable: Color32::from_rgb(184, 187, 38),
                control: Color32::from_rgb(211, 134, 155),
                non_ascii: Color32::from_rgb(254, 128, 25),
            },
        }
    }

    fn gruvbox_light() -> Self {
        ColorTheme {
            name: "Gruvbox Light".to_string(),
            bands: vec![
                (0.0, Color32::from_rgb(7, 102, 120)),
                (0.25, Color32::from_rgb(121, 116, 14)),
                (0.625, Color32::from_rgb(175, 58, 3)),
                (0.875, Color32::from_rgb(157, 0, 6)),
            ],
            bg: Color32::from_rgb(251, 241, 199),
            text: Color32::from_rgb(80, 73, 69),
            grid: Color32::from_rgb(213, 196, 161),
            caption: Color32::from_rgb(60, 56, 54),
            error: Color32::from_rgb(204, 36, 29),
            dark: false,
            hex: HexPalette {
                null: Color32::from_rgb(168, 153, 132),
                whitespace: Color32::from_rgb(7, 102, 120),
                printable: Color32::from_rgb(121, 116, 14),
                control: Color32::from_rgb(143, 63, 113),
                non_ascii: Color32::from_rgb(175, 58, 3),
            },
        }
    }

}

#[derive(Deserialize)]
struct ThemeFile {
    name: String,
    bands: Vec<BandEntry>,
    bg: Option<String>,
    text: Option<String>,
    grid: Option<String>,
    caption: Option<String>,
    error: Option<String>,
    dark: Option<bool>,
    hex: Option<HexPaletteFile>,
}

#[derive(Deserialize)]
struct BandEntry {
    threshold: f64,
    color: String,
}

#[derive(Deserialize)]
struct HexPaletteFile {
    null: Option<String>,
    whitespace: Option<String>,
    printable: Option<String>,
    control: Option<String>,
    non_ascii: Option<String>,
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
dark = true
bg = "#1E1E1E"
text = "#A0A0A0"
grid = "#323232"
caption = "#C8C8C8"

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
dark = false
bg = "#F5F5F5"
text = "#3C3C3C"
grid = "#C8C8C8"
caption = "#282828"

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

    let gruvbox_dark_path = dir.join("gruvbox-dark.toml");
    if !gruvbox_dark_path.exists() {
        let _ = fs::write(
            gruvbox_dark_path,
            r##"name = "Gruvbox Dark"
dark = true
bg = "#282828"
text = "#BDAE93"
grid = "#504945"
caption = "#EBDBB2"

[[bands]]
threshold = 0.0
color = "#458588"

[[bands]]
threshold = 0.25
color = "#98971A"

[[bands]]
threshold = 0.625
color = "#D65D0E"

[[bands]]
threshold = 0.875
color = "#CC241D"
"##,
        );
    }

    let gruvbox_light_path = dir.join("gruvbox-light.toml");
    if !gruvbox_light_path.exists() {
        let _ = fs::write(
            gruvbox_light_path,
            r##"name = "Gruvbox Light"
dark = false
bg = "#FBF1C7"
text = "#504945"
grid = "#D5C4A1"
caption = "#3C3836"

[[bands]]
threshold = 0.0
color = "#076678"

[[bands]]
threshold = 0.25
color = "#79740E"

[[bands]]
threshold = 0.625
color = "#AF3A03"

[[bands]]
threshold = 0.875
color = "#9D0006"
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
    let mut themes = vec![
        ColorTheme::dark(),
        ColorTheme::light(),
        ColorTheme::gruvbox_dark(),
        ColorTheme::gruvbox_light(),
    ];

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
                    if themes.iter().any(|t| t.name == tf.name) {
                        continue;
                    }
                    let bands: Vec<(f64, Color32)> = tf
                        .bands
                        .iter()
                        .filter_map(|b| parse_hex_color(&b.color).map(|c| (b.threshold, c)))
                        .collect();
                    if !bands.is_empty() {
                        let dark = tf.dark.unwrap_or(true);
                        let base = if dark { ColorTheme::dark() } else { ColorTheme::light() };
                        let hex_file = tf.hex.as_ref();
                        themes.push(ColorTheme {
                            name: tf.name,
                            bands,
                            bg: tf.bg.as_deref().and_then(parse_hex_color).unwrap_or(base.bg),
                            text: tf.text.as_deref().and_then(parse_hex_color).unwrap_or(base.text),
                            grid: tf.grid.as_deref().and_then(parse_hex_color).unwrap_or(base.grid),
                            caption: tf.caption.as_deref().and_then(parse_hex_color).unwrap_or(base.caption),
                            error: tf.error.as_deref().and_then(parse_hex_color).unwrap_or(base.error),
                            dark,
                            hex: HexPalette {
                                null: hex_file.and_then(|h| h.null.as_deref()).and_then(parse_hex_color).unwrap_or(base.hex.null),
                                whitespace: hex_file.and_then(|h| h.whitespace.as_deref()).and_then(parse_hex_color).unwrap_or(base.hex.whitespace),
                                printable: hex_file.and_then(|h| h.printable.as_deref()).and_then(parse_hex_color).unwrap_or(base.hex.printable),
                                control: hex_file.and_then(|h| h.control.as_deref()).and_then(parse_hex_color).unwrap_or(base.hex.control),
                                non_ascii: hex_file.and_then(|h| h.non_ascii.as_deref()).and_then(parse_hex_color).unwrap_or(base.hex.non_ascii),
                            },
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
    pub hex_byte_colors: bool,
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
            hex_byte_colors: true,
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
        let current_name = self.themes[self.theme_index].name.clone();
        egui::ComboBox::from_id_salt("theme_selector")
            .selected_text(&current_name)
            .width(ui.available_width() - 8.0)
            .show_ui(ui, |ui| {
                for i in 0..self.themes.len() {
                    let name = self.themes[i].name.clone();
                    ui.selectable_value(&mut self.theme_index, i, &name);
                }
            });
        ui.checkbox(&mut self.hex_byte_colors, "Hex byte colors");
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
