use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::fs;
use std::path::PathBuf;

use crate::export::{export_all_animations, export_current_animation};
use crate::file::{pick_export_file, pick_export_folder, pick_open_file, pick_save_file};
use crate::imaging::{create_reference_thumbnail, decode_base64_to_texture, calculate_fit_scale};
use crate::model::Project;
use crate::state::{ActiveTab, ContextMenuTarget, GalleryDrag, PendingAction, DEFAULT_PANEL_MARGIN, ZOOM_LEVELS};
use crate::state::AppState;
use crate::ui::canvas::render_canvas;
use crate::ui::character_editor::render_character_editor;
use crate::ui::dialogs::render_dialogs;
use crate::ui::widgets::{format_relative_time, format_zoom, scaled_font, scaled_margin, tab_button};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn ui_system(mut contexts: EguiContexts, mut state: ResMut<AppState>, time: Res<Time>) {
    let ctx = contexts.ctx_mut();

    // Apply UI scale to global text styles and spacing
    let ui_scale = state.config.ui_scale;
    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(scaled_font(20.0, ui_scale)),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::proportional(scaled_font(14.0, ui_scale)),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::proportional(scaled_font(14.0, ui_scale)),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::proportional(scaled_font(12.0, ui_scale)),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::monospace(scaled_font(14.0, ui_scale)),
    );
    // Scale checkbox/radio button sizes
    style.spacing.icon_width = scaled_margin(14.0, ui_scale);
    style.spacing.icon_width_inner = scaled_margin(8.0, ui_scale);
    style.spacing.icon_spacing = scaled_margin(4.0, ui_scale);
    // Prevent text wrapping in menus
    style.wrap_mode = Some(egui::TextWrapMode::Extend);
    ctx.set_style(style);

    // Global keyboard shortcuts for UI scale (Ctrl+Plus/Minus/0)
    // Plus requires Shift on most keyboards (Shift+=), Minus and 0 do not
    // Consume the key events to prevent UI jumbling even at min/max
    let increase_pressed = ctx.input_mut(|i| {
        i.modifiers.command
            && (i.consume_key(egui::Modifiers::COMMAND, egui::Key::Plus)
                || i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Equals))
    });
    if increase_pressed && state.config.ui_scale < 2.0 {
        state.config.ui_scale = (state.config.ui_scale + 0.25).min(2.0);
        state.config.save();
    }
    let decrease_pressed = ctx.input_mut(|i| {
        i.consume_key(egui::Modifiers::COMMAND, egui::Key::Minus)
    });
    if decrease_pressed && state.config.ui_scale > 0.75 {
        state.config.ui_scale = (state.config.ui_scale - 0.25).max(0.75);
        state.config.save();
    }
    let reset_pressed = ctx.input_mut(|i| {
        i.consume_key(egui::Modifiers::COMMAND, egui::Key::Num0)
    });
    if reset_pressed && state.config.ui_scale != 1.0 {
        state.config.ui_scale = 1.0;
        state.config.save();
    }

    // Handle animation playback
    if state.is_playing {
        let delta = time.delta_secs();
        state.playback_time += delta;

        // Calculate frame duration from animation's FPS
        let fps = state.current_animation().map(|a| a.fps).unwrap_or(12);
        let frame_duration_secs = 1.0 / fps.max(1) as f32;

        // Advance frame if enough time has passed
        if state.playback_time >= frame_duration_secs {
            state.playback_time -= frame_duration_secs;
            let total = state.total_frames();
            if total > 0 {
                state.current_frame = (state.current_frame + 1) % total;
            }
        }
    }

    // Dialogs (rendered first so they appear on top)
    render_dialogs(ctx, &mut state);

    // Menu bar
    let menu_font_size = scaled_font(15.0, state.config.ui_scale);
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button(
                egui::RichText::new("File").size(menu_font_size),
                |ui| {
                    if ui.button("New Project").clicked() {
                        if state.has_unsaved_changes() {
                            state.pending_action = Some(PendingAction::NewProject);
                        } else {
                            state.new_project();
                            state.set_status("Created new project");
                        }
                        ui.close_menu();
                    }
                    if ui.button("Open...").clicked() {
                        if state.has_unsaved_changes() {
                            state.pending_action = Some(PendingAction::OpenProject);
                        } else if let Some(path) = pick_open_file() {
                            let path_str = path.to_string_lossy().to_string();
                            match state.load_project(&path_str) {
                                Ok(()) => state.set_status(format!("Loaded {}", path_str)),
                                Err(e) => state.set_status(format!("Load failed: {}", e)),
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    let has_project = state.project.is_some();
                    let has_path = state.project_path.is_some();

                    // Save - only if we have a path already
                    if ui
                        .add_enabled(has_project && has_path, egui::Button::new("Save"))
                        .clicked()
                    {
                        match state.save_project() {
                            Ok(()) => state.set_status("Project saved"),
                            Err(e) => state.set_status(format!("Save failed: {}", e)),
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_project, egui::Button::new("Save As..."))
                        .clicked()
                    {
                        if let Some(path) = pick_save_file() {
                            let path_str = path.to_string_lossy().to_string();
                            match state.save_project_as(&path_str) {
                                Ok(()) => state.set_status(format!("Saved to {}", path_str)),
                                Err(e) => state.set_status(format!("Save failed: {}", e)),
                            }
                        }
                        ui.close_menu();
                    }
                    // Close Project
                    if ui
                        .add_enabled(has_project, egui::Button::new("Close Project"))
                        .clicked()
                    {
                        if state.has_unsaved_changes() {
                            state.pending_action = Some(PendingAction::CloseProject);
                        } else {
                            state.close_project();
                            state.set_status("Project closed");
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        if state.has_unsaved_changes() {
                            state.pending_action = Some(PendingAction::Exit);
                        } else {
                            std::process::exit(0);
                        }
                        ui.close_menu();
                    }
                },
            );

            ui.menu_button(egui::RichText::new("Edit").size(menu_font_size), |ui| {
                if ui.button("Undo").clicked() {
                    ui.close_menu();
                }
                if ui.button("Redo").clicked() {
                    ui.close_menu();
                }
            });

            let view_menu_id = ui.make_persistent_id("view_menu");
            if state.reopen_view_menu {
                state.reopen_view_menu = false;
                ui.memory_mut(|mem| mem.open_popup(view_menu_id));
            }
            egui::menu::menu_button(
                ui,
                egui::RichText::new("View").size(menu_font_size),
                |ui| {
                    let mut scale_changed = false;
                    ui.horizontal(|ui| {
                        ui.label("UI Scale:");
                        let button_size = scaled_margin(20.0, state.config.ui_scale);
                        if ui
                            .add_sized([button_size, button_size], egui::Button::new("−"))
                            .on_hover_text("Decrease UI scale (Ctrl+−)")
                            .clicked()
                            && state.config.ui_scale > 0.75
                        {
                            state.config.ui_scale = (state.config.ui_scale - 0.25).max(0.75);
                            state.config.save();
                            scale_changed = true;
                        }
                        ui.label(format!("{:.0}%", state.config.ui_scale * 100.0));
                        if ui
                            .add_sized([button_size, button_size], egui::Button::new("+"))
                            .on_hover_text("Increase UI scale (Ctrl++)")
                            .clicked()
                            && state.config.ui_scale < 2.0
                        {
                            state.config.ui_scale = (state.config.ui_scale + 0.25).min(2.0);
                            state.config.save();
                            scale_changed = true;
                        }
                        if state.config.ui_scale != 1.0 {
                            if ui
                                .small_button("Reset")
                                .on_hover_text("Reset to 100% (Ctrl+0)")
                                .clicked()
                            {
                                state.config.ui_scale = 1.0;
                                state.config.save();
                                scale_changed = true;
                            }
                        }
                    });
                    if scale_changed {
                        state.reopen_view_menu = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.checkbox(&mut state.show_grid, "Show Grid");
                    ui.checkbox(&mut state.show_labels, "Show Labels");
                    ui.checkbox(&mut state.show_overlay_info, "Show Overlay Info");
                },
            );

            let has_project = state.project.is_some();
            ui.menu_button(
                egui::RichText::new("Character").size(menu_font_size),
                |ui| {
                    if ui
                        .add_enabled(has_project, egui::Button::new("New Character..."))
                        .clicked()
                    {
                        state.show_new_character_dialog = true;
                        state.new_character_name.clear();
                        ui.close_menu();
                    }

                    let has_characters = state
                        .project
                        .as_ref()
                        .map(|p| !p.characters.is_empty())
                        .unwrap_or(false);

                    if ui
                        .add_enabled(has_characters, egui::Button::new("Add Part..."))
                        .clicked()
                    {
                        state.show_new_part_dialog = true;
                        state.new_part_name.clear();
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_characters, egui::Button::new("Add State..."))
                        .clicked()
                    {
                        state.show_new_state_dialog = true;
                        state.new_state_name.clear();
                        ui.close_menu();
                    }
                },
            );

            ui.menu_button(
                egui::RichText::new("Animation").size(menu_font_size),
                |ui| {
                    if ui
                        .add_enabled(has_project, egui::Button::new("New Animation..."))
                        .clicked()
                    {
                        state.show_new_animation_dialog = true;
                        state.new_animation_name.clear();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(has_project, egui::Button::new("Add Frame"))
                        .clicked()
                    {
                        if let Some(anim) = state.current_animation_mut() {
                            anim.add_frame();
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_project, egui::Button::new("Delete Frame"))
                        .clicked()
                    {
                        let total = state.total_frames();
                        let current = state.current_frame;
                        if total > 1 {
                            if let Some(anim) = state.current_animation_mut() {
                                let frame_idx = current.min(anim.frames.len() - 1);
                                anim.frames.remove(frame_idx);
                            }
                            // Fix current_frame after mutation
                            let new_total = state.total_frames();
                            if state.current_frame >= new_total {
                                state.current_frame = new_total.saturating_sub(1);
                            }
                        }
                        ui.close_menu();
                    }
                },
            );

            ui.menu_button(
                egui::RichText::new("Export").size(menu_font_size),
                |ui| {
                    let has_animation = state
                        .current_animation()
                        .map(|a| !a.frames.is_empty())
                        .unwrap_or(false);
                    if ui
                        .add_enabled(
                            has_project && has_animation,
                            egui::Button::new("Export Current Animation..."),
                        )
                        .clicked()
                    {
                        if let Some(path) = pick_export_file() {
                            let path_str = path.to_string_lossy().to_string();
                            match export_current_animation(&state, &path_str) {
                                Ok((png_path, json_path)) => {
                                    state.set_status(format!(
                                        "Exported to {} and {}",
                                        png_path, json_path
                                    ));
                                }
                                Err(e) => {
                                    state.set_status(format!("Export failed: {}", e));
                                }
                            }
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(has_project, egui::Button::new("Export All Animations..."))
                        .clicked()
                    {
                        if let Some(path) = pick_export_folder() {
                            let path_str = path.to_string_lossy().to_string();
                            match export_all_animations(&state, &path_str) {
                                Ok(count) => {
                                    state.set_status(format!(
                                        "Exported {} animations to {}",
                                        count, path_str
                                    ));
                                }
                                Err(e) => {
                                    state.set_status(format!("Export failed: {}", e));
                                }
                            }
                        }
                        ui.close_menu();
                    }
                },
            );
        });
    });

    // Status bar (bottom-most panel, before timeline)
    egui::TopBottomPanel::bottom("status_bar")
        .max_height(24.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some((ref msg, ref when)) = state.status_message {
                    let relative_time = format_relative_time(when.elapsed());
                    ui.label(format!("{} ({})", msg, relative_time));
                } else {
                    ui.label("Ready");
                }

                // Version number floating right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("v{}", VERSION));
                });
            });
        });

    // Asset browser (left panel) - Simplified: flat character list + animations
    // Limit panel widths to ensure at least 25% of screen remains for canvas
    let screen_width = ctx.screen_rect().width();
    let max_panel_width = screen_width * 0.375;
    if state.project.is_some() {
        egui::SidePanel::left("asset_browser")
            .default_width(scaled_margin(250.0, ui_scale))
            .min_width(scaled_margin(200.0, ui_scale))
            .max_width(max_panel_width)
            .resizable(true)
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(0.0))
            .show(ctx, |ui| {
                render_asset_browser(ui, &mut state);
            });
    }

    // Inspector (right panel)
    if state.project.is_some() {
        egui::SidePanel::right("inspector")
            .default_width(scaled_margin(280.0, ui_scale))
            .min_width(scaled_margin(200.0, ui_scale))
            .max_width(max_panel_width)
            .resizable(true)
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(0.0))
            .show(ctx, |ui| {
                render_inspector(ui, &mut state);
            });
    }

    // Timeline (bottom panel) - only show on canvas tab when an animation is selected
    let is_canvas_tab = matches!(state.active_tab, ActiveTab::Canvas);
    if is_canvas_tab && state.current_animation().is_some() {
        render_timeline(ctx, &mut state);
    }

    // Central canvas area with tabs
    egui::CentralPanel::default()
        .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(egui::Margin::ZERO))
        .show(ctx, |ui| {
            render_central_panel(ui, &mut state, ctx);
        });
}

