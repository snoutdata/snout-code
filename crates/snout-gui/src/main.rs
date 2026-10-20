mod app;
mod art;
mod diff_view;
mod graph;
mod terminal;
mod theme;

fn main() -> eframe::Result {
    // `snout [path]` opens (and remembers) the repository containing `path`.
    let open_arg = std::env::args_os().nth(1).map(std::path::PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("snout")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native("snout", options, Box::new(move |cc| Ok(Box::new(app::SnoutApp::new(cc, open_arg)))))
}
