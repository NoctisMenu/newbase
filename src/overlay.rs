use std::time::Instant;

use device_query::DeviceQuery;

/// Backing storage for [`VERSION_BANNER`]. `"version:"` (8) + up to a 40-char
/// git hash fits comfortably; oversized hashes are truncated in [`build_banner`].
const BANNER_CAP: usize = 64;

/// Compile-time `"version:" + hash` into a fixed buffer, returning the filled
/// length. Pure `const fn` (no crates) so the whole thing folds at compile time.
const fn build_banner(hash: &str) -> ([u8; BANNER_CAP], usize) {
    let mut buf = [0u8; BANNER_CAP];
    let prefix = b"version:";
    let mut n = 0;
    while n < prefix.len() {
        buf[n] = prefix[n];
        n += 1;
    }
    let hash = hash.as_bytes();
    let mut i = 0;
    while i < hash.len() && n < BANNER_CAP {
        buf[n] = hash[i];
        n += 1;
        i += 1;
    }
    (buf, n)
}

/// `OFFLINE` banner, produced the same way so the types line up in the `match`.
const fn build_banner_offline() -> ([u8; BANNER_CAP], usize) {
    let mut buf = [0u8; BANNER_CAP];
    let word = b"OFFLINE";
    let mut n = 0;
    while n < word.len() {
        buf[n] = word[n];
        n += 1;
    }
    (buf, n)
}

const BANNER_BUF: ([u8; BANNER_CAP], usize) = match option_env!("COMMIT_HASH") {
    Some(hash) => build_banner(hash),
    None => build_banner_offline(),
};

/// Build version banner shown top-left every frame.
///
/// Resolved at **compile time** from the `COMMIT_HASH` environment variable via
/// `option_env!`: when set (release builds tag it with the git hash) it renders
/// as `version:HASH`; when the variable is absent at build time it renders
/// `OFFLINE`. Everything folds to a static string in the compiled binary — no
/// runtime env lookup. A `build.rs` reruns the build when `COMMIT_HASH` changes.
const VERSION_BANNER: &str = match std::str::from_utf8(BANNER_BUF.0.split_at(BANNER_BUF.1).0) {
    Ok(s) => s,
    Err(_) => "OFFLINE",
};

