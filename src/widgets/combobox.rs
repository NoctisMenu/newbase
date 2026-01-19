use egui::{Color32, Id, Key, Pos2, Response, Rounding, Sense, Stroke, Ui, Vec2};
use std::time::Instant;

#[derive(Clone)]
pub struct ComboBoxColors {
    pub background: Color32,
    pub background_hovered: Color32,
    pub background_open: Color32,
    pub text: Color32,
    pub border: Color32,
    pub border_focused: Color32,
    pub dropdown_background: Color32,
    pub item_hovered: Color32,
}

impl Default for ComboBoxColors {
    fn default() -> Self {
        Self {
            background: Color32::from_rgba_unmultiplied(0, 0, 0, 70),
            background_hovered: Color32::from_rgba_unmultiplied(20, 20, 20, 80),
            background_open: Color32::from_rgba_unmultiplied(30, 30, 30, 90),
            text: Color32::WHITE,
            border: Color32::from_rgb(60, 60, 60),
            border_focused: Color32::from_rgb(100, 100, 100),
            dropdown_background: Color32::from_rgba_unmultiplied(0, 0, 0, 90),
            item_hovered: Color32::from_rgba_unmultiplied(40, 40, 40, 255),
        }
    }
}

pub struct ComboBox {
    id: String,
    selected_index: usize,
    pub options: Vec<String>,
    width: f32,
    height: f32,
    pub colors: ComboBoxColors,
    last_interaction: Option<Instant>,
}

impl ComboBox {
    pub fn new(id: impl Into<String>, options: Vec<String>, selected_index: usize) -> Self {
        Self {
            id: id.into(),
            selected_index: selected_index.min(options.len().saturating_sub(1)),
            options,
            width: 140.0,
            height: 24.0,
            colors: ComboBoxColors::default(),
            last_interaction: None,
        }
    }

    pub fn with_width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn with_height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn with_colors(mut self, colors: ComboBoxColors) -> Self {
        self.colors = colors;
        self
    }

