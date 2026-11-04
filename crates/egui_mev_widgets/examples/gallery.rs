//! Every widget on one page, with a theme switcher.
//!
//! ```bash
//! cargo run -p egui_mev_widgets --example gallery --all-features
//! ```

#[path = "../tests/support/gallery_ui.rs"]
mod gallery_ui;

use egui_mev_theme::{FontOptions, IconFont};
use egui_mev_widgets::segmented;
use gallery_ui::{GalleryState, Look, gallery_ui};

fn install(look: Look, ctx: &egui::Context) {
    look.theme().install_with_fonts(
        ctx,
        &FontOptions {
            icons: vec![IconFont::Lucide],
            ..Default::default()
        },
    );
    ctx.set_theme(look.mode());
    // Fitted text belongs to the fonts it was laid out with.
    egui_mev_widgets::text_fit::clear_pool();
}

struct Gallery {
    look: Look,
    state: GalleryState,
}

impl eframe::App for Gallery {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("look").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Theme");
                let options = Look::ALL.map(|look| (look, look.label()));
                if segmented(ui, "look", &mut self.look, &options) {
                    install(self.look, ui.ctx());
                }
            });
            ui.add_space(4.0);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            gallery_ui(ui, &mut self.state);
        });
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "egui_mev_widgets gallery",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 860.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            let look = Look::GraphiteDark;
            install(look, &cc.egui_ctx);
            Ok(Box::new(Gallery {
                look,
                state: GalleryState::default(),
            }))
        }),
    )
}