impl<S: 'static + Send + Sync> crate::App<S> {
    fn draw_baseline_overlay_primitives(
        &self,
        ui: &newoverlay::imgui::Ui,
        draw_list: &newoverlay::imgui::DrawListMut,
        clear_fullscreen: bool,
    ) {
        let [display_w, display_h] = ui.io().display_size;

        if clear_fullscreen {
            let clear_w = display_w.max(1.0);
            let clear_h = display_h.max(1.0);
            draw_list
                .add_rect([0.0, 0.0], [clear_w, clear_h], [0.0, 0.0, 0.0, 0.0])
                .filled(true)
                .build();
        } else {
            // Keep at least one guaranteed draw command every normal frame.
            draw_list
                .add_rect([0.0, 0.0], [1.0, 1.0], [0.0, 0.0, 0.0, 0.0])
                .filled(true)
                .build();
        }

        // Always emit the semi-transparent white frame so the draw list is non-empty
        // and consistent even during shutdown frames.
        let mut frame_max_x = self.window_info.size.0 as f32;
        let mut frame_max_y = self.window_info.size.1 as f32;
        if !frame_max_x.is_finite() || frame_max_x <= 64.0 {
            frame_max_x = display_w.max(65.0);
        }
        if !frame_max_y.is_finite() || frame_max_y <= 64.0 {
            frame_max_y = display_h.max(65.0);
        }

        draw_list
            .add_rect(
                [64.0, 64.0],
                [frame_max_x, frame_max_y],
                [1.0, 1.0, 1.0, 0.01],
            )
            .build();

        // Build version banner, top-left. Shadow then text for legibility over
        // any background.
        draw_list.add_text([5.0, 4.0], [0.0, 0.0, 0.0, 0.75], VERSION_BANNER);
        draw_list.add_text([4.0, 3.0], [1.0, 1.0, 1.0, 1.0], VERSION_BANNER);
    }

    fn force_clear_overlay_window(&mut self, overlay: &mut newoverlay::Overlay) {
        self.visible = false;
        self.debug_lines.clear();

        // Push a couple of fully transparent frames so the last UI frame
        // does not linger on screen while the process exits.
        for _ in 0..2 {
            if !overlay.start_render() {
                break;
            }

            overlay.render(|ui| {
                let draw_list = ui.get_background_draw_list();
                self.draw_baseline_overlay_primitives(ui, &draw_list, true);
            });

            unsafe {
                let _ = windows::Win32::Graphics::Dwm::DwmFlush();
            }
        }
    }

    fn initialize_fps_font(&mut self, overlay: &mut newoverlay::Overlay) {
        let mut loaded_font = None;
        let _ = overlay.configure_fonts(|ctx| {
            let mut fonts = ctx.fonts();
            loaded_font = Some(fonts.add_font(&[newoverlay::imgui::FontSource::TtfData {
                data: include_bytes!("../resources/tahoma.ttf"),
                size_pixels: 22.0,
                config: Some(newoverlay::imgui::FontConfig {
                    // Keep glyphs on the pixel grid and avoid heavy subpixel blending.
                    pixel_snap_h: true,
                    oversample_h: 1,
                    oversample_v: 1,
                    // Slightly embolden the glyph coverage.
                    rasterizer_multiply: 1.30,
                    ..Default::default()
                }),
            }]));
        });

        if loaded_font.is_none() {
            log::warn!("Failed to load resources/tahoma.ttf for FPS counter");
        }
        self.fps_font = loaded_font;
    }

    fn initialize_logo_texture(&mut self, overlay: &mut newoverlay::Overlay) {
        let Ok(logo) = image::load_from_memory(include_bytes!("../resources/logo.png")) else {
            log::warn!("Failed to decode resources/logo.png; menu intro logo disabled");
            self.menu_logo = None;
            return;
        };

        let mut logo = logo.to_rgba8();
        let (mut logo_w, mut logo_h) = logo.dimensions();
        if logo_w == 0 || logo_h == 0 {
            log::warn!("resources/logo.png has invalid dimensions; menu intro logo disabled");
            self.menu_logo = None;
            return;
        }

        const MAX_ATLAS_LOGO_DIM: u32 = 512;
        if logo_w > MAX_ATLAS_LOGO_DIM || logo_h > MAX_ATLAS_LOGO_DIM {
            let scale = (MAX_ATLAS_LOGO_DIM as f32 / logo_w as f32)
                .min(MAX_ATLAS_LOGO_DIM as f32 / logo_h as f32);
            let new_w = ((logo_w as f32 * scale).round() as u32).max(1);
            let new_h = ((logo_h as f32 * scale).round() as u32).max(1);
            logo = image::imageops::resize(
                &logo,
                new_w,
                new_h,
                image::imageops::FilterType::Lanczos3,
            );
            logo_w = new_w;
            logo_h = new_h;
        }

        let logo_pixels = logo.into_raw();
        let mut uv_min = [0.0_f32; 2];
        let mut uv_max = [0.0_f32; 2];
        let mut loaded_font = None;
        let mut packed_ok = false;
        let upload_ok = overlay.configure_fonts(|ctx| unsafe {
            let mut fonts = ctx.fonts();
            loaded_font = Some(fonts.add_font(&[newoverlay::imgui::FontSource::TtfData {
                data: include_bytes!("../resources/tahoma.ttf"),
                size_pixels: 22.0,
                config: Some(newoverlay::imgui::FontConfig {
                    pixel_snap_h: true,
                    oversample_h: 1,
                    oversample_v: 1,
                    rasterizer_multiply: 1.30,
                    ..Default::default()
                }),
            }]));

            // The font atlas is already built by the renderer at this point.
            // Clear texture data so custom rect packing can run again.
            fonts.clear_tex_data();
            if fonts.tex_desired_width < 2048 {
                fonts.tex_desired_width = 2048;
            }

            let atlas = (&mut *fonts as *mut _) as *mut newoverlay::imgui::sys::ImFontAtlas;
            let rect_index = newoverlay::imgui::sys::ImFontAtlas_AddCustomRectRegular(
                atlas,
                logo_w as i32,
                logo_h as i32,
            );
            if rect_index < 0 {
                return;
            }

            let _ = fonts.build_rgba32_texture();

            let mut atlas_pixels = std::ptr::null_mut();
            let mut atlas_w = 0_i32;
            let mut atlas_h = 0_i32;
            let mut bytes_per_pixel = 0_i32;
            newoverlay::imgui::sys::ImFontAtlas_GetTexDataAsRGBA32(
                atlas,
                &mut atlas_pixels,
                &mut atlas_w,
                &mut atlas_h,
                &mut bytes_per_pixel,
            );

            if atlas_pixels.is_null() || bytes_per_pixel != 4 || atlas_w <= 0 || atlas_h <= 0 {
                return;
            }

            let rect = newoverlay::imgui::sys::ImFontAtlas_GetCustomRectByIndex(atlas, rect_index);
            if rect.is_null() || !newoverlay::imgui::sys::ImFontAtlasCustomRect_IsPacked(rect) {
                return;
            }

            let rect = &*rect;
            let rect_x = rect.X as usize;
            let rect_y = rect.Y as usize;
            let atlas_w = atlas_w as usize;
            let logo_w = logo_w as usize;
            let logo_h = logo_h as usize;
            let atlas_pixels = atlas_pixels as *mut u8;

            for row in 0..logo_h {
                let src_offset = row * logo_w * 4;
                let dst_offset = ((rect_y + row) * atlas_w + rect_x) * 4;
                let src_row =
                    std::slice::from_raw_parts(logo_pixels.as_ptr().add(src_offset), logo_w * 4);
                let dst_row =
                    std::slice::from_raw_parts_mut(atlas_pixels.add(dst_offset), logo_w * 4);

                // DX9 path in this renderer expects BGRA ordering.
                for px in 0..logo_w {
                    let i = px * 4;
                    dst_row[i] = src_row[i + 2];
                    dst_row[i + 1] = src_row[i + 1];
                    dst_row[i + 2] = src_row[i];
                    dst_row[i + 3] = src_row[i + 3];
                }
            }

            let mut uv0 = newoverlay::imgui::sys::ImVec2 { x: 0.0, y: 0.0 };
            let mut uv1 = newoverlay::imgui::sys::ImVec2 { x: 0.0, y: 0.0 };
            newoverlay::imgui::sys::ImFontAtlas_CalcCustomRectUV(atlas, rect, &mut uv0, &mut uv1);
            uv_min = [uv0.x, uv0.y];
            uv_max = [uv1.x, uv1.y];
            packed_ok = true;
        });

        if loaded_font.is_none() {
            log::warn!("Failed to load resources/tahoma.ttf for FPS counter");
        }
        self.fps_font = loaded_font;

        if !upload_ok || !packed_ok {
            log::warn!("Failed to prepare logo texture in ImGui atlas; menu intro logo disabled");
            self.menu_logo = None;
            return;
        }

        self.menu_logo = Some(crate::app::MenuLogo {
            texture_id: newoverlay::imgui::TextureId::from(!0usize),
            uv_min,
            uv_max,
            aspect_ratio: logo_w as f32 / logo_h as f32,
        });
    }

    fn draw_perf_and_debug(
        &self,
        ui: &newoverlay::imgui::Ui,
        draw_list: &newoverlay::imgui::DrawListMut,
    ) {
        draw_list.add_text(
            [10.0, 250.0],
            [1.0, 1.0, 1.0, 1.0],
            format!("FPS: {:.0}", self.averaged_fps),
        );
        draw_list.add_text(
            [10.0, 278.0],
            [0.59, 0.59, 0.59, 1.0],
            format!("True FPS: {:.0}", self.averaged_true_fps),
        );

        if self.debug_lines.is_empty() {
            return;
        }

        const DEBUG_X: f32 = 10.0;
        const DEBUG_Y: f32 = 312.0;
        const DEBUG_LINE_GAP: f32 = 4.0;
        const DEBUG_PADDING_X: f32 = 8.0;
        const DEBUG_PADDING_Y: f32 = 6.0;

        let mut max_width = 0.0_f32;
        let mut total_height = 0.0_f32;
        for (idx, line) in self.debug_lines.iter().enumerate() {
            let [w, h] = ui.calc_text_size(line);
            max_width = max_width.max(w);
            total_height += h;
            if idx + 1 < self.debug_lines.len() {
                total_height += DEBUG_LINE_GAP;
            }
        }

        draw_list
            .add_rect(
                [DEBUG_X - DEBUG_PADDING_X, DEBUG_Y - DEBUG_PADDING_Y],
                [
                    DEBUG_X + max_width + DEBUG_PADDING_X,
                    DEBUG_Y + total_height + DEBUG_PADDING_Y,
                ],
                [0.0, 0.0, 0.0, 0.70],
            )
            .filled(true)
            .rounding(4.0)
            .build();

        let mut y = DEBUG_Y;
        for line in &self.debug_lines {
            draw_list.add_text([DEBUG_X, y], [1.0, 1.0, 1.0, 1.0], line);
            y += ui.calc_text_size(line)[1] + DEBUG_LINE_GAP;
        }
    }

    pub fn run(&mut self) {
        let mut overlay = loop {
            match newoverlay::Overlay::new() {
                Some(o) => break o,
                None => {
                    log::error!("Searching for discord window...");
                    std::thread::sleep(std::time::Duration::from_secs(2))
                }
            }
        };

        log::info!("Overlay initialized successfully");
        // Font + logo are configured together in initialize_logo_texture().
        self.initialize_logo_texture(&mut overlay);

        loop {
            let start = std::time::Instant::now();

            if self.exit {
                log::warn!("Exiting!");
                self.force_clear_overlay_window(&mut overlay);
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
                self.visible = !self.visible;
                self.show_time = std::time::Instant::now();
            }

            self.visible = true;
            self.show_time = std::time::Instant::now();

            // Set window size to match game window size (x axis+1 to avoid glfw passthrough blackout bug)
            // Cache window info updates to ~10Hz to avoid expensive Windows API calls every frame
            if self.last_fps_update.elapsed().as_millis() >= 100 {
                self.window_info = windowing::get_window_info(self.game_window).unwrap().unwrap();
            }

            if !overlay.start_render() {
                break;
            }

            // Render UI
            overlay.render(|ui| {
                let draw_list = ui.get_background_draw_list();
                self.draw_baseline_overlay_primitives(ui, &draw_list, false);

                // Render menu and main loop
                if self.visible {
                    self.render_menu(ui);
                    // Clamp intro timeline advancement so first-frame delta spikes
                    // don't skip the fade-in phase.
                    let intro_dt = ui.io().delta_time.clamp(0.0, 1.0 / 30.0);
                    self.menu_intro_elapsed += intro_dt;
                }

                if self.menu_intro_finished {
                    self.tick_logic(ui, &draw_list);

                    if let Some(font) = self.fps_font {
                        let _font = ui.push_font(font);
                        self.draw_perf_and_debug(ui, &draw_list);
                    } else {
                        self.draw_perf_and_debug(ui, &draw_list);
                    }
                }

                // Debug text is transient by design; callers should re-submit each frame.
                self.debug_lines.clear();
            });

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
}