    pub fn with_accent_color(mut self, accent: Color32) -> Self {
        self.colors.border_focused = accent;
        self.colors.item_hovered = accent.linear_multiply(0.3);
        self
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn selected_value(&self) -> Option<&str> {
        self.options.get(self.selected_index).map(|s| s.as_str())
    }

    pub fn set_selected_index(&mut self, index: usize) {
        if index < self.options.len() {
            self.selected_index = index;
            self.last_interaction = Some(Instant::now());
        }
    }

    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let id = Id::new(&self.id);
        let popup_id = id.with("popup");

        // Allocate space for the combobox button
        let (button_rect, mut response) =
            ui.allocate_exact_size(Vec2::new(self.width, self.height), Sense::click());

        // Track if popup is open
        let is_open = ui.memory(|mem| mem.is_popup_open(popup_id));

        // Handle button click
        if response.clicked() {
            ui.memory_mut(|mem| {
                if is_open {
                    mem.close_popup();
                } else {
                    mem.open_popup(popup_id);
                }
            });
            response.mark_changed();
        }

        // Determine button appearance
        let background_color = if is_open {
            self.colors.background_open
        } else if response.hovered() {
            self.colors.background_hovered
        } else {
            self.colors.background
        };

        let border_color = if is_open {
            self.colors.border_focused
        } else {
            self.colors.border
        };

        // Draw the combobox button
        if ui.is_rect_visible(button_rect) {
            let painter = ui.painter();

            // Background
            painter.rect_filled(button_rect, Rounding::same(4.0), background_color);

            // Border
            painter.rect_stroke(
                button_rect,
                Rounding::same(4.0),
                Stroke::new(1.0, border_color),
            );

            // Selected text
            let selected_text = self
                .options
                .get(self.selected_index)
                .map(|s| s.as_str())
                .unwrap_or("");

            let text_pos = Pos2::new(button_rect.left() + 8.0, button_rect.center().y - 6.0);

            painter.text(
                text_pos,
                egui::Align2::LEFT_TOP,
                selected_text,
                egui::FontId::proportional(12.0),
                self.colors.text,
            );

            // Draw dropdown arrow
            let arrow_center = Pos2::new(button_rect.right() - 12.0, button_rect.center().y);

            let arrow_size = 4.0;
            let arrow_points = if is_open {
                // Up arrow
                [
                    Pos2::new(
                        arrow_center.x - arrow_size,
                        arrow_center.y + arrow_size / 2.0,
                    ),
                    Pos2::new(arrow_center.x, arrow_center.y - arrow_size / 2.0),
                    Pos2::new(
                        arrow_center.x + arrow_size,
                        arrow_center.y + arrow_size / 2.0,
                    ),
                ]
            } else {
                // Down arrow
                [
                    Pos2::new(
                        arrow_center.x - arrow_size,
                        arrow_center.y - arrow_size / 2.0,
                    ),
                    Pos2::new(arrow_center.x, arrow_center.y + arrow_size / 2.0),
                    Pos2::new(
                        arrow_center.x + arrow_size,
                        arrow_center.y - arrow_size / 2.0,
                    ),
                ]
            };

            painter.add(egui::Shape::convex_polygon(
                arrow_points.to_vec(),
                self.colors.text,
                Stroke::NONE,
            ));
        }

        // Show popup if open
        if is_open {
            let mut selected_this_frame = None;
            let mut close_popup = false;

            let area_response = egui::Area::new(popup_id)
                .order(egui::Order::Foreground)
                .fixed_pos(button_rect.left_bottom() + Vec2::new(0.0, 2.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::none()
                        .fill(self.colors.dropdown_background)
                        .rounding(Rounding::same(4.0))
                        .stroke(Stroke::new(1.0, self.colors.border_focused))
                        .inner_margin(4.0)
                        .show(ui, |ui| {
                            ui.set_min_width(self.width);
                            ui.set_max_width(self.width);

                            // Handle keyboard navigation
                            let up_pressed = ui.input(|input| input.key_pressed(Key::ArrowUp));
                            let down_pressed = ui.input(|input| input.key_pressed(Key::ArrowDown));
                            let enter_pressed = ui.input(|input| input.key_pressed(Key::Enter));
                            let escape_pressed = ui.input(|input| input.key_pressed(Key::Escape));

                            if up_pressed && self.selected_index > 0 {
                                self.selected_index -= 1;
                            }
                            if down_pressed && self.selected_index < self.options.len() - 1 {
                                self.selected_index += 1;
                            }
                            if enter_pressed || escape_pressed {
                                close_popup = true;
                            }

                            // Render options
                            for (i, option) in self.options.iter().enumerate() {
                                let is_selected = i == self.selected_index;
                                let (item_rect, item_response) = ui.allocate_exact_size(
                                    Vec2::new(self.width - 8.0, self.height - 4.0),
                                    Sense::click(),
                                );

                                if item_response.clicked() {
                                    selected_this_frame = Some(i);
                                    close_popup = true;
                                }

                                // Draw item background if hovered or selected
                                if item_response.hovered() {
                                    ui.painter().rect_filled(
                                        item_rect,
                                        Rounding::same(2.0),
                                        self.colors.item_hovered,
                                    );
                                } else if is_selected {
                                    ui.painter().rect_filled(
                                        item_rect,
                                        Rounding::same(2.0),
                                        self.colors.border,
                                    );
                                }

                                // Draw text
                                let text_pos =
                                    Pos2::new(item_rect.left() + 6.0, item_rect.center().y - 6.0);

                                ui.painter().text(
                                    text_pos,
                                    egui::Align2::LEFT_TOP,
                                    option,
                                    egui::FontId::proportional(12.0),
                                    self.colors.text,
                                );
                            }
                        });
                });

            // Update selected index if an item was clicked
            if let Some(new_index) = selected_this_frame {
                if new_index != self.selected_index {
                    self.selected_index = new_index;
                    self.last_interaction = Some(Instant::now());
                    response.mark_changed();
                }
            }

            // Close popup if needed
            if close_popup {
                ui.memory_mut(|mem| mem.close_popup());
            }

            // Close popup if clicked outside
            if ui.input(|i| i.pointer.any_click()) {
                let pointer_pos = ui.input(|i| i.pointer.interact_pos());
                if let Some(pos) = pointer_pos {
                    // Check if click is outside both button and popup
                    let clicked_outside =
                        !button_rect.contains(pos) && !area_response.response.rect.contains(pos);

                    if clicked_outside {
                        ui.memory_mut(|mem| mem.close_popup());
                    }
                }
            }
        }

        response
    }
}
