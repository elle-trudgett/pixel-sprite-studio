use bevy_egui::egui;

use crate::state::{ActiveTab, ContextMenuTarget};
use crate::state::AppState;
use crate::ui::rotation_wheel::render_rotation_wheel;

pub fn render_character_editor(ui: &mut egui::Ui, state: &mut AppState, char_name: &str) {
    // Don't process interactions if a dialog is open
    if state.show_new_part_dialog || state.show_new_state_dialog {
        ui.set_enabled(false);
    }

    // Get character data
    let (parts, selected_part_states, selected_state_rotations) = {
        let Some(ref project) = state.project else {
            ui.label("No project loaded");
            return;
        };
        let Some(character) = project.get_character(char_name) else {
            ui.label(format!("Character '{}' not found", char_name));
            return;
        };

        let parts: Vec<String> = character.parts.iter().map(|p| p.name.clone()).collect();

        let selected_part_states: Vec<(String, bool)> = state
            .editor_selected_part
            .as_ref()
            .and_then(|pn| character.get_part(pn))
            .map(|p| {
                p.states
                    .iter()
                    .map(|s| {
                        let has_images = s.rotations.values().any(|r| r.image_data.is_some());
                        (s.name.clone(), has_images)
                    })
                    .collect()
            })
            .unwrap_or_default();

        let selected_state_rotations: Vec<(u16, bool)> = state
            .editor_selected_part
            .as_ref()
            .and_then(|pn| character.get_part(pn))
            .and_then(|p| {
                let state_name = state
                    .editor_selected_state
                    .as_ref()
                    .or(p.states.first().map(|s| &s.name))?;
                p.states.iter().find(|s| &s.name == state_name)
            })
            .map(|s| {
                s.rotations
                    .iter()
                    .map(|(angle, r)| (*angle, r.image_data.is_some()))
                    .collect()
            })
            .unwrap_or_default();

        (parts, selected_part_states, selected_state_rotations)
    };

    // Character settings
    ui.heading("Character Settings");

    // Name field for renaming
    let mut new_name = char_name.to_string();
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut new_name);
    });

    // Handle rename if name changed
    if new_name != char_name && !new_name.is_empty() {
        if let Some(ref mut project) = state.project {
            // Check if new name doesn't conflict with existing character
            let name_exists = project.characters.iter().any(|c| c.name == new_name);
            if !name_exists {
                if let Some(character) = project.get_character_mut(char_name) {
                    character.name = new_name.clone();
                }
                // Update active character reference
                state.active_character = Some(new_name.clone());
                // Update active tab if it references this character
                if let ActiveTab::CharacterEditor(_) = state.active_tab {
                    state.active_tab = ActiveTab::CharacterEditor(new_name);
                }
            }
        }
    }

    // Frame size
    ui.horizontal(|ui| {
        ui.label("Frame size:");
        if let Some(ref mut project) = state.project {
            if let Some(character) = project.get_character_mut(char_name) {
                let mut w = character.canvas_size.0 as i32;
                let mut h = character.canvas_size.1 as i32;
                ui.add(egui::DragValue::new(&mut w).speed(1).range(8..=512));
                ui.label("x");
                ui.add(egui::DragValue::new(&mut h).speed(1).range(8..=512));
                character.canvas_size = (w.max(8) as u32, h.max(8) as u32);
            }
        }
    });

    ui.separator();

    // Three-column layout: 20% / 20% / 60%
    let available_width = ui.available_width();
    let available_height = ui.available_height();

    ui.horizontal(|ui| {
        // Parts column (20%)
        ui.allocate_ui_with_layout(
            egui::vec2(available_width * 0.2, available_height),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                ui.heading("Parts");
                ui.separator();

                if parts.is_empty() {
                    ui.label("(No parts)");
                }
                for part_name in &parts {
                    let is_selected = state.editor_selected_part.as_ref() == Some(part_name);
                    let response = ui.selectable_label(is_selected, part_name);
                    if response.clicked() {
                        state.editor_selected_part = Some(part_name.clone());
                        state.editor_selected_state = None;
                    }
                    // Right-click context menu
                    response.context_menu(|ui| {
                        if ui.button("Rename...").clicked() {
                            state.context_menu_target = Some(ContextMenuTarget::Part {
                                char_name: char_name.to_string(),
                                part_name: part_name.clone(),
                            });
                            state.rename_new_name = part_name.clone();
                            state.show_rename_dialog = true;
                            state.dialog_needs_focus = true;
                            ui.close_menu();
                        }
                        if ui.button("Delete").clicked() {
                            state.context_menu_target = Some(ContextMenuTarget::Part {
                                char_name: char_name.to_string(),
                                part_name: part_name.clone(),
                            });
                            state.show_delete_confirm_dialog = true;
                            ui.close_menu();
                        }
                    });
                }

                ui.separator();
                if ui.button("+ Add Part").clicked() {
                    state.show_new_part_dialog = true;
                    state.new_part_name.clear();
                    state.dialog_needs_focus = true;
                }
            },
        );

        ui.separator();

        // States column (20%)
        ui.allocate_ui_with_layout(
            egui::vec2(available_width * 0.2, available_height),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                ui.heading("States");
                ui.separator();

                if state.editor_selected_part.is_some() {
                    if selected_part_states.is_empty() {
                        ui.label("(No states)");
                    }
                    for (state_name, has_images) in &selected_part_states {
                        let is_selected = state.editor_selected_state.as_ref() == Some(state_name)
                            || (state.editor_selected_state.is_none()
                                && selected_part_states.first().map(|(n, _)| n) == Some(state_name));

                        let label = if *has_images {
                            egui::RichText::new(state_name).strong()
                        } else {
                            egui::RichText::new(state_name)
                        };

                        if ui.selectable_label(is_selected, label).clicked() {
                            state.editor_selected_state = Some(state_name.clone());
                        }
                    }

                    ui.separator();
                    if ui.button("+ Add State").clicked() {
                        state.show_new_state_dialog = true;
                        state.new_state_name.clear();
                        state.dialog_needs_focus = true;
                    }
                } else {
                    ui.label("Select a part");
                }
            },
        );

        ui.separator();

        // Rotation wheel column (60%)
        ui.allocate_ui_with_layout(
            egui::vec2(available_width * 0.58, available_height),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.heading("Rotations");
                ui.separator();

                if state.editor_selected_part.is_some() {
                    render_rotation_wheel(ui, state, char_name, &selected_state_rotations);
                } else {
                    ui.label("Select a part and state");
                }
            },
        );
    });
}
