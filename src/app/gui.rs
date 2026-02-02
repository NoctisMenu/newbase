
use egui::{Color32, FontId, Painter, Pos2, Rect, RichText, Rounding, Stroke, Vec2};
use std::time::Duration;

use crate::config;

pub struct Cheat<'a> {
    pub tab: super::MenuTab,
    pub highlight: &'a mut bool,
}

impl crate::App {
    pub fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none().inner_margin(10.0).show(ui, |ui| {
            ui.set_min_size(Vec2::new(100.0, 500.0));
            ui.vertical(|ui| {
                ui.add_space(100.0);

                // Use accent color from config_store
                use crate::app::config_system::keys;
                let accent_color = self
                    .config_store
                    .read()
                    .get_color(keys::ACCENT_COLOR)
                    .unwrap_or(Color32::GREEN);

                self.aim_button.line_color = accent_color;
                self.esp_button.line_color = accent_color;
                self.exploits_button.line_color = accent_color;
                self.misc_button.line_color = accent_color;

                if self.aim_button.show(ui, "AIM").clicked() {
                    self.tab = super::MenuTab::Aim;
                    self.esp_button.set_selected(false);
                    self.exploits_button.set_selected(false);
                    self.misc_button.set_selected(false);
                };
                if self.esp_button.show(ui, "ESP").clicked() {
                    self.tab = super::MenuTab::Esp;
                    self.aim_button.set_selected(false);
                    self.exploits_button.set_selected(false);
                    self.misc_button.set_selected(false);
                };
                if self.exploits_button.show(ui, "EXPLOITS").clicked() {
                    self.tab = super::MenuTab::Exploits;
                    self.aim_button.set_selected(false);
                    self.esp_button.set_selected(false);
                    self.misc_button.set_selected(false);
                };
                if self.misc_button.show(ui, "MISC").clicked() {
                    self.tab = super::MenuTab::Misc;
                    self.aim_button.set_selected(false);
                    self.esp_button.set_selected(false);
                    self.exploits_button.set_selected(false);
                };
            });
        });
    }

    pub fn render_aim_frame(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .inner_margin(5.0)
            .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 70))
            //.stroke(Stroke::new(1.0, self.config.core_config.accent_color))
            .rounding(Rounding::same(8.0))
            .show(ui, |ui| {
                ui.style_mut().interaction.selectable_labels = false;
                ui.set_min_size(Vec2::new(200.0, 250.0));

                use crate::app::config_system::keys;
                let mut config = self.config_store.write();
                let accent = config
                    .get_color(keys::ACCENT_COLOR)
                    .unwrap_or(Color32::GREEN);

                // Auto-render all widgets from "aim" section
                config.render_section(ui, "aim", accent).ok();
            });
    }

    pub fn render_esp_frame(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .inner_margin(5.0)
            .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 70))
            //.stroke(Stroke::new(1.0, self.config.core_config.accent_color))
            .rounding(Rounding::same(8.0))
            .show(ui, |ui| {
                ui.style_mut().interaction.selectable_labels = false;
                ui.set_min_size(Vec2::new(200.0, 250.0));

                use crate::app::config_system::keys;
                let mut config = self.config_store.write();
                let accent = config
                    .get_color(keys::ACCENT_COLOR)
                    .unwrap_or(Color32::GREEN);

                // Auto-render all widgets from "esp" section
                config.render_section(ui, "esp", accent).ok();
            });
    }

    pub fn render_exploits_frame(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .inner_margin(5.0)
            .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 70))
            .rounding(Rounding::same(8.0))
            .show(ui, |ui| {
                ui.style_mut().interaction.selectable_labels = false;
                ui.set_min_size(Vec2::new(200.0, 250.0));

                use crate::app::config_system::keys;
                let mut config = self.config_store.write();
                let accent = config
                    .get_color(keys::ACCENT_COLOR)
                    .unwrap_or(Color32::GREEN);

                // Auto-render all widgets from "exploits" section
                config.render_section(ui, "exploits", accent).ok();
            });
    }

    pub fn render_misc_frame(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .inner_margin(5.0)
            .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 70))
            //.stroke(Stroke::new(1.0, self.config.core_config.accent_color))
            .rounding(Rounding::same(8.0))
            .show(ui, |ui| {
                ui.set_min_size(Vec2::splat(100.0));

                use crate::app::config_system::keys;
                let mut config = self.config_store.write();
                let accent_color = config
                    .get_color(keys::ACCENT_COLOR)
                    .unwrap_or(Color32::GREEN);

                // Auto-render all widgets from "core" section
                config.render_section(ui, "core", accent_color).ok();
                drop(config); // Release write lock before buttons

                // Manual buttons (keep these)
                if ui.button("Save Config").clicked() {
                    match self.config_store.write().save() {
                        Ok(_) => {
                            self.toasts
                                .add_toast("Config saved!", Duration::from_secs(2));
                        }
                        Err(e) => {
                            self.toasts.add_toast(
                                format!("Failed to save: {}", e),
                                Duration::from_secs(3),
                            );
                        }
                    }
                }

                if ui.button("Load Config").clicked() {
                    match self.config_store.write().reload() {
                        Ok(_) => {
                            self.toasts
                                .add_toast("Config loaded!", Duration::from_secs(2));
                        }
                        Err(e) => {
                            self.toasts.add_toast(
                                format!("Failed to load: {}", e),
                                Duration::from_secs(3),
                            );
                        }
                    }
                }

                if ui.button("Terminate").clicked() {
                    self.exit = true;
                }
            });
    }

    pub fn render_menu(&mut self, ctx: &egui::Context, painter: Painter) {
        //make it so that windows won't have drop shadow behind them (could look cool in future?)
        let mut visuals = egui::Visuals::default();

        visuals.window_shadow = egui::epaint::Shadow {
            offset: Vec2::default(),
            blur: 0.0,
            spread: 0.0,
            color: Color32::from_rgb(0, 0, 0),
        };

        visuals.popup_shadow = egui::epaint::Shadow {
            offset: Vec2::default(),
            blur: 0.0,
            spread: 0.0,
            color: Color32::from_rgb(0, 0, 0),
        };

        visuals.override_text_color = Some(Color32::WHITE);

        // let mut complementary_color: Hsva = self.config.core_config.accent_color.into();
        // complementary_color.h += 0.5;
        // complementary_color.h %= 1.0;
        // let mut complementary_color: Color32 = complementary_color.into();

        // Get accent color from config_store
        use crate::app::config_system::keys;
        let accent_color = self
            .config_store
            .read()
            .get_color(keys::ACCENT_COLOR)
            .unwrap_or(Color32::GREEN);

        ctx.set_visuals(visuals);
        if self.visible {
            

            egui::Window::new("")
            .id("main_window".into())
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(
                        Color32::from_rgba_unmultiplied(0, 0, 0, 180)
                            .lerp_to_gamma(accent_color, 0.03),
                    )
                    .stroke((1.0, accent_color))
                    .rounding(10.0),
            )
            .fixed_size((900.0, 600.0))
            .title_bar(false)
            .show(ctx, |ui| {
                self.visible_animation.update();
                ui.add_space(25.0);
                ui.horizontal(|ui| {
                    self.render_menu_bar(ui);
                    ui.vertical(|ui| match self.tab {
                        super::MenuTab::Aim => self.render_aim_frame(ui),
                        super::MenuTab::Esp => self.render_esp_frame(ui),
                        super::MenuTab::Exploits => self.render_exploits_frame(ui),
                        super::MenuTab::Misc => self.render_misc_frame(ui),
                    })
                });

                ui.ctx().request_repaint();
            });
        }

        self.toasts
            .show(&ctx, painter.clone(), accent_color);
        
        #[cfg(not(debug_assertions))] {
            let time_remaining = self
                .time_remaining
                .load(std::sync::atomic::Ordering::Relaxed) as f32;
            let galley = ctx.fonts(|fonts| {
                fonts.layout_no_wrap(
                    format!(
                        "Time remaining: {} days {} hours {} minutes",
                        (time_remaining / 60.0 / 60.0 / 24.0).floor(),
                        ((time_remaining / 60.0 / 60.0) % 24.0).floor(),
                        ((time_remaining / 60.0) % 60.0).floor(),
                    ),
                    FontId::proportional(14.0),
                    Color32::WHITE,
                )
            });
            let rect_color = if time_remaining < 60.0 * 30.0 {
                //30 mins
                Color32::RED
            } else {
                accent_color
            };
            painter.rect(
                Rect {
                    min: ctx.input(|ui| {
                        ui.screen_rect().right_bottom() - Vec2::new(galley.size().x + 30.0, 45.0)
                    }),
                    max: ctx.input(|ui| ui.screen_rect().right_bottom() - Vec2::new(20.0, 20.0)),
                },
                3.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, 150).lerp_to_gamma(rect_color, 0.3),
                Stroke::new(1.0, rect_color),
            );
            painter.text(
                ctx.input(|ui| ui.screen_rect().right_bottom() - Vec2::new(25.0, 25.0)),
                egui::Align2::RIGHT_BOTTOM,
                format!(
                    "Time remaining: {} days {} hours {} minutes",
                    (time_remaining / 60.0 / 60.0 / 24.0).floor(),
                    ((time_remaining / 60.0 / 60.0) % 24.0).floor(),
                    ((time_remaining / 60.0) % 60.0).floor(),
                ),
                FontId::proportional(14.0),
                Color32::WHITE,
            );
        }
        
        //only render debug menu & offline build if building in debug mode
        #[cfg(debug_assertions)]
        {
            let galley = ctx.fonts(|fonts| {
                fonts.layout_no_wrap(
                    format!("Offline Build"),
                    FontId::proportional(14.0),
                    Color32::WHITE,
                )
            });
            painter.rect(
                Rect {
                    min: ctx.input(|ui| {
                        ui.screen_rect().right_bottom() - Vec2::new(galley.size().x + 30.0, 45.0)
                    }),
                    max: ctx.input(|ui| ui.screen_rect().right_bottom() - Vec2::new(20.0, 20.0)),
                },
                3.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, 150).lerp_to_gamma(accent_color, 0.3),
                Stroke::new(1.0, accent_color),
            );
            painter.text(
                ctx.input(|ui| ui.screen_rect().right_bottom() - Vec2::new(25.0, 25.0)),
                egui::Align2::RIGHT_BOTTOM,
                format!("Offline Build"),
                FontId::proportional(14.0),
                Color32::WHITE,
            );

            egui::Window::new("debug menu")
                .interactable(self.visible)
                .show(ctx, |ui| {
                    ui.label(&self.debug);
                });
            self.debug = "".to_string()
        }
    }
    pub fn dim_window(&self, ui: &mut egui::Ui, painter: Painter) {
        let rect = ui.max_rect();
        painter.rect_filled(rect, 3.0, Color32::from_rgba_unmultiplied(0, 0, 0, 150));
    }
}
