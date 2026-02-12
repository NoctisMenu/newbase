use std::time::Instant;

use device_query::DeviceQuery;
use newoverlay::imgui::{Condition, FontSource, Window};
use windows::Win32::UI::WindowsAndMessaging::{SetForegroundWindow, WDA_NONE, WS_POPUP};

impl crate::App {
    pub fn run(
        &mut self,
    ) {
        let mut overlay = loop {
            match newoverlay::Overlay::new() {
                Some(o) => break o,
                None => {
                    log::error!("Failed to initialize overlay");
                    std::thread::sleep(std::time::Duration::from_secs(2))
                }
            }
        };

        log::info!("Overlay initialized successfully");

        let mut frame_count = 0;

        loop {
            let start = std::time::Instant::now();

            if self.exit {
                log::warn!("Exiting!");
                // Auto-save config on exit
                if let Err(e) = self.config_store.write().save_if_dirty() {
                    log::error!("Failed to save config on exit: {}", e);
                }
                std::process::exit(0);
            }

            // //if neither game window nor overlay window is focused, return
            let foreground_window_pid = windowing::get_active_window_pid().unwrap().unwrap();
            if foreground_window_pid != self.game_pid && foreground_window_pid != self.pid {
                continue;
            }

            if self
                .device_state
                .get_keys()
                .contains(&device_query::Keycode::Insert)
                && self.show_time.elapsed().as_millis() > 250
            {
                println!("Toggling menu visibility...");
                self.visible = !self.visible;
                self.show_time = std::time::Instant::now();
            }

            self.visible = true;
            self.show_time = std::time::Instant::now();

            // Set window size to match game window size (x axis+1 to avoid glfw passthrough blackout bug)
            // Cache window info updates to ~10Hz to avoid expensive Windows API calls every frame
            if self.last_fps_update.elapsed().as_millis() >= 100 {
                self.window_info = windowing::get_window_info(self.game_window)
                    .unwrap()
                    .unwrap();
            }

            if !overlay.start_render() {
                break;
            }

            // Render UI
            overlay.render(|ui| {
                let draw_list = ui.get_background_draw_list();
                let _ = draw_list.add_rect(
                    [64., 64.],
                    [
                        self.window_info.size.0 as f32 - 64.,
                        self.window_info.size.1 as f32 - 64.,
                    ],
                    [1.0, 1.0, 1.0, 0.5], // Semi-transparent white
                );

                // Render menu and main loop
                if self.visible {
                    self.render_menu(ui);
                }
                self.main_loop(ui, &draw_list);

                // Display FPS counters
                draw_list.add_text(
                    [10.0, 250.0],
                    [1.0, 1.0, 1.0, 1.0], // White
                    format!("FPS: {:.0}", self.averaged_fps)
                );

                draw_list.add_text(
                    [10.0, 278.0],
                    [0.59, 0.59, 0.59, 1.0], // Gray (150/255 = 0.59)
                    format!("True FPS: {:.0}", self.averaged_true_fps)
                );

                #[cfg(debug_assertions)]
                {
                    // Display thread performance
                    let mut y_offset = 296.0;

                    for (thread_name, frametime) in self.threads_status() {
                        let fps = 1.0 / frametime;
                        let text = format!("{}: {:.0} fps ({:.2}ms)", thread_name, fps, frametime * 1000.0);

                        // Draw black shadow for better readability (offset by 1 pixel)
                        draw_list.add_text(
                            [11.0, y_offset + 1.0],
                            [0.0, 0.0, 0.0, 0.5], // Semi-transparent black shadow
                            &text
                        );

                        // Draw actual text
                        draw_list.add_text(
                            [10.0, y_offset],
                            [0.59, 0.59, 0.59, 1.0], // Gray (150/255 = 0.59)
                            text
                        );
                        y_offset += 18.0;
                    }
                }
            });


            // Measure true frametime BEFORE DwmFlush (no vsync wait)
            self.true_frametime = start.elapsed();
            self.true_frame_samples.push(self.true_frametime);
            unsafe { let _ = windows::Win32::Graphics::Dwm::DwmFlush(); };
            // Measure total frametime AFTER DwmFlush (includes vsync wait)
            self.frametime = start.elapsed();
            self.frame_samples.push(self.frametime);

            // Update averaged FPS every 0.2 seconds
            if self.last_fps_update.elapsed().as_secs_f32() >= 0.2 {
                if !self.frame_samples.is_empty() {
                    let avg_frametime: f32 = self.frame_samples.iter()
                        .map(|d| d.as_secs_f32())
                        .sum::<f32>() / self.frame_samples.len() as f32;
                    self.averaged_fps = 1.0 / avg_frametime;
                    self.frame_samples.clear();
                }

                if !self.true_frame_samples.is_empty() {
                    let avg_true_frametime: f32 = self.true_frame_samples.iter()
                        .map(|d| d.as_secs_f32())
                        .sum::<f32>() / self.true_frame_samples.len() as f32;
                    self.averaged_true_fps = 1.0 / avg_true_frametime;
                    self.true_frame_samples.clear();
                }

                self.last_fps_update = Instant::now();

            }
        }
    }
}