//! Every widget on one page, with a theme switcher.
//!
//! ```bash
//! cargo run -p egui_mev_widgets --example gallery --all-features
//! ```

#[path = "../tests/support/gallery_ui.rs"]
mod gallery_ui;

use egui_mev_theme::{FontOptions, IconFont, Theme, fonts, presets};
use egui_mev_widgets::segmented;
use gallery_ui::{GalleryState, gallery_ui};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    GraphiteDark,
    GraphiteLight,
    SlateDark,
}

impl Look {
    fn install(self, ctx: &egui::Context) {
        match self {
            Self::GraphiteDark => {
                Theme::new(presets::GRAPHITE_DARK, presets::GRAPHITE_LIGHT).install(ctx);
                ctx.set_theme(egui::Theme::Dark);
            }
            Self::GraphiteLight => {
                Theme::new(presets::GRAPHITE_DARK, presets::GRAPHITE_LIGHT).install(ctx);
                ctx.set_theme(egui::Theme::Light);
            }
            Self::SlateDark => Theme::dark_only(presets::SLATE_DARK).install(ctx),
        }
    }
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
                let changed = segmented(
                    ui,
                    "look",
                    &mut self.look,
                    &[
                        (Look::GraphiteDark, "Graphite dark"),
                        (Look::GraphiteLight, "Graphite light"),
                        (Look::SlateDark, "Slate dark"),
                    ],
                );
                if changed {
                    self.look.install(ui.ctx());
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
            look.install(&cc.egui_ctx);
            fonts::install_system(
                &cc.egui_ctx,
                &FontOptions {
                    icons: vec![IconFont::Lucide],
                    ..Default::default()
                },
            );
            Ok(Box::new(Gallery {
                look,
                state: GalleryState::default(),
            }))
        }),
    )
}