fn render_asset_browser(ui: &mut egui::Ui, state: &mut AppState) {
    if let Some(ref project) = state.project.clone() {
        // Project section
        egui::TopBottomPanel::top("project_section")
            .show_separator_line(true)
            .frame(
                egui::Frame::none()
                    .inner_margin(scaled_margin(DEFAULT_PANEL_MARGIN, state.config.ui_scale)),
            )
            .show_inside(ui, |ui| {
                ui.heading("Project");
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    let mut name = project.name.clone();
                    if ui.text_edit_singleline(&mut name).changed() {
                        if let Some(ref mut p) = state.project {
                            p.name = name;
                        }
                    }
                });
            });

        // Character section
        egui::TopBottomPanel::top("character_section")
            .show_separator_line(true)
            .frame(
                egui::Frame::none()
                    .inner_margin(scaled_margin(DEFAULT_PANEL_MARGIN, state.config.ui_scale)),
            )
            .show_inside(ui, |ui| {
                ui.heading("Character");
                let current_char_name = state
                    .active_character
                    .clone()
                    .unwrap_or_else(|| "(None)".to_string());
                egui::ComboBox::from_id_salt("character_selector")
                    .selected_text(&current_char_name)
                    .width((ui.available_width() - 10.0).max(10.0))
                    .show_ui(ui, |ui| {
                        for character in &project.characters {
                            let is_selected =
                                state.active_character.as_ref() == Some(&character.name);
                            if ui
                                .selectable_label(is_selected, &character.name)
                                .clicked()
                            {
                                state.active_character = Some(character.name.clone());
                                state.editor_selected_part =
                                    character.parts.first().map(|p| p.name.clone());
                                state.editor_selected_state = None;
                                state.current_animation = 0;
                                state.current_frame = 0;
                                state.needs_zoom_fit = true;
                            }
                        }
                        ui.separator();
                        if ui.selectable_label(false, "+ New Character...").clicked() {
                            state.show_new_character_dialog = true;
                            state.new_character_name.clear();
                        }
                    });

                // Character actions
                if let Some(ref active_char) = state.active_character.clone() {
                    ui.horizontal(|ui| {
                        if ui.small_button("Edit").clicked() {
                            state.active_tab = ActiveTab::CharacterEditor(active_char.clone());
                        }
                        if ui.small_button("Rename").clicked() {
                            state.context_menu_target = Some(ContextMenuTarget::Character {
                                char_name: active_char.clone(),
                            });
                            state.rename_new_name = active_char.clone();
                            state.show_rename_dialog = true;
                        }
                        if ui.small_button("Clone").clicked() {
                            state.clone_source_character = Some(active_char.clone());
                            state.clone_character_name = format!("{} (copy)", active_char);
                            state.show_clone_character_dialog = true;
                        }
                        if ui.small_button("Delete").clicked() {
                            state.context_menu_target = Some(ContextMenuTarget::Character {
                                char_name: active_char.clone(),
                            });
                            state.show_delete_confirm_dialog = true;
                        }
                    });
                }
            });

        // Animations section
        if let Some(ref active_char) = state.active_character.clone() {
            egui::TopBottomPanel::top("animations_section")
                .show_separator_line(true)
                .frame(
                    egui::Frame::none()
                        .inner_margin(scaled_margin(DEFAULT_PANEL_MARGIN, state.config.ui_scale)),
                )
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Animations");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let size = ui.available_height();
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("+")
                                            .strong()
                                            .size(scaled_font(16.0, state.config.ui_scale)),
                                    )
                                    .min_size(egui::vec2(size, size)),
                                )
                                .on_hover_text("New animation")
                                .clicked()
                            {
                                state.show_new_animation_dialog = true;
                                state.new_animation_name.clear();
                            }
                        });
                    });
                    if let Some(character) = project.get_character(&active_char) {
                        let available_width = ui.available_width();
                        egui::Frame::none()
                            .fill(egui::Color32::from_gray(35))
                            .rounding(4.0)
                            .inner_margin(4.0)
                            .show(ui, |ui| {
                                ui.set_width(available_width - 8.0);
                                for (i, anim) in character.animations.iter().enumerate() {
                                    let is_selected_anim = i == state.current_animation;
                                    let anim_text = if is_selected_anim {
                                        egui::RichText::new(&anim.name).strong()
                                    } else {
                                        egui::RichText::new(&anim.name)
                                    };

                                    let response = ui.add_sized(
                                        [ui.available_width(), 0.0],
                                        egui::SelectableLabel::new(is_selected_anim, anim_text),
                                    );
                                    if response.clicked() {
                                        state.current_animation = i;
                                        state.current_frame = 0;
                                    }

                                    let anim_name = anim.name.clone();
                                    let char_name_for_menu = active_char.clone();
                                    response.context_menu(|ui| {
                                        if ui.button("Rename...").clicked() {
                                            state.context_menu_target =
                                                Some(ContextMenuTarget::Animation {
                                                    char_name: char_name_for_menu.clone(),
                                                    anim_index: i,
                                                    anim_name: anim_name.clone(),
                                                });
                                            state.rename_new_name = anim_name.clone();
                                            state.show_rename_dialog = true;
                                            ui.close_menu();
                                        }
                                        if ui.button("Delete").clicked() {
                                            state.context_menu_target =
                                                Some(ContextMenuTarget::Animation {
                                                    char_name: char_name_for_menu.clone(),
                                                    anim_index: i,
                                                    anim_name: anim_name.clone(),
                                                });
                                            state.show_delete_confirm_dialog = true;
                                            ui.close_menu();
                                        }
                                    });
                                }
                            });
                    }
                });
        }

        // Parts Gallery section
        if let Some(ref active_char_name) = state.active_character.clone() {
            if let Some(character) = project.get_character(&active_char_name) {
                egui::TopBottomPanel::top("parts_gallery_section")
                    .show_separator_line(true)
                    .frame(
                        egui::Frame::none().inner_margin(scaled_margin(
                            DEFAULT_PANEL_MARGIN,
                            state.config.ui_scale,
                        )),
                    )
                    .show_inside(ui, |ui| {
                        ui.heading("Parts Gallery");
                        ui.label("(Drag to canvas)");

                        let gallery_parts: Vec<(String, String, Option<String>)> = character
                            .parts
                            .iter()
                            .map(|p| {
                                let thumb_data = p
                                    .states
                                    .first()
                                    .and_then(|s| s.rotations.get(&0))
                                    .and_then(|r| r.image_data.clone());
                                (
                                    p.name.clone(),
                                    p.states
                                        .first()
                                        .map(|s| s.name.clone())
                                        .unwrap_or_else(|| "default".to_string()),
                                    thumb_data,
                                )
                            })
                            .collect();

                        let char_id_for_gallery = character.id;
                        let char_name_for_gallery = active_char_name.clone();
                        let ui_scale = state.config.ui_scale;
                        let gallery_size = scaled_margin(48.0, ui_scale);
                        let gallery_spacing = scaled_margin(8.0, ui_scale);
                        let items_per_row = ((ui.available_width() - scaled_margin(20.0, ui_scale))
                            / (gallery_size + gallery_spacing))
                            .max(1.0) as usize;

                        egui::Grid::new("parts_gallery_grid")
                            .spacing([
                                scaled_margin(4.0, ui_scale),
                                scaled_margin(4.0, ui_scale),
                            ])
                            .show(ui, |ui| {
                                for (idx, (part_name, state_name, thumb_data)) in
                                    gallery_parts.iter().enumerate()
                                {
                                    let texture_key =
                                        format!("gallery/{}/{}", char_name_for_gallery, part_name);

                                    if let Some(ref data) = thumb_data {
                                        if !state.texture_cache.contains_key(&texture_key) {
                                            if let Ok(tex) =
                                                decode_base64_to_texture(ui.ctx(), &texture_key, data)
                                            {
                                                state.texture_cache.insert(texture_key.clone(), tex);
                                            }
                                        }
                                    }

                                    let label_height = scaled_margin(14.0, ui_scale);
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(gallery_size, gallery_size + label_height),
                                        egui::Sense::drag(),
                                    );

                                    let image_rect = egui::Rect::from_min_size(
                                        rect.min,
                                        egui::vec2(gallery_size, gallery_size),
                                    );

                                    let bg_color = if response.dragged() || response.hovered() {
                                        egui::Color32::from_rgb(80, 80, 100)
                                    } else {
                                        egui::Color32::from_rgb(50, 50, 60)
                                    };
                                    ui.painter().rect_filled(image_rect, 4.0, bg_color);

                                    if let Some(texture) = state.texture_cache.get(&texture_key) {
                                        let uv = egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        );
                                        ui.painter().image(
                                            texture.id(),
                                            image_rect.shrink(2.0),
                                            uv,
                                            egui::Color32::WHITE,
                                        );
                                    } else {
                                        ui.painter().text(
                                            image_rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            &part_name.chars().take(3).collect::<String>(),
                                            egui::FontId::proportional(scaled_font(
                                                12.0,
                                                state.config.ui_scale,
                                            )),
                                            egui::Color32::GRAY,
                                        );
                                    }

                                    ui.painter().rect_stroke(
                                        image_rect,
                                        4.0,
                                        egui::Stroke::new(
                                            1.0,
                                            egui::Color32::from_rgb(100, 100, 120),
                                        ),
                                    );

                                    let label_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x, rect.min.y + gallery_size + 1.0),
                                        egui::vec2(
                                            gallery_size,
                                            scaled_font(12.0, state.config.ui_scale),
                                        ),
                                    );
                                    let truncated_name: String = if part_name.len() > 6 {
                                        format!("{}...", &part_name[..5])
                                    } else {
                                        part_name.clone()
                                    };
                                    ui.painter().text(
                                        label_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        truncated_name,
                                        egui::FontId::proportional(scaled_font(
                                            12.0,
                                            state.config.ui_scale,
                                        )),
                                        egui::Color32::WHITE,
                                    );

                                    if response.dragged() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                    } else if response.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                                    }

                                    if response.drag_started() {
                                        state.gallery_drag = Some(GalleryDrag {
                                            character_id: char_id_for_gallery,
                                            character_name: char_name_for_gallery.clone(),
                                            part_name: part_name.clone(),
                                            state_name: state_name.clone(),
                                        });
                                    }

                                    if (idx + 1) % items_per_row == 0 {
                                        ui.end_row();
                                    }
                                }
                            });
                    });
            }
        }
    }
}

