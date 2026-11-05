mod app;

use anyhow::Result;
use clap::Parser;
use eframe::egui;
use filespring::Engine;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "FileSpring native desktop prototype")]
struct Args {
    #[arg(long)]
    root: Vec<PathBuf>,
    #[arg(long)]
    state: Option<PathBuf>,
    /// Close automatically after N seconds for a native-window smoke test.
    #[arg(long)]
    smoke_seconds: Option<u64>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let roots = if args.root.is_empty() {
        let Some(root) = rfd::FileDialog::new()
            .set_title("FileSpring · 选择工作目录 / Choose workspace")
            .pick_folder()
        else {
            return Ok(());
        };
        vec![root]
    } else {
        args.root
    };
    let state = args.state.unwrap_or_else(|| roots[0].join(".filespring"));
    let engine = Engine::open(&state, &roots)?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1240.0, 800.0])
            .with_min_inner_size([960.0, 640.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "FileSpring",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, engine, args.smoke_seconds)))),
    )
    .map_err(|e| anyhow::anyhow!("Native window: {e}"))
}
