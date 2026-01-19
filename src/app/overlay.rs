use std::time::Instant;

use device_query::DeviceQuery;
use egui::{Color32, FontId, LayerId, Pos2, Rect, Vec2};
use egui_overlay::EguiOverlay;
use windows::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, WDA_NONE, WS_POPUP};

impl EguiOverlay for crate::App {
    fn gui_run(
        &mut self,
        egui_context: &egui::Context,
        _default_gfx_backend: &mut egui_render_three_d::ThreeDBackend,
        glfw_backend: &mut egui_window_glfw_passthrough::GlfwBackend,
    ) {
        let start = std::time::Instant::now();
        //hide overlay from taskbar and alt+tab list + initialize lua (eventually)
        if !self.init {
            use windows::Win32::UI::WindowsAndMessaging::{
                GWL_EXSTYLE, SW_HIDE, SW_SHOW, SetWindowLongA,
                ShowWindow, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
            };
            log::info!("Performing one time window setup...");
            self.init = true;
            //still returns error even tho it works???
            let window = windowing::find_window_by_pid(self.pid).unwrap().unwrap();
            unsafe {
                let _ = ShowWindow(window, SW_HIDE);
                let _ = SetWindowLongA(
                    window,
                    GWL_EXSTYLE,
                    WS_EX_TOOLWINDOW.0 as i32
                        | WS_EX_TOPMOST.0 as i32
                        | WS_EX_LAYERED.0 as i32
                        | WS_POPUP.0 as i32,
                );

                let _ = ShowWindow(window, SW_SHOW);
                let _ = SetForegroundWindow(window);

                // Demonstrates how to add a font to the existing ones
                fn setup_custom_fonts(ctx: &egui::Context) {
                    let mut fonts = egui::FontDefinitions::default();

                    fonts.font_data.insert(
                        "tahoma".to_owned(),
                        egui::FontData::from_static(include_bytes!("../../resources/tahoma.ttf")),
                    );

                    fonts.font_data.insert(
                        "geist".to_owned(),
                        egui::FontData::from_static(include_bytes!("../../resources/geist.ttf")),
                    );

                    fonts
                        .families
                        .entry(egui::FontFamily::Proportional)
                        .or_default()
                        .insert(0, "geist".to_owned());

                    fonts
                        .families
                        .entry(egui::FontFamily::Monospace)
                        .or_default()
                        .insert(0, "geist".to_owned());

                    // Tell egui to use these fonts:
                    ctx.set_fonts(fonts);
                }
                setup_custom_fonts(egui_context);
                egui_extras::install_image_loaders(egui_context);
            }
        }

        if self.exit {
            log::warn!("Exiting!");
            // Auto-save config on exit
            if let Err(e) = self.config_store.write().save_if_dirty() {
                log::error!("Failed to save config on exit: {}", e);
            }
            glfw_backend.window.set_should_close(true);
            std::process::exit(0);
        }

        // //if neither game window nor overlay window is focused, return
        let foreground_window_pid = windowing::get_active_window_pid().unwrap().unwrap();
        if foreground_window_pid != self.game_pid && foreground_window_pid != self.pid {
            return;
        }

        if self
            .device_state
            .get_keys()
            .contains(&device_query::Keycode::Insert)
            && self.show_time.elapsed().as_millis() > 250
        {
            self.visible = !self.visible;
            self.visible_animation.start(Instant::now());
            self.visible_animation.set_values((
                if !self.visible { 0.0 } else { 1.0 },
                self.visible_animation.progress,
            ));
            self.show_time = std::time::Instant::now();
        }

        // Set window size to match game window size (x axis+1 to avoid glfw passthrough blackout bug)
        // basically if a transparent glfw window is focused and fullscreen, it won't be transparent
        // but if we set the size to be slightly larger than the monitor size, it wont count as fullscreen
        // so we just always set it to be slightly larger than the game window size.
        // this should always work unless someone has a game window that is exactly their monitor size but
        // -1 on the x-axis, and if that happens thats a them problem :nod:
        self.window_info = windowing::get_window_info(self.game_window)
            .unwrap()
            .unwrap(); //if window info fails we fucked anyways so might as well unwrap
        glfw_backend.window_size_virtual = self.window_info.size.into();
        glfw_backend.window.set_size(
            self.window_info.size.0 as i32 + 1,
            self.window_info.size.1 as i32,
        );
        glfw_backend
            .window
            .set_pos(self.window_info.pos.0, self.window_info.pos.1);

        //remove shadow from overlay
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

        egui_context.set_visuals(visuals);

        //painter can only exist within the context of the window, so setting it to the window size makes it cover the entire window
        let painter = egui::Painter::new(
            egui_context.clone(),
            LayerId::debug(),
            Rect {
                min: Pos2 { x: 0.0, y: 0.0 },
                max: Pos2 {
                    x: self.window_info.size.0 as f32,
                    y: self.window_info.size.0 as f32,
                },
            },
        );
        self.visible_animation.update();
        self.render_menu(egui_context, painter.clone());
        self.main_loop(painter.clone());

        // Display FPS counters
        painter.text(
            egui::Pos2 { x: 10.0, y: 250.0 },
            egui::Align2::LEFT_TOP,
            &format!("FPS: {:.0}", self.averaged_fps),
            egui::FontId::proportional(16.0),
            Color32::WHITE,
        );

        painter.text(
            egui::Pos2 { x: 10.0, y: 278.0 },
            egui::Align2::LEFT_TOP,
            &format!("True FPS: {:.0}", self.averaged_true_fps),
            egui::FontId::proportional(16.0),
            Color32::from_rgb(150, 150, 150),
        );

        let pos = egui_context.input(|ui| ui.screen_rect().left_top() + Vec2::new(25.0, 25.0));
        // Create layout for "noctis" in gray
        let noctis_galley = egui_context.fonts(|fonts| {
            fonts.layout_no_wrap(
                "noctis".to_string(),
                FontId::proportional(14.0),
                Color32::DARK_GRAY,
            )
        });

        // Create layout for "menu.dev | {} FPS" in white
        let rest_galley = egui_context.fonts(|fonts| {
            fonts.layout_no_wrap(
                "menu.dev".to_string(),
                FontId::proportional(14.0),
                Color32::WHITE,
            )
        });

        // Draw "noctis" in gray
        painter.galley(pos, noctis_galley.clone(), Color32::GRAY);

        // Draw "menu.dev | {} FPS" in white, offset by the width of "noctis"
        painter.galley(
            pos + Vec2::new(noctis_galley.size().x, 0.0),
            rest_galley,
            Color32::WHITE,
        );
        #[cfg(debug_assertions)]
        {
            // Display thread performance
            let mut y_offset = 296.0;

            for (thread_name, frametime) in self.threads_status() {
                let fps = 1.0 / frametime;
                painter.text(
                    egui::Pos2 {
                        x: 10.0,
                        y: y_offset,
                    },
                    egui::Align2::LEFT_TOP,
                    &format!(
                        "{}: {:.0} fps ({:.2}ms)",
                        thread_name,
                        fps,
                        frametime * 1000.0
                    ),
                    egui::FontId::proportional(14.0),
                    Color32::from_rgb(100, 100, 100),
                );
                y_offset += 18.0;
            }
        }

        //streamproof
        use crate::app::config_system::keys;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
        };
        let streamproof_enabled = self
            .config_store
            .read()
            .get_bool(keys::STREAMPROOF)
            .unwrap_or(false);

