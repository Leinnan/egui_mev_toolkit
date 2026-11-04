use egui::{Align, CornerRadius, Layout, Sense, Stroke, UiBuilder};
use egui_mev_theme::{Metrics, Palette};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonGroupElement {
    First,
    Last,
    InTheMiddle,
}

impl ButtonGroupElement {
    pub const fn from_index(index: usize, total: usize) -> Self {
        match index {
            0 => Self::First,
            n if n == total - 1 => Self::Last,
            _ => Self::InTheMiddle,
        }
    }

    pub const fn to_corner_radius(self, corner_size: u8) -> CornerRadius {
        match self {
            Self::First => CornerRadius {
                nw: corner_size,
                ne: 0,
                sw: corner_size,
                se: 0,
            },
            Self::Last => CornerRadius {
                nw: 0,
                ne: corner_size,
                sw: 0,
                se: corner_size,
            },
            Self::InTheMiddle => CornerRadius {
                nw: 0,
                ne: 0,
                sw: 0,
                se: 0,
            },
        }
    }
}

pub trait UiBuilderExt {
    fn btn_frame(&self) -> egui::Frame;
    fn btn_frame_ui(&self) -> UiBuilder;
}
impl UiBuilderExt for egui::Ui {
    fn btn_frame(&self) -> egui::Frame {
        let palette = Palette::of(self);
        egui::Frame::new()
            .corner_radius(Metrics::of(self).control_radius)
            .inner_margin(2)
            .fill(palette.control_track)
            .stroke(Stroke::new(1.0, palette.border))
    }

    fn btn_frame_ui(&self) -> UiBuilder {
        UiBuilder::new()
            .sense(Sense::CLICK)
            .layout(Layout::left_to_right(Align::Min))
    }
}
