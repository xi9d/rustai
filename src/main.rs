// main.rs
use eframe::egui;

mod models;
mod ollama;
mod rag;
mod analytics;
mod ui;
mod file_handler;

use crate::ui::TouristApp;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "TouristXi9d - Enhanced AI Client with RAG & Analytics",
        options,
        Box::new(|_cc| Ok(Box::new(TouristApp::default()))),
    )
}