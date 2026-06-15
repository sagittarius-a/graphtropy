use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use memmap2::Mmap;

mod app;
mod entropy;
mod export;
mod hexview;
mod options;
mod plot;

#[derive(Parser)]
#[command(name = "graphtropy", about = "Interactive binary entropy visualizer")]
struct Cli {
    /// Path to binary file (opens file dialog if omitted)
    file: Option<PathBuf>,

    /// Block size in bytes
    #[arg(short, long, default_value_t = 256)]
    block_size: usize,

    /// Step size in bytes (defaults to block_size)
    #[arg(short, long)]
    step: Option<usize>,

    /// Export entropy graph to PNG and exit (no GUI)
    #[arg(short, long)]
    export: Option<PathBuf>,

    /// Image width for --export
    #[arg(long, default_value_t = 1920)]
    width: u32,

    /// Image height for --export
    #[arg(long, default_value_t = 600)]
    height: u32,

    /// Hide file name from export caption
    #[arg(long)]
    no_filename: bool,

    /// Hide algorithm from export caption
    #[arg(long)]
    no_algorithm: bool,

    /// Hide block/step sizes from export caption
    #[arg(long)]
    no_sizes: bool,

    /// Hide entire caption from export
    #[arg(long)]
    no_caption: bool,

    /// Theme name for export (e.g. "Dark", "Light")
    #[arg(short, long, default_value = "Dark")]
    theme: String,
}

const MAX_POINTS: usize = 4096;
const STANDARD_SIZES: [usize; 8] = [64, 128, 256, 512, 1024, 2048, 4096, 8192];

pub fn auto_adapt(file_size: usize, block_size: usize, step: usize) -> (usize, usize, bool) {
    let num_points = file_size / step.max(1);
    if num_points <= MAX_POINTS {
        return (block_size, step, false);
    }
    // Pick the smallest standard step that keeps points <= MAX_POINTS
    for &s in &STANDARD_SIZES {
        if s >= step && file_size / s <= MAX_POINTS {
            return (block_size, s, true);
        }
    }
    // For very large files, compute step to stay under MAX_POINTS.
    // Keep block_size as-is so we only read small samples, not the entire file.
    let new_step = file_size.div_ceil(MAX_POINTS);
    (block_size, new_step, true)
}

fn app_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../images/icon.png"))
        .expect("embedded application icon must be a valid PNG")
}

fn main() -> eframe::Result {
    let cli = Cli::parse();

    let file_path = match cli.file {
        Some(p) => p,
        None => {
            let picked = rfd::FileDialog::new()
                .set_title("Open file")
                .pick_file();
            match picked {
                Some(p) => p,
                None => std::process::exit(0),
            }
        }
    };

    let file = File::open(&file_path).unwrap_or_else(|e| {
        eprintln!("Error opening {}: {e}", file_path.display());
        std::process::exit(1);
    });

    let mmap = unsafe { Mmap::map(&file) }.unwrap_or_else(|e| {
        eprintln!("Error mapping file: {e}");
        std::process::exit(1);
    });

    if mmap.is_empty() {
        eprintln!("File is empty");
        std::process::exit(1);
    }

    let user_step = cli.step.unwrap_or(cli.block_size);

    // Headless export: compute synchronously, no GUI
    if let Some(export_path) = &cli.export {
        let (block_size, step, _) = auto_adapt(mmap.len(), cli.block_size, user_step);
        let _ = mmap.advise(memmap2::Advice::Sequential);
        let entropy_data = entropy::compute(&mmap, block_size, step, entropy::Algorithm::Shannon);
        let _ = mmap.advise(memmap2::Advice::Normal);

        let themes = options::load_themes();
        let theme = themes.iter()
            .find(|t| t.name.eq_ignore_ascii_case(&cli.theme))
            .unwrap_or(&themes[0]);
        let (y_min, y_max) = entropy::Algorithm::Shannon.y_range();

        let caption = if cli.no_caption {
            None
        } else {
            let mut parts = Vec::new();
            if !cli.no_filename {
                let name = file_path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                parts.push(name);
            }
            if !cli.no_algorithm {
                parts.push(entropy::Algorithm::Shannon.label().to_string());
            }
            if !cli.no_sizes {
                parts.push(format!("block={block_size} step={step}"));
            }
            if parts.is_empty() { None } else { Some(parts.join(" | ")) }
        };

        let path_str = export_path.to_string_lossy();
        match export::render_to_png(
            &path_str,
            &entropy_data,
            theme,
            y_min, y_max,
            mmap.len() as u64,
            caption.as_deref(),
            "Offset",
            entropy::Algorithm::Shannon.y_label(),
            cli.width, cli.height,
        ) {
            Ok(()) => {
                eprintln!("Exported to {}", export_path.display());
                return Ok(());
            }
            Err(e) => {
                eprintln!("Export failed: {e}");
                std::process::exit(1);
            }
        }
    }

    // GUI mode: compute in background, show spinner
    let file_info = app::FileInfo {
        path: file_path.clone(),
        size: mmap.len() as u64,
        block_size: cli.block_size,
        step: user_step,
    };

    let mmap = Arc::new(mmap);
    let title = format!("graphtropy - {}", file_path.display());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 1000.0])
            // eframe sets its default icon at runtime if none is provided.
            .with_icon(app_icon()),
        ..Default::default()
    };

    eframe::run_native(
        &title,
        options,
        Box::new(move |_cc| Ok(Box::new(app::App::new(mmap, file_info)))),
    )
}