fn render_inspector(ui: &mut egui::Ui, state: &mut AppState) {
    // Inspector section
    egui::TopBottomPanel::top("inspector_section")
        .show_separator_line(true)
        .frame(
            egui::Frame::none()
                .inner_margin(scaled_margin(DEFAULT_PANEL_MARGIN, state.config.ui_scale)),
        )
        .show_inside(ui, |ui| {
            ui.heading("Inspector");

            let inspector_height = scaled_margin(160.0, state.config.ui_scale);
            let available_width = ui.available_width().max(1.0);
            ui.allocate_ui_with_layout(
                egui::vec2(available_width, inspector_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.set_min_height(inspector_height);

                    let selected_info = state.get_selected_placed_part().map(|p| {
                        (
                            p.character_id,
                            p.part_name.clone(),
                            p.state_name.clone(),
                            p.position,
                            p.rotation,
                            p.z_override,
                        )
                    });

                    let available_states: Vec<String> =
                        if let Some((character_id, ref part_name, _, _, _, _)) = selected_info {
                            state
                                .project
                                .as_ref()
                                .and_then(|p| p.get_character_by_id(character_id))
                                .and_then(|c| c.get_part(part_name))
                                .map(|p| p.states.iter().map(|s| s.name.clone()).collect())
                                .unwrap_or_default()
                        } else {
                            vec![]
                        };

                    if let Some((
                        _character_id,
                        part_name,
                        current_state,
                        position,
                        rotation,
                        _z_override,
                    )) = selected_info
                    {
                        ui.label(format!("Selected layer: {}", part_name));
                        ui.separator();

                        let mut selected_state = current_state.clone();
                        ui.horizontal(|ui| {
                            ui.label("State:");
                            egui::ComboBox::from_id_salt("part_state")
                                .selected_text(&selected_state)
                                .show_ui(ui, |ui| {
                                    for state_name in &available_states {
                                        if ui
                                            .selectable_value(
                                                &mut selected_state,
                                                state_name.clone(),
                                                state_name,
                                            )
                                            .changed()
                                        {
                                            if let Some(part) = state.get_selected_placed_part_mut()
                                            {
                                                part.state_name = selected_state.clone();
                                                state.texture_cache.clear();
                                            }
                                        }
                                    }
                                });
                        });

                        ui.horizontal(|ui| {
                            ui.label("Position:");
                            let was_pixel_aligned = state.pixel_aligned;
                            if ui
                                .checkbox(&mut state.pixel_aligned, "Pixel aligned")
                                .changed()
                            {
                                if !was_pixel_aligned && state.pixel_aligned {
                                    if let Some(part) = state.get_selected_placed_part_mut() {
                                        part.position.0 = part.position.0.round();
                                        part.position.1 = part.position.1.round();
                                    }
                                }
                            }
                        });
                        let mut pos_x = position.0;
                        let mut pos_y = position.1;
                        let pixel_aligned = state.pixel_aligned;
                        ui.horizontal(|ui| {
                            ui.label("  X:");
                            if ui
                                .add(egui::DragValue::new(&mut pos_x).speed(1.0))
                                .changed()
                            {
                                if let Some(part) = state.get_selected_placed_part_mut() {
                                    part.position.0 =
                                        if pixel_aligned { pos_x.round() } else { pos_x };
                                }
                            }
                            ui.label("  Y:");
                            if ui
                                .add(egui::DragValue::new(&mut pos_y).speed(1.0))
                                .changed()
                            {
                                if let Some(part) = state.get_selected_placed_part_mut() {
                                    part.position.1 =
                                        if pixel_aligned { pos_y.round() } else { pos_y };
                                }
                            }
                        });

                        let mut rot = rotation;
                        ui.horizontal(|ui| {
                            ui.label("Rotation:");
                            egui::ComboBox::from_id_salt("part_rotation")
                                .selected_text(format!("{}°", rot))
                                .show_ui(ui, |ui| {
                                    for angle in [0, 45, 90, 135, 180, 225, 270, 315] {
                                        if ui
                                            .selectable_value(&mut rot, angle, format!("{}°", angle))
                                            .changed()
                                        {
                                            if let Some(part) = state.get_selected_placed_part_mut()
                                            {
                                                part.rotation = rot;
                                            }
                                        }
                                    }
                                });
                        });
                    } else {
                        ui.label("No layer selected");
                    }
                },
            );
        });

    // Layers section
    render_layers_panel(ui, state);

    // Reference Image section
    render_reference_panel(ui, state);
}

