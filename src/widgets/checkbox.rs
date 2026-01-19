use crate::widgets::{Animation, DEFAULT_SPRING};
use egui::{self, *};
use std::time::{Duration, Instant};

use super::theming::Colors;

#[derive(Default)]
pub struct Checkbox {
    pub enabled: bool,
    pub colors: Colors,
    pub animation: Animation,
    /// Size of the toggle switch
    size: Vec2,
    pub highlight: bool,
}

impl Checkbox {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            colors: Colors::default(),
            animation: Animation::new(Duration::from_millis(1000), Some(DEFAULT_SPRING)),
            size: Vec2::new(25.0, 25.0),
            highlight: false,
        }
    }
    /// Update animation state
    fn update_animation(&mut self, ctx: &Context) {
        self.animation.update();
        ctx.request_repaint();
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        self.animation.start(Instant::now());
        self.animation.set_values((
            if self.enabled { 1.0 } else { 0.0 },
            self.animation.progress,
        ));
    }

    pub fn set(&mut self, enabled: bool) {
        if enabled != self.enabled {
            self.toggle();
        }
    }

    pub fn display(&mut self, ui: &mut Ui, text: impl Into<String>, color: Color32) {
        let inner = ui.horizontal(|ui| {
            ui.label(
                RichText::new(text)
                    .color(Color32::GRAY.lerp_to_gamma(Color32::WHITE, self.animation.progress)),
            );
            ui.add_space(90.0);
            self.colors = Colors::default().set_on_color(color);
            self.show(ui);
        });
        if inner.response.hovered() {
            self.highlight = false;
        }
        if self.highlight {
            ui.painter().rect(
                inner.response.rect.expand(5.0),
                3.0,
                Color32::from_rgba_unmultiplied(255, 255, 255, 4),
                (0.0, Color32::TRANSPARENT),
            );
        }
    }

    pub fn show(&mut self, ui: &mut Ui) -> Response {
        self.update_animation(ui.ctx());

        let (rect, mut response) = ui.allocate_exact_size(self.size, Sense::click_and_drag()); // block window moving when user drags button

        if response.clicked() {
            self.toggle();
            response.mark_changed();
            ui.ctx().request_repaint();
        }

        if ui.is_rect_visible(rect) {
            let radius = rect.height() * 0.2;
            let progress = self.animation.progress;

            // Interpolate background color based on progress
            let background_color = self
                .colors
                .background_off
                .lerp_to_gamma(self.colors.background_on, progress);

            // Draw the checkbox background
            ui.painter()
                .rect_filled(rect, Rounding::same(radius), background_color);
        }

        response
    }
}