        if self.streamproofed && !streamproof_enabled {
            let window = windowing::find_window_by_pid(self.pid).unwrap().unwrap();
            let _ = unsafe { SetWindowDisplayAffinity(window, WDA_NONE) }; //budget streamproofing
            self.streamproofed = false;
        } else if !self.streamproofed && streamproof_enabled {
            let window = windowing::find_window_by_pid(self.pid).unwrap().unwrap();
            let _ = unsafe { SetWindowDisplayAffinity(window, WDA_EXCLUDEFROMCAPTURE) }; //budget streamproofing
            self.streamproofed = true;
        }

        //set passthrough enabling and request egui_repaint
        if egui_context.wants_pointer_input() || egui_context.wants_keyboard_input() {
            glfw_backend.set_passthrough(false);
        } else {
            glfw_backend.set_passthrough(true)
        }
        egui_context.request_repaint();
        // Measure true frametime BEFORE DwmFlush (no vsync wait)
        self.true_frametime = start.elapsed();
        self.true_frame_samples.push(self.true_frametime);
        unsafe {
            let _ = windows::Win32::Graphics::Dwm::DwmFlush();
        };
        // Measure total frametime AFTER DwmFlush (includes vsync wait)
        self.frametime = start.elapsed();
        self.frame_samples.push(self.frametime);

        // Update averaged FPS every 0.2 seconds
        if self.last_fps_update.elapsed().as_secs_f32() >= 0.2 {
            if !self.frame_samples.is_empty() {
                let avg_frametime: f32 = self
                    .frame_samples
                    .iter()
                    .map(|d| d.as_secs_f32())
                    .sum::<f32>()
                    / self.frame_samples.len() as f32;
                self.averaged_fps = 1.0 / avg_frametime;
                self.frame_samples.clear();
            }

            if !self.true_frame_samples.is_empty() {
                let avg_true_frametime: f32 = self
                    .true_frame_samples
                    .iter()
                    .map(|d| d.as_secs_f32())
                    .sum::<f32>()
                    / self.true_frame_samples.len() as f32;
                self.averaged_true_fps = 1.0 / avg_true_frametime;
                self.true_frame_samples.clear();
            }

            self.last_fps_update = Instant::now();
        }
    }
}