fn render_layers_panel(ui: &mut egui::Ui, state: &mut AppState) {
    egui::TopBottomPanel::top("layers_section")
        .show_separator_line(true)
        .frame(
            egui::Frame::none()
                .inner_margin(scaled_margin(DEFAULT_PANEL_MARGIN, state.config.ui_scale)),
        )
        .show_inside(ui, |ui| {
            ui.heading("Layers");

            let layers: Vec<(u64, String, usize, bool, String, u16)> = {
                if let Some(anim) = state.current_animation() {
                    if let Some(frame) = anim.frames.get(state.current_frame) {
                        frame
                            .placed_parts
                            .iter()
                            .enumerate()
                            .map(|(idx, p)| {
                                (
                                    p.id,
                                    if p.layer_name.is_empty() {
                                        p.part_name.clone()
                                    } else {
                                        p.layer_name.clone()
                                    },
                                    idx,
                                    p.visible,
                                    p.state_name.clone(),
                                    p.rotation,
                                )
                            })
                            .collect()
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            };

            let mut move_up: Option<usize> = None;
            let mut move_down: Option<usize> = None;
            let mut move_to_top: Option<usize> = None;
            let mut move_to_bottom: Option<usize> = None;
            let mut toggle_visibility: Option<usize> = None;

            if layers.is_empty() {
                ui.label("(No layers)");
            } else {
                let available_width = ui.available_width();
                let layers_len = layers.len();
                let ui_scale = state.config.ui_scale;
                let button_width = scaled_margin(22.0, ui_scale);
                let buttons_total = button_width * 4.0 + scaled_margin(16.0, ui_scale);
                let name_width =
                    (available_width - scaled_margin(8.0, ui_scale) - buttons_total)
                        .max(scaled_margin(50.0, ui_scale));

                egui::Frame::none()
                    .fill(egui::Color32::from_gray(35))
                    .rounding(scaled_margin(4.0, ui_scale))
                    .inner_margin(scaled_margin(4.0, ui_scale))
                    .show(ui, |ui| {
                        ui.set_width(available_width - scaled_margin(8.0, ui_scale));
                        egui::Grid::new("layers_grid")
                            .num_columns(7)
                            .min_col_width(0.0)
                            .spacing([
                                scaled_margin(4.0, ui_scale),
                                scaled_margin(2.0, ui_scale),
                            ])
                            .show(ui, |ui| {
                                for (id, name, idx, visible, state_name, rotation) in
                                    layers.iter().rev()
                                {
                                    let is_selected = state.selected_part_id == Some(*id);
                                    let layer_id = *id;
                                    let layer_name = name.clone();
                                    let row_height = ui.spacing().interact_size.y;

                                    let eye_icon = if *visible { "👁" } else { "○" };
                                    if ui
                                        .add_sized(
                                            [button_width, row_height],
                                            egui::Button::new(eye_icon).small(),
                                        )
                                        .on_hover_text(if *visible {
                                            "Hide layer"
                                        } else {
                                            "Show layer"
                                        })
                                        .clicked()
                                    {
                                        toggle_visibility = Some(*idx);
                                    }

                                    let label = if is_selected {
                                        egui::RichText::new(name.as_str()).strong()
                                    } else {
                                        egui::RichText::new(name.as_str())
                                    };
                                    let response = ui.add_sized(
                                        [
                                            (name_width - scaled_margin(80.0, ui_scale)).max(1.0),
                                            row_height,
                                        ],
                                        egui::SelectableLabel::new(is_selected, label),
                                    );
                                    if response.clicked() {
                                        state.selected_part_id = Some(*id);
                                        state.selection_time = Some(std::time::Instant::now());
                                    }
                                    response.context_menu(|ui| {
                                        if ui.button("Delete").clicked() {
                                            state.context_menu_target =
                                                Some(ContextMenuTarget::Layer {
                                                    layer_id,
                                                    layer_name: layer_name.clone(),
                                                });
                                            state.show_delete_confirm_dialog = true;
                                            ui.close_menu();
                                        }
                                    });

                                    let state_badge = egui::Frame::none()
                                        .fill(egui::Color32::from_rgb(50, 80, 130))
                                        .rounding(scaled_margin(3.0, ui_scale))
                                        .inner_margin(egui::Margin::symmetric(
                                            scaled_margin(4.0, ui_scale),
                                            scaled_margin(1.0, ui_scale),
                                        ));
                                    state_badge.show(ui, |ui| {
                                        ui.label(
                                            egui::RichText::new(state_name)
                                                .small()
                                                .color(egui::Color32::WHITE),
                                        );
                                    });

                                    let rot_badge = egui::Frame::none()
                                        .fill(egui::Color32::from_rgb(50, 100, 60))
                                        .rounding(scaled_margin(3.0, ui_scale))
                                        .inner_margin(egui::Margin::symmetric(
                                            scaled_margin(4.0, ui_scale),
                                            scaled_margin(1.0, ui_scale),
                                        ));
                                    rot_badge.show(ui, |ui| {
                                        ui.label(
                                            egui::RichText::new(format!("{}°", rotation))
                                                .small()
                                                .color(egui::Color32::WHITE),
                                        );
                                    });

                                    let can_move_up = *idx < layers_len - 1;
                                    let shift_held = ui.input(|i| i.modifiers.shift);
                                    if ui
                                        .add_sized(
                                            [button_width, row_height],
                                            egui::Button::new("⏶").small().sense(if can_move_up {
                                                egui::Sense::click()
                                            } else {
                                                egui::Sense::hover()
                                            }),
                                        )
                                        .on_hover_text("Move layer up (Shift-Click to move to top)")
                                        .clicked()
                                        && can_move_up
                                    {
                                        if shift_held {
                                            move_to_top = Some(*idx);
                                        } else {
                                            move_up = Some(*idx);
                                        }
                                    }

                                    let can_move_down = *idx > 0;
                                    if ui
                                        .add_sized(
                                            [button_width, row_height],
                                            egui::Button::new("⏷").small().sense(
                                                if can_move_down {
                                                    egui::Sense::click()
                                                } else {
                                                    egui::Sense::hover()
                                                },
                                            ),
                                        )
                                        .on_hover_text(
                                            "Move layer down (Shift-Click to move to bottom)",
                                        )
                                        .clicked()
                                        && can_move_down
                                    {
                                        if shift_held {
                                            move_to_bottom = Some(*idx);
                                        } else {
                                            move_down = Some(*idx);
                                        }
                                    }

                                    if ui
                                        .add_sized(
                                            [button_width, row_height],
                                            egui::Button::new("×").small(),
                                        )
                                        .on_hover_text("Delete layer")
                                        .clicked()
                                    {
                                        state.context_menu_target = Some(ContextMenuTarget::Layer {
                                            layer_id,
                                            layer_name: layer_name.clone(),
                                        });
                                        state.show_delete_confirm_dialog = true;
                                    }

                                    ui.end_row();
                                }
                            });
                    });
            }

            // Apply visibility toggle
            if let Some(idx) = toggle_visibility {
                let current_anim = state.current_animation;
                let current_frame_idx = state.current_frame;
                if let Some(ref char_name) = state.active_character.clone() {
                    if let Some(ref mut project) = state.project {
                        if let Some(character) = project.get_character_mut(char_name) {
                            if let Some(anim) = character.animations.get_mut(current_anim) {
                                if let Some(frame) = anim.frames.get_mut(current_frame_idx) {
                                    if let Some(part) = frame.placed_parts.get_mut(idx) {
                                        part.visible = !part.visible;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Apply layer reordering
            let current_anim = state.current_animation;
            let current_frame_idx = state.current_frame;
            let active_char = state.active_character.clone();
            if let Some(idx) = move_up {
                if let Some(ref mut project) = state.project {
                    if let Some(ref char_name) = active_char {
                        if let Some(character) = project.get_character_mut(char_name) {
                            if let Some(anim) = character.animations.get_mut(current_anim) {
                                if let Some(frame) = anim.frames.get_mut(current_frame_idx) {
                                    if idx + 1 < frame.placed_parts.len() {
                                        frame.placed_parts.swap(idx, idx + 1);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(idx) = move_down {
                if let Some(ref mut project) = state.project {
                    if let Some(ref char_name) = active_char {
                        if let Some(character) = project.get_character_mut(char_name) {
                            if let Some(anim) = character.animations.get_mut(current_anim) {
                                if let Some(frame) = anim.frames.get_mut(current_frame_idx) {
                                    if idx > 0 {
                                        frame.placed_parts.swap(idx, idx - 1);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(idx) = move_to_top {
                if let Some(ref mut project) = state.project {
                    if let Some(ref char_name) = active_char {
                        if let Some(character) = project.get_character_mut(char_name) {
                            if let Some(anim) = character.animations.get_mut(current_anim) {
                                if let Some(frame) = anim.frames.get_mut(current_frame_idx) {
                                    let len = frame.placed_parts.len();
                                    if idx < len - 1 {
                                        let part = frame.placed_parts.remove(idx);
                                        frame.placed_parts.push(part);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(idx) = move_to_bottom {
                if let Some(ref mut project) = state.project {
                    if let Some(ref char_name) = active_char {
                        if let Some(character) = project.get_character_mut(char_name) {
                            if let Some(anim) = character.animations.get_mut(current_anim) {
                                if let Some(frame) = anim.frames.get_mut(current_frame_idx) {
                                    if idx > 0 {
                                        let part = frame.placed_parts.remove(idx);
                                        frame.placed_parts.insert(0, part);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
}

fn render_reference_panel(ui: &mut egui::Ui, state: &mut AppState) {
    use crate::file::pick_image_file;
    use crate::model;

    egui::TopBottomPanel::top("reference_section")
        .show_separator_line(true)
        .frame(
            egui::Frame::none()
                .inner_margin(scaled_margin(DEFAULT_PANEL_MARGIN, state.config.ui_scale)),
        )
        .show_inside(ui, |ui| {
            ui.heading("Reference Image");

            let char_name = state.active_character.clone();
            let current_anim = state.current_animation;
            let current_frame_idx = state.current_frame;

            let current_frame_ref = state.project.as_ref().and_then(|p| {
                char_name.as_ref().and_then(|cn| {
                    p.get_character(cn).and_then(|c| {
                        c.animations.get(current_anim).and_then(|a| {
                            a.frames
                                .get(current_frame_idx)
                                .and_then(|f| f.reference.as_ref().map(|r| r.file_path.clone()))
                        })
                    })
                })
            });

            let using_fallback = current_frame_ref
                .as_ref()
                .and_then(|path| state.reference_using_fallback.get(path).copied())
                .unwrap_or(false);

            let mut load_clicked = false;
            let mut clear_clicked = false;
            let mut copy_to_all_clicked = false;
            let mut copy_settings: Option<(f32, (f32, f32))> = None;

            ui.horizontal(|ui| {
                if ui.button("Load Image...").clicked() {
                    load_clicked = true;
                }

                if current_frame_ref.is_some() {
                    if ui.button("Clear").clicked() {
                        clear_clicked = true;
                    }
                }
            });

            let load_ref_path = if load_clicked {
                pick_image_file().map(|p| p.to_string_lossy().to_string())
            } else {
                None
            };
            let clear_ref = clear_clicked;

            if let Some(path_str) = load_ref_path {
                if let Some(ref mut project) = state.project {
                    if !project.reference_thumbnails.contains_key(&path_str) {
                        if let Ok((thumbnail, _original_size)) =
                            create_reference_thumbnail(&path_str, 256)
                        {
                            project
                                .reference_thumbnails
                                .insert(path_str.clone(), thumbnail);
                        }
                    }

                    if let Some(ref cn) = char_name {
                        let canvas_size = project
                            .get_character(cn)
                            .map(|c| c.canvas_size)
                            .unwrap_or((64, 64));

                        let scale = if let Ok(bytes) = fs::read(&path_str) {
                            if let Ok(img) = image::load_from_memory(&bytes) {
                                calculate_fit_scale((img.width(), img.height()), canvas_size)
                            } else {
                                1.0
                            }
                        } else {
                            1.0
                        };

                        if let Some(character) = project.get_character_mut(cn) {
                            if let Some(anim) = character.animations.get_mut(current_anim) {
                                if let Some(frame) = anim.frames.get_mut(current_frame_idx) {
                                    frame.reference =
                                        Some(model::FrameReference::new(path_str.clone(), scale));
                                }
                            }
                        }
                    }

                    state.reference_texture_cache.remove(&path_str);
                    state.set_status(format!("Loaded reference: {}", path_str));
                }
            }

            if clear_ref {
                if let Some(ref mut project) = state.project {
                    if let Some(ref cn) = char_name {
                        if let Some(character) = project.get_character_mut(cn) {
                            if let Some(anim) = character.animations.get_mut(current_anim) {
                                if let Some(frame) = anim.frames.get_mut(current_frame_idx) {
                                    frame.reference = None;
                                }
                            }
                        }
                    }
                }
                state.set_status("Cleared reference image");
            }

            if let Some(ref mut project) = state.project {
                if let Some(ref cn) = char_name {
                    if let Some(character) = project.get_character_mut(cn) {
                        if let Some(anim) = character.animations.get_mut(current_anim) {
                            if let Some(frame) = anim.frames.get_mut(current_frame_idx) {
                                if let Some(ref mut frame_ref) = frame.reference {
                                    ui.horizontal(|ui| {
                                        ui.label("Scale:");
                                        ui.add(
                                            egui::DragValue::new(&mut frame_ref.scale)
                                                .speed(0.01)
                                                .range(0.01..=10.0),
                                        );
                                    });

                                    ui.horizontal(|ui| {
                                        ui.label("Position:");
                                        ui.add(
                                            egui::DragValue::new(&mut frame_ref.position.0)
                                                .prefix("X: ")
                                                .speed(1.0),
                                        );
                                        ui.add(
                                            egui::DragValue::new(&mut frame_ref.position.1)
                                                .prefix("Y: ")
                                                .speed(1.0),
                                        );
                                    });

                                    let display_path = if frame_ref.file_path.len() > 30 {
                                        format!(
                                            "...{}",
                                            &frame_ref.file_path[frame_ref.file_path.len() - 27..]
                                        )
                                    } else {
                                        frame_ref.file_path.clone()
                                    };

                                    if using_fallback {
                                        ui.colored_label(
                                            egui::Color32::YELLOW,
                                            "File missing (using thumbnail)",
                                        );
                                    }
                                    ui.small(&display_path);

                                    ui.add_space(4.0);
                                    if ui.button("Copy to all frames").clicked() {
                                        copy_to_all_clicked = true;
                                        copy_settings =
                                            Some((frame_ref.scale, frame_ref.position));
                                    }

                                    ui.small("Shift+MMB/Space to drag");
                                }
                            }
                        }
                    }
                }
            }

            if copy_to_all_clicked {
                if let Some((scale, position)) = copy_settings {
                    if let Some(ref mut project) = state.project {
                        if let Some(ref cn) = char_name {
                            if let Some(character) = project.get_character_mut(cn) {
                                if let Some(anim) = character.animations.get_mut(current_anim) {
                                    let mut count = 0;
                                    for (i, frame) in anim.frames.iter_mut().enumerate() {
                                        if i != current_frame_idx {
                                            if let Some(ref mut frame_ref) = frame.reference {
                                                frame_ref.scale = scale;
                                                frame_ref.position = position;
                                                count += 1;
                                            }
                                        }
                                    }
                                    state.set_status(format!(
                                        "Copied settings to {} other frames",
                                        count
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        });
}

fn render_timeline(ctx: &egui::Context, state: &mut AppState) {
    use crate::file::pick_save_file;

    let total_frames = state.total_frames();
    let timeline_height = scaled_margin(120.0, state.config.ui_scale);
    egui::TopBottomPanel::bottom("timeline")
        .exact_height(timeline_height)
        .show(ctx, |ui| {
            egui::TopBottomPanel::top("timeline_controls")
                .show_separator_line(true)
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("Timeline");

                        if let Some(anim) = state.current_animation() {
                            ui.separator();
                            ui.label(&anim.name);
                        }

                        ui.separator();

                        let play_text = if state.is_playing { "⏸" } else { "▶" };
                        let play_tooltip = if state.is_playing {
                            "Pause (Enter)"
                        } else {
                            "Play (Enter)"
                        };
                        if ui
                            .button(play_text)
                            .on_hover_text(play_tooltip)
                            .clicked()
                        {
                            state.is_playing = !state.is_playing;
                            if state.is_playing {
                                state.playback_time = 0.0;
                                state.selected_part_id = None;
                            }
                        }
                        if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
                            if state.project_path.is_some() {
                                match state.save_project() {
                                    Ok(()) => state.set_status("Project saved"),
                                    Err(e) => state.set_status(format!("Save failed: {}", e)),
                                }
                            } else if let Some(path) = pick_save_file() {
                                let path_str = path.to_string_lossy().to_string();
                                match state.save_project_as(&path_str) {
                                    Ok(()) => state.set_status(format!("Saved to {}", path_str)),
                                    Err(e) => state.set_status(format!("Save failed: {}", e)),
                                }
                            }
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            state.is_playing = !state.is_playing;
                            if state.is_playing {
                                state.playback_time = 0.0;
                                state.selected_part_id = None;
                            }
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Delete))
                            && state.selected_part_id.is_some()
                        {
                            state.delete_selected_part();
                            state.set_status("Part deleted");
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) && total_frames > 0 {
                            state.current_frame = if state.current_frame == 0 {
                                total_frames - 1
                            } else {
                                state.current_frame - 1
                            };
                            state.playback_time = 0.0;
                            state.selected_part_id = None;
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) && total_frames > 0 {
                            state.current_frame = (state.current_frame + 1) % total_frames;
                            state.playback_time = 0.0;
                            state.selected_part_id = None;
                        }
                        if ui.button("⏹").clicked() {
                            state.is_playing = false;
                            state.current_frame = 0;
                            state.playback_time = 0.0;
                        }
                        if ui.button("⏮").clicked() && state.current_frame > 0 {
                            state.current_frame -= 1;
                            state.playback_time = 0.0;
                            state.selected_part_id = None;
                        }
                        if ui.button("⏭").clicked() && state.current_frame < total_frames - 1 {
                            state.current_frame += 1;
                            state.playback_time = 0.0;
                            state.selected_part_id = None;
                        }

                        ui.separator();
                        ui.label(format!(
                            "Frame: {} / {}",
                            state.current_frame + 1,
                            total_frames
                        ));

                        ui.separator();
                        ui.label("FPS:");
                        let mut fps = state.current_animation().map(|a| a.fps).unwrap_or(12);
                        if ui
                            .add(egui::DragValue::new(&mut fps).speed(0.1).range(1..=60))
                            .changed()
                        {
                            if let Some(anim) = state.current_animation_mut() {
                                anim.fps = fps;
                            }
                        }
                    });
                });

            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    let char_name = state.active_character.clone();
                    let anim_idx = state.current_animation;
                    for frame in 0..total_frames {
                        let is_current = frame == state.current_frame;
                        let text = format!("{}", frame + 1);

                        let button = if is_current {
                            egui::Button::new(egui::RichText::new(text).strong())
                                .fill(egui::Color32::from_rgb(80, 120, 180))
                        } else {
                            egui::Button::new(text)
                        };

                        let response = ui.add_sized([40.0, 60.0], button);
                        if response.clicked() {
                            state.current_frame = frame;
                            state.selected_part_id = None;
                        }
                        if let Some(ref cn) = char_name {
                            let cn = cn.clone();
                            response.context_menu(|ui| {
                                if ui.button("Delete Frame").clicked() {
                                    state.context_menu_target = Some(ContextMenuTarget::Frame {
                                        char_name: cn,
                                        anim_index: anim_idx,
                                        frame_index: frame,
                                    });
                                    state.show_delete_confirm_dialog = true;
                                    ui.close_menu();
                                }
                            });
                        }
                    }
                    let mut add_blank = false;
                    let mut add_copy = false;
                    ui.vertical(|ui| {
                        if ui.button("New blank frame").clicked() {
                            add_blank = true;
                        }
                        if ui.button("Duplicate last frame").clicked() {
                            add_copy = true;
                        }
                    });
                    if add_blank {
                        if let Some(anim) = state.current_animation_mut() {
                            anim.add_frame();
                        }
                    }
                    if add_copy {
                        let cloned_frame = state
                            .current_animation()
                            .and_then(|a| a.frames.last())
                            .cloned();
                        if let Some(mut new_frame) = cloned_frame {
                            if let Some(ref mut project) = state.project {
                                for part in &mut new_frame.placed_parts {
                                    part.id = project.next_id();
                                }
                            }
                            if let Some(anim) = state.current_animation_mut() {
                                anim.frames.push(new_frame);
                            }
                        }
                    }
                });
            });
        });
}

fn render_central_panel(ui: &mut egui::Ui, state: &mut AppState, ctx: &egui::Context) {
    if state.project.is_none() {
        render_welcome_screen(ui, state);
    } else {
        if state.active_character.is_some() {
            let ui_scale = state.config.ui_scale;
            egui::TopBottomPanel::top("tab_bar")
                .show_separator_line(true)
                .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::Margin {
                    left: scaled_margin(16.0, ui_scale),
                    right: scaled_margin(16.0, ui_scale),
                    top: scaled_margin(8.0, ui_scale),
                    bottom: 0.0,
                }))
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        let is_canvas = matches!(state.active_tab, ActiveTab::Canvas);
                        if tab_button(ui, is_canvas, "Canvas", ui_scale).clicked() {
                            state.active_tab = ActiveTab::Canvas;
                        }

                        if let Some(ref char_name) = state.active_character {
                            ui.add_space(scaled_margin(2.0, ui_scale));
                            let is_editor = matches!(state.active_tab, ActiveTab::CharacterEditor(_));
                            if tab_button(
                                ui,
                                is_editor,
                                format!("Edit Character: {}", char_name),
                                ui_scale,
                            )
                            .clicked()
                            {
                                state.active_tab = ActiveTab::CharacterEditor(char_name.clone());
                            }
                        }
                    });
                });

            match &state.active_tab {
                ActiveTab::Canvas => {
                    egui::TopBottomPanel::top("view_options")
                        .show_separator_line(true)
                        .show_inside(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.heading("View");
                                ui.separator();
                                ui.label("Zoom:");
                                egui::ComboBox::from_id_salt("zoom_level_canvas")
                                    .selected_text(format_zoom(state.zoom_level))
                                    .width(50.0)
                                    .show_ui(ui, |ui| {
                                        for &level in &ZOOM_LEVELS {
                                            ui.selectable_value(
                                                &mut state.zoom_level,
                                                level,
                                                format_zoom(level),
                                            );
                                        }
                                    });
                                ui.label("Show:");
                                ui.checkbox(&mut state.show_grid, "Grid");
                                ui.checkbox(&mut state.show_labels, "Labels");

                                ui.separator();
                                ui.heading("Reference image");
                                ui.separator();
                                ui.label("Alpha:");
                                let mut alpha_percent =
                                    (state.reference_opacity * 10.0).round() as i32;
                                ui.add_sized(
                                    [60.0, 18.0],
                                    egui::Slider::new(&mut alpha_percent, 0..=10).show_value(false),
                                );
                                state.reference_opacity = alpha_percent as f32 / 10.0;
                                ui.label(format!("{}%", alpha_percent * 10));
                                ui.checkbox(&mut state.reference_show_on_top, "On top");
                            });
                        });

                    egui::CentralPanel::default()
                        .frame(egui::Frame::none().fill(egui::Color32::from_gray(10)))
                        .show_inside(ui, |ui| {
                            render_canvas(ui, state);
                        });
                }
                ActiveTab::CharacterEditor(_) => {
                    if let Some(char_name) = state.active_character.clone() {
                        render_character_editor(ui, state, &char_name);
                    }
                }
            }
        } else {
            let ui_scale = state.config.ui_scale;
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() / 3.0);
                    ui.label(
                        egui::RichText::new("Select or create a character in the left panel.")
                            .size(scaled_font(16.0, ui_scale))
                            .color(egui::Color32::GRAY),
                    );
                });
            });
        }
    }
}

fn render_welcome_screen(ui: &mut egui::Ui, state: &mut AppState) {
    let ui_scale = state.config.ui_scale;
    ui.vertical_centered(|ui| {
        ui.add_space(scaled_margin(40.0, ui_scale));
        ui.label(
            egui::RichText::new("Welcome to Pixel Sprite Studio!")
                .size(scaled_font(28.0, ui_scale))
                .strong(),
        );
        ui.label(
            egui::RichText::new("by Elle Trudgett")
                .size(scaled_font(18.0, ui_scale))
                .color(egui::Color32::GRAY),
        );
        ui.label(format!("v{}", VERSION));
        ui.add_space(scaled_margin(20.0, ui_scale));

        let button_size = egui::vec2(scaled_margin(160.0, ui_scale), scaled_margin(40.0, ui_scale));
        let mut open_project_path: Option<String> = None;
        let total_buttons_width = button_size.x * 2.0 + scaled_margin(10.0, ui_scale);
        let available = ui.available_width();
        ui.horizontal(|ui| {
            ui.add_space(((available - total_buttons_width) / 2.0).max(0.0));
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("New Project").size(scaled_font(18.0, ui_scale)),
                    )
                    .min_size(button_size),
                )
                .clicked()
            {
                state.new_project();
            }
            ui.add_space(scaled_margin(10.0, ui_scale));
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("Open Project...").size(scaled_font(18.0, ui_scale)),
                    )
                    .min_size(button_size),
                )
                .clicked()
            {
                if let Some(path) = pick_open_file() {
                    open_project_path = Some(path.to_string_lossy().to_string());
                }
            }
        });
        if let Some(path_str) = open_project_path {
            match state.load_project(&path_str) {
                Ok(()) => state.set_status(format!("Loaded {}", path_str)),
                Err(e) => state.set_status(format!("Load failed: {}", e)),
            }
        }

        // Recent Projects
        let recent = state.config.recent_projects.clone();
        if !recent.is_empty() {
            ui.add_space(scaled_margin(30.0, ui_scale));
            ui.heading("Recent Projects");
            ui.add_space(scaled_margin(15.0, ui_scale));

            let mut project_to_open: Option<String> = None;
            let mut project_to_remove: Option<String> = None;

            for path in &recent {
                let path_buf = PathBuf::from(path);
                let filename = path_buf
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                let modified_ago = std::fs::metadata(&path_buf)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|time| time.elapsed().ok())
                    .map(format_relative_time)
                    .unwrap_or_else(|| "unknown".to_string());

                let loaded_project = std::fs::read_to_string(&path_buf)
                    .ok()
                    .and_then(|json| Project::from_json(&json).ok());
                let project_name = loaded_project
                    .as_ref()
                    .map(|p| p.name.clone())
                    .filter(|n| !n.is_empty() && n != "Untitled");
                let characters: Vec<String> = loaded_project
                    .map(|p| p.characters.iter().map(|c| c.name.clone()).collect())
                    .unwrap_or_default();
                let display_name = project_name.unwrap_or_else(|| filename.clone());

                let char_width = scaled_margin(7.0, ui_scale);
                let min_card_width = scaled_margin(300.0, ui_scale);
                let estimated_card_width = (path.len() as f32 * char_width).max(min_card_width);
                let card_response = ui.horizontal(|ui| {
                    let available = ui.available_width();
                    ui.add_space(((available - estimated_card_width) / 2.0).max(0.0));

                    let frame_response = egui::Frame::none()
                        .fill(egui::Color32::from_rgb(45, 45, 55))
                        .rounding(scaled_margin(8.0, ui_scale))
                        .inner_margin(scaled_margin(12.0, ui_scale))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.heading(&display_name);

                                ui.label(
                                    egui::RichText::new(&modified_ago)
                                        .size(scaled_font(11.0, ui_scale))
                                        .color(egui::Color32::from_gray(120)),
                                );

                                ui.label(
                                    egui::RichText::new(path)
                                        .size(scaled_font(12.0, ui_scale))
                                        .color(egui::Color32::GRAY),
                                );

                                if !characters.is_empty() {
                                    ui.add_space(scaled_margin(8.0, ui_scale));
                                    ui.label(
                                        egui::RichText::new("Characters:")
                                            .size(scaled_font(12.0, ui_scale))
                                            .strong(),
                                    );
                                    for char_name in &characters {
                                        ui.label(
                                            egui::RichText::new(format!("• {}", char_name))
                                                .size(scaled_font(12.0, ui_scale)),
                                        );
                                    }
                                }
                            });
                        });

                    ui.add_space(ui.available_width());

                    frame_response
                });

                let card_rect = card_response.inner.response.rect;

                let button_size_small = scaled_margin(24.0, ui_scale);
                let button_margin = scaled_margin(4.0, ui_scale);
                let button_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        card_rect.right() - button_size_small - button_margin,
                        card_rect.top() + button_margin,
                    ),
                    egui::vec2(button_size_small, button_size_small),
                );
                let button_response =
                    ui.interact(button_rect, ui.id().with(path), egui::Sense::click());
                let visuals = ui.style().interact(&button_response);
                ui.painter().rect_filled(button_rect, 4.0, visuals.bg_fill);
                ui.painter().text(
                    button_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "×",
                    egui::FontId::proportional(scaled_font(18.0, ui_scale)),
                    visuals.text_color(),
                );
                if button_response
                    .on_hover_text("Remove from recent")
                    .clicked()
                {
                    project_to_remove = Some(path.clone());
                }

                let hovering_button = ui.rect_contains_pointer(button_rect);
                if ui.rect_contains_pointer(card_rect) && !hovering_button {
                    ui.painter().rect_stroke(
                        card_rect,
                        8.0,
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 150, 255)),
                    );
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);

                    if ui.input(|i| i.pointer.primary_clicked()) {
                        project_to_open = Some(path.clone());
                    }
                }

                ui.add_space(10.0);
            }

            if let Some(path) = project_to_open {
                match state.load_project(&path) {
                    Ok(()) => state.set_status(format!("Loaded {}", path)),
                    Err(e) => {
                        state.set_status(format!("Load failed: {}", e));
                        if e.contains("Read error") {
                            state.config.remove_recent(&path);
                        }
                    }
                }
            }
            if let Some(path) = project_to_remove {
                state.config.remove_recent(&path);
            }
        }
    });
}
