use super::theming::Colors;
use egui::{self, Color32};
use std::time::Instant;

pub struct SmoothSlider {
    pub value: f32,
    pub colors: Colors,
    target_value: f32,
    pub min: f32,
    pub max: f32,
    animation_start_time: Option<Instant>,
    animation_start_value: f32,
    width: f32,
    height: f32,
    pub highlight: bool,
}

impl SmoothSlider {
    pub fn new(initial_value: f32, min: f32, max: f32) -> Self {
        Self {
            value: initial_value,
            colors: Colors::default(),
            target_value: initial_value,
            min,
            max,
            animation_start_time: None,
            animation_start_value: initial_value,
            width: 140.0,
            height: 20.0,
            highlight: false,
        }
    }

    pub fn colors(mut self, colors: Colors) -> Self {
        self.colors = colors;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn display(
        &mut self,
        ui: &mut egui::Ui,
        text: impl Into<String>,
        tooltip: Option<impl Into<String>>,
        accent_color: Color32,
    ) {
        let inner = ui.horizontal(|ui| {
            let label = ui.label(text.into());
            if let Some(tooltip) = tooltip {
                label.on_hover_text_at_pointer(tooltip.into());
            }

            self.colors = Colors::default().set_on_color(accent_color);
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

    pub fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        // Update animation
        self.update_animation();

        // Reserve space for the slider
        let desired_size = egui::vec2(self.width, self.height);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

        // Handle interactions
        self.handle_input(&response, rect);

        // Draw the slider
        self.draw_slider(ui, rect, &response);

        // Request repaint for smooth animation
        ui.ctx().request_repaint();

        response
    }

    fn handle_input(&mut self, response: &egui::Response, rect: egui::Rect) {
        if response.clicked() || response.dragged() {
            // User clicked on the slider - animate to click position
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                let relative_pos = (pointer_pos.x - rect.min.x - 10.0) / (rect.width() - 20.0);
                let clicked_value = self.min + relative_pos * (self.max - self.min);
                self.set_target_smooth(clicked_value.clamp(self.min, self.max));
            }
        }
    }
    pub fn set_value(&mut self, value: f32) {
        self.set_target_smooth(value.clamp(self.min, self.max));
    }

    fn draw_slider(&self, ui: &mut egui::Ui, rect: egui::Rect, response: &egui::Response) {
        let visuals = ui.style().interact(response);
        let painter = ui.painter();

        // Draw track background
        let track_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x + 10.0, rect.center().y - 2.0),
            egui::vec2(rect.width() - 20.0, 8.0),
        );

        painter.rect_filled(
            track_rect,
            egui::Rounding::same(2.0),
            self.colors.background_off,
        );

        // Draw filled track (from min to current value)
        let value_ratio = (self.value - self.min) / (self.max - self.min);
        let filled_width = track_rect.width() * value_ratio;

        if filled_width > 0.0 {
            let filled_rect = egui::Rect::from_min_size(
                track_rect.min,
                egui::vec2(filled_width, track_rect.height()),
            );

            painter.rect_filled(
                filled_rect,
                egui::Rounding::same(2.0),
                self.colors.background_on,
            );
        }

        // Draw handle
        let handle_x = track_rect.min.x + value_ratio * track_rect.width();
        let handle_center = egui::pos2(handle_x, track_rect.center().y);
        let handle_radius = 6.0;

        // Handle shadow/glow effect
        painter.circle_filled(
            handle_center,
            handle_radius + 2.0,
            egui::Color32::from_black_alpha(30),
        );

        painter.circle_filled(
            handle_center,
            handle_radius,
            ui.style().visuals.widgets.active.fg_stroke.color,
        );

        // Draw value text
        let value_text = format!("{:.2}", self.value);
        let value_pos = egui::pos2(rect.max.x + 10.0, rect.center().y);
        painter.text(
            value_pos,
            egui::Align2::LEFT_CENTER,
            value_text,
            egui::FontId::default(),
            visuals.text_color(),
        );
    }

    fn set_target_smooth(&mut self, new_target: f32) {
        if (self.target_value - new_target).abs() > 0.001 {
            self.animation_start_time = Some(Instant::now());
            self.animation_start_value = self.value;
            self.target_value = new_target;
        }
    }

    fn update_animation(&mut self) {
        if let Some(start_time) = self.animation_start_time {
            // Smooth easing animation
            let elapsed = Instant::now().duration_since(start_time).as_secs_f32();
            let duration = 0.25; // Animation duration in seconds

            if elapsed >= duration {
                // Animation complete
                self.value = self.target_value;
                self.animation_start_time = None;
            } else {
                // Smooth easing function (ease-out cubic)
                let t = elapsed / duration;
                let eased_t = 1.0 - (1.0 - t).powi(3);

                self.value = self.animation_start_value
                    + (self.target_value - self.animation_start_value) * eased_t;
            }
        }
    }
}
