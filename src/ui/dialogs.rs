use bevy_egui::egui;

use crate::file::{pick_file, pick_image_file, pick_save_file};
use crate::imaging::import_image_as_base64;
use crate::model::{Animation, Character, Part, RotationMode, State};
use crate::state::{ActiveTab, ContextMenuTarget, PendingAction};
use crate::state::AppState;
use crate::ui::widgets::format_relative_time;

pub fn render_dialogs(ctx: &egui::Context, state: &mut AppState) {
    // Rename dialog
    if state.show_rename_dialog {
        let title = match &state.context_menu_target {
            Some(ContextMenuTarget::Character { .. }) => "Rename Character",
            Some(ContextMenuTarget::Part { .. }) => "Rename Part",
            Some(ContextMenuTarget::Animation { .. }) => "Rename Animation",
            Some(ContextMenuTarget::Frame { .. })
            | Some(ContextMenuTarget::Layer { .. })
            | None => "Rename",
        };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("New name:");
                    ui.text_edit_singleline(&mut state.rename_new_name);
                });
                ui.horizontal(|ui| {
                    if ui.button("Rename").clicked() && !state.rename_new_name.is_empty() {
                        let new_name = state.rename_new_name.clone();
                        if let Some(target) = state.context_menu_target.take() {
                            match target {
                                ContextMenuTarget::Character { char_name } => {
                                    if let Some(ref mut project) = state.project {
                                        if let Some(character) = project
                                            .characters
                                            .iter_mut()
                                            .find(|c| c.name == char_name)
                                        {
                                            let old_name = character.name.clone();
                                            character.name = new_name.clone();
                                            // Update active character if it was renamed
                                            if state.active_character.as_ref() == Some(&old_name) {
                                                state.active_character = Some(new_name.clone());
                                            }
                                            // Update active tab if it was the character editor for this character
                                            if let ActiveTab::CharacterEditor(ref tab_name) =
                                                state.active_tab
                                            {
                                                if tab_name == &old_name {
                                                    state.active_tab =
                                                        ActiveTab::CharacterEditor(new_name.clone());
                                                }
                                            }
                                            state.set_status(format!(
                                                "Renamed character to '{}'",
                                                new_name
                                            ));
                                        }
                                    }
                                }
                                ContextMenuTarget::Part {
                                    char_name,
                                    part_name,
                                } => {
                                    if let Some(ref mut project) = state.project {
                                        if let Some(character) = project.get_character_mut(&char_name)
                                        {
                                            if let Some(part) =
                                                character.parts.iter_mut().find(|p| p.name == part_name)
                                            {
                                                part.name = new_name.clone();
                                                state.editor_selected_part = Some(new_name.clone());
                                                state.set_status(format!(
                                                    "Renamed part to '{}'",
                                                    new_name
                                                ));
                                            }
                                        }
                                    }
                                }
                                ContextMenuTarget::Animation {
                                    char_name,
                                    anim_index,
                                    ..
                                } => {
                                    if let Some(ref mut project) = state.project {
                                        if let Some(character) = project.get_character_mut(&char_name)
                                        {
                                            if let Some(anim) =
                                                character.animations.get_mut(anim_index)
                                            {
                                                anim.name = new_name.clone();
                                                state.set_status(format!(
                                                    "Renamed animation to '{}'",
                                                    new_name
                                                ));
                                            }
                                        }
                                    }
                                }
                                ContextMenuTarget::Frame { .. } => {
                                    // Frames cannot be renamed
                                }
                                ContextMenuTarget::Layer { .. } => {
                                    // Layers cannot be renamed (name comes from part definition)
                                }
                            }
                        }
                        state.show_rename_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_rename_dialog = false;
                        state.context_menu_target = None;
                    }
                });
            });
    }

    // Delete confirmation dialog
    if state.show_delete_confirm_dialog {
        let (title, item_type, item_name) = match &state.context_menu_target {
            Some(ContextMenuTarget::Character { char_name }) => {
                ("Delete Character?", "character", char_name.clone())
            }
            Some(ContextMenuTarget::Part { part_name, .. }) => {
                ("Delete Part?", "part", part_name.clone())
            }
            Some(ContextMenuTarget::Animation { anim_name, .. }) => {
                ("Delete Animation?", "animation", anim_name.clone())
            }
            Some(ContextMenuTarget::Frame { frame_index, .. }) => {
                ("Delete Frame?", "frame", format!("{}", frame_index + 1))
            }
            Some(ContextMenuTarget::Layer { layer_name, .. }) => {
                ("Delete Layer?", "layer", layer_name.clone())
            }
            None => ("Delete?", "item", String::new()),
        };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Are you sure you want to delete {} ", item_type));
                    ui.label(egui::RichText::new(&item_name).strong());
                    ui.label("?");
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        if let Some(target) = state.context_menu_target.take() {
                            match target {
                                ContextMenuTarget::Character { char_name } => {
                                    if let Some(ref mut project) = state.project {
                                        project.characters.retain(|c| c.name != char_name);
                                        // Clear active character if it was deleted
                                        if state.active_character.as_ref() == Some(&char_name) {
                                            state.active_character = None;
                                            state.active_tab = ActiveTab::Canvas;
                                        }
                                        // Close character editor tab if it was for this character
                                        if let ActiveTab::CharacterEditor(ref tab_name) =
                                            state.active_tab
                                        {
                                            if tab_name == &char_name {
                                                state.active_tab = ActiveTab::Canvas;
                                            }
                                        }
                                        state.set_status(format!(
                                            "Deleted character '{}'",
                                            char_name
                                        ));
                                    }
                                }
                                ContextMenuTarget::Part {
                                    char_name,
                                    part_name,
                                } => {
                                    if let Some(ref mut project) = state.project {
                                        if let Some(character) = project.get_character_mut(&char_name)
                                        {
                                            character.parts.retain(|p| p.name != part_name);
                                            if state.editor_selected_part.as_ref() == Some(&part_name)
                                            {
                                                state.editor_selected_part = None;
                                            }
                                            state.set_status(format!("Deleted part '{}'", part_name));
                                        }
                                    }
                                }
                                ContextMenuTarget::Animation {
                                    char_name,
                                    anim_index,
                                    anim_name,
                                } => {
                                    if let Some(ref mut project) = state.project {
                                        if let Some(character) = project.get_character_mut(&char_name)
                                        {
                                            if anim_index < character.animations.len() {
                                                character.animations.remove(anim_index);
                                                // Adjust current animation index
                                                if state.current_animation
                                                    >= character.animations.len()
                                                    && !character.animations.is_empty()
                                                {
                                                    state.current_animation =
                                                        character.animations.len() - 1;
                                                }
                                                state.current_frame = 0;
                                                state.set_status(format!(
                                                    "Deleted animation '{}'",
                                                    anim_name
                                                ));
                                            }
                                        }
                                    }
                                }
                                ContextMenuTarget::Frame {
                                    char_name,
                                    anim_index,
                                    frame_index,
                                } => {
                                    if let Some(ref mut project) = state.project {
                                        if let Some(character) = project.get_character_mut(&char_name)
                                        {
                                            if let Some(anim) =
                                                character.animations.get_mut(anim_index)
                                            {
                                                if frame_index < anim.frames.len()
                                                    && anim.frames.len() > 1
                                                {
                                                    anim.frames.remove(frame_index);
                                                    // Adjust current frame index
                                                    if state.current_frame >= anim.frames.len() {
                                                        state.current_frame = anim.frames.len() - 1;
                                                    }
                                                    state.selected_part_id = None;
                                                    state.set_status(format!(
                                                        "Deleted frame {}",
                                                        frame_index + 1
                                                    ));
                                                } else if anim.frames.len() == 1 {
                                                    state.set_status("Cannot delete the only frame");
                                                }
                                            }
                                        }
                                    }
                                }
                                ContextMenuTarget::Layer {
                                    layer_id,
                                    layer_name,
                                } => {
                                    let current_anim_idx = state.current_animation;
                                    let current_frame_idx = state.current_frame;
                                    if let Some(ref char_name) = state.active_character.clone() {
                                        if let Some(ref mut project) = state.project {
                                            if let Some(character) =
                                                project.get_character_mut(char_name)
                                            {
                                                if let Some(anim) =
                                                    character.animations.get_mut(current_anim_idx)
                                                {
                                                    if let Some(frame) =
                                                        anim.frames.get_mut(current_frame_idx)
                                                    {
                                                        frame
                                                            .placed_parts
                                                            .retain(|p| p.id != layer_id);
                                                        if state.selected_part_id == Some(layer_id) {
                                                            state.selected_part_id = None;
                                                        }
                                                        state.set_status(format!(
                                                            "Deleted layer '{}'",
                                                            layer_name
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        state.show_delete_confirm_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_delete_confirm_dialog = false;
                        state.context_menu_target = None;
                    }
                });
            });
    }

    // Unsaved changes confirmation dialog
    if let Some(ref pending) = state.pending_action.clone() {
        let title = match pending {
            PendingAction::CloseProject => "Close Project?",
            PendingAction::NewProject => "Create New Project?",
            PendingAction::OpenProject => "Open Project?",
            PendingAction::Exit => "Exit Application?",
        };
        let action_text = match pending {
            PendingAction::CloseProject => "closing",
            PendingAction::NewProject => "creating a new project",
            PendingAction::OpenProject => "opening another project",
            PendingAction::Exit => "exiting",
        };
        let continue_btn = match pending {
            PendingAction::CloseProject => "Close Without Saving",
            PendingAction::NewProject => "Don't Save",
            PendingAction::OpenProject => "Don't Save",
            PendingAction::Exit => "Exit Without Saving",
        };

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("You have unsaved changes.");

                // Show time since last save
                if let Some(last_saved) = state.last_saved_time {
                    ui.label(format!(
                        "Last saved {}.",
                        format_relative_time(last_saved.elapsed())
                    ));
                } else {
                    ui.label("This project has never been saved.");
                }

                ui.add_space(10.0);
                ui.label(format!("Do you want to save before {}?", action_text));
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        // Try to save first
                        let save_ok = if state.project_path.is_some() {
                            match state.save_project() {
                                Ok(()) => {
                                    state.set_status("Project saved");
                                    true
                                }
                                Err(e) => {
                                    state.set_status(format!("Save failed: {}", e));
                                    false
                                }
                            }
                        } else {
                            // Need to Save As first
                            if let Some(path) = pick_save_file() {
                                let path_str = path.to_string_lossy().to_string();
                                match state.save_project_as(&path_str) {
                                    Ok(()) => {
                                        state.set_status("Project saved");
                                        true
                                    }
                                    Err(e) => {
                                        state.set_status(format!("Save failed: {}", e));
                                        false
                                    }
                                }
                            } else {
                                false
                            }
                        };

                        if save_ok {
                            // Perform the pending action
                            match pending {
                                PendingAction::CloseProject => {
                                    state.close_project();
                                    state.set_status("Project saved and closed");
                                }
                                PendingAction::NewProject => {
                                    state.new_project();
                                    state.set_status("Created new project");
                                }
                                PendingAction::OpenProject => {
                                    if let Some(path) = pick_file() {
                                        let path_str = path.to_string_lossy().to_string();
                                        match state.load_project(&path_str) {
                                            Ok(()) => {
                                                state.set_status(format!("Loaded {}", path_str))
                                            }
                                            Err(e) => {
                                                state.set_status(format!("Load failed: {}", e))
                                            }
                                        }
                                    }
                                }
                                PendingAction::Exit => {
                                    std::process::exit(0);
                                }
                            }
                            state.pending_action = None;
                        }
                    }

                    if ui.button(continue_btn).clicked() {
                        // Perform action without saving
                        match pending {
                            PendingAction::CloseProject => {
                                state.close_project();
                                state.set_status("Project closed without saving");
                            }
                            PendingAction::NewProject => {
                                state.new_project();
                                state.set_status("Created new project");
                            }
                            PendingAction::OpenProject => {
                                if let Some(path) = pick_file() {
                                    let path_str = path.to_string_lossy().to_string();
                                    match state.load_project(&path_str) {
                                        Ok(()) => state.set_status(format!("Loaded {}", path_str)),
                                        Err(e) => state.set_status(format!("Load failed: {}", e)),
                                    }
                                }
                            }
                            PendingAction::Exit => {
                                std::process::exit(0);
                            }
                        }
                        state.pending_action = None;
                    }

                    if ui.button("Cancel").clicked() {
                        state.pending_action = None;
                    }
                });
            });
    }

    // New Animation dialog
    if state.show_new_animation_dialog {
        egui::Window::new("New Animation")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut state.new_animation_name);
                });
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() && !state.new_animation_name.is_empty() {
                        let active_char = state.active_character.clone();
                        if let Some(ref mut project) = state.project {
                            if let Some(ref char_name) = active_char {
                                if let Some(character) = project.get_character_mut(char_name) {
                                    let animation = Animation::new(&state.new_animation_name);
                                    character.add_animation(animation);
                                    state.current_animation = character.animations.len() - 1;
                                    state.current_frame = 0;
                                }
                            }
                        }
                        state.show_new_animation_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_new_animation_dialog = false;
                    }
                });
            });
    }

    // Import Image dialog
    if state.show_import_image_dialog {
        egui::Window::new("Import Rotation Image")
            .collapsible(false)
            .resizable(false)
            .min_width(400.0)
            .show(ctx, |ui| {
                // Show what we're importing to
                if let (Some(ref char_name), Some(ref part_name), Some(ref state_name)) = (
                    &state.selected_character_for_part,
                    &state.selected_part_for_state,
                    &state.selected_state_for_import,
                ) {
                    ui.label(format!(
                        "Importing to: {} / {} / {} @ {}°",
                        char_name, part_name, state_name, state.selected_rotation_for_import
                    ));
                }
                ui.separator();

                ui.label("Enter path to PNG image:");
                ui.text_edit_singleline(&mut state.import_image_path);

                ui.horizontal(|ui| {
                    if ui.button("Import").clicked() && !state.import_image_path.is_empty() {
                        let path = state.import_image_path.clone();
                        let rotation_angle = state.selected_rotation_for_import;
                        match import_image_as_base64(&path) {
                            Ok(base64_data) => {
                                if let (
                                    Some(ref char_name),
                                    Some(ref part_name),
                                    Some(ref state_name),
                                ) = (
                                    &state.selected_character_for_part,
                                    &state.selected_part_for_state,
                                    &state.selected_state_for_import,
                                ) {
                                    if let Some(ref mut project) = state.project {
                                        if let Some(character) =
                                            project.get_character_mut(char_name)
                                        {
                                            if let Some(part) = character.get_part_mut(part_name) {
                                                if let Some(state_obj) = part
                                                    .states
                                                    .iter_mut()
                                                    .find(|s| s.name == *state_name)
                                                {
                                                    if let Some(rotation) =
                                                        state_obj.rotations.get_mut(&rotation_angle)
                                                    {
                                                        rotation.image_data = Some(base64_data);
                                                        state.set_status(format!(
                                                            "Image imported for {}° rotation",
                                                            rotation_angle
                                                        ));
                                                        // Clear texture cache so it gets reloaded
                                                        state.texture_cache.clear();
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    state.set_status("Missing selection context");
                                }
                            }
                            Err(e) => {
                                state.set_status(format!("Import failed: {}", e));
                            }
                        }
                        state.show_import_image_dialog = false;
                        state.selected_character_for_part = None;
                        state.selected_part_for_state = None;
                        state.selected_state_for_import = None;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_import_image_dialog = false;
                        state.selected_character_for_part = None;
                        state.selected_part_for_state = None;
                        state.selected_state_for_import = None;
                    }
                });
            });
    }

    // New Character dialog
    if state.show_new_character_dialog {
        egui::Window::new("New Character")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut state.new_character_name);
                });
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() && !state.new_character_name.is_empty() {
                        if let Some(ref mut project) = state.project {
                            let char_id = project.next_char_id();
                            let character = Character::new(char_id, &state.new_character_name);
                            project.add_character(character);
                            state.active_character = Some(state.new_character_name.clone());
                            state.current_animation = 0;
                            state.current_frame = 0;
                            state.needs_zoom_fit = true;
                            // Open the character editor for the new character
                            state.active_tab =
                                ActiveTab::CharacterEditor(state.new_character_name.clone());
                            state
                                .set_status(format!("Created character: {}", state.new_character_name));
                        }
                        state.show_new_character_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_new_character_dialog = false;
                    }
                });
            });
    }

    // Clone Character dialog
    if state.show_clone_character_dialog {
        egui::Window::new("Clone Character")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                if let Some(ref source_name) = state.clone_source_character.clone() {
                    ui.label(format!("Cloning: {}", source_name));
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("New name:");
                        let text_edit =
                            egui::TextEdit::singleline(&mut state.clone_character_name)
                                .desired_width(200.0);
                        let response = ui.add(text_edit);

                        // Request focus and select all text on first frame
                        if response.gained_focus() || !response.has_focus() {
                            response.request_focus();
                            if let Some(mut text_state) =
                                egui::TextEdit::load_state(ui.ctx(), response.id)
                            {
                                text_state.cursor.set_char_range(Some(
                                    egui::text::CCursorRange::two(
                                        egui::text::CCursor::new(0),
                                        egui::text::CCursor::new(state.clone_character_name.len()),
                                    ),
                                ));
                                text_state.store(ui.ctx(), response.id);
                            }
                        }
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let name_empty = state.clone_character_name.is_empty();
                        let name_exists = state
                            .project
                            .as_ref()
                            .map(|p| p.get_character(&state.clone_character_name).is_some())
                            .unwrap_or(false);
                        let name_valid = !name_empty && !name_exists;

                        if ui
                            .add_enabled(name_valid, egui::Button::new("Clone"))
                            .clicked()
                        {
                            if let Some(ref mut project) = state.project {
                                if let Some(original) = project.get_character(&source_name) {
                                    let mut cloned = original.clone();
                                    cloned.id = project.next_char_id(); // Assign new unique ID
                                    cloned.name = state.clone_character_name.clone();
                                    project.add_character(cloned);
                                    state.active_character = Some(state.clone_character_name.clone());
                                    state.current_animation = 0;
                                    state.current_frame = 0;
                                    state.needs_zoom_fit = true;
                                    state.set_status(format!(
                                        "Cloned '{}' as '{}'",
                                        source_name, state.clone_character_name
                                    ));
                                }
                            }
                            state.show_clone_character_dialog = false;
                            state.clone_source_character = None;
                        }
                        if ui.button("Cancel").clicked() {
                            state.show_clone_character_dialog = false;
                            state.clone_source_character = None;
                        }
                    });
                    if state
                        .project
                        .as_ref()
                        .map(|p| p.get_character(&state.clone_character_name).is_some())
                        .unwrap_or(false)
                    {
                        ui.colored_label(
                            egui::Color32::from_rgb(255, 150, 150),
                            "Name already in use",
                        );
                    }
                }
            });
    }

    // New Part dialog (for character editor)
    if state.show_new_part_dialog {
        egui::Window::new("New Part")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                if let ActiveTab::CharacterEditor(ref char_name) = state.active_tab.clone() {
                    ui.label(format!("Adding part to: {}", char_name));
                    ui.separator();
                }
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut state.new_part_name);
                });
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() && !state.new_part_name.is_empty() {
                        if let ActiveTab::CharacterEditor(ref char_name) = state.active_tab.clone() {
                            if let Some(ref mut project) = state.project {
                                if let Some(character) = project.get_character_mut(&char_name) {
                                    let part = Part::new(&state.new_part_name);
                                    character.add_part(part);
                                    state.editor_selected_part = Some(state.new_part_name.clone());
                                    state.editor_selected_state = None;
                                    state.set_status(format!("Created part: {}", state.new_part_name));
                                }
                            }
                        }
                        state.show_new_part_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_new_part_dialog = false;
                    }
                });
            });
    }

    // New State dialog (for character editor)
    if state.show_new_state_dialog {
        egui::Window::new("New State")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                if let (ActiveTab::CharacterEditor(ref char_name), Some(ref part_name)) =
                    (state.active_tab.clone(), state.editor_selected_part.clone())
                {
                    ui.label(format!("Adding state to: {} / {}", char_name, part_name));
                    ui.separator();
                }
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut state.new_state_name);
                });
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() && !state.new_state_name.is_empty() {
                        if let ActiveTab::CharacterEditor(ref char_name) = state.active_tab.clone() {
                            if let Some(ref part_name) = state.editor_selected_part.clone() {
                                if let Some(ref mut project) = state.project {
                                    if let Some(character) = project.get_character_mut(&char_name) {
                                        if let Some(part) = character.get_part_mut(&part_name) {
                                            let new_state =
                                                State::new(&state.new_state_name, RotationMode::Deg45);
                                            part.add_state(new_state);
                                            state.editor_selected_state =
                                                Some(state.new_state_name.clone());
                                            state.set_status(format!(
                                                "Created state: {}",
                                                state.new_state_name
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                        state.show_new_state_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_new_state_dialog = false;
                    }
                });
            });
    }

    // Handle pending rotation import with file picker
    if let Some(angle) = state.pending_rotation_import {
        if let Some(path) = pick_image_file() {
            if let ActiveTab::CharacterEditor(ref char_name) = state.active_tab.clone() {
                let part_name = state.editor_selected_part.clone();
                let state_name = state
                    .editor_selected_state
                    .clone()
                    .or_else(|| Some("default".to_string()));

                match import_image_as_base64(path.to_str().unwrap_or("")) {
                    Ok(base64_data) => {
                        if let (Some(ref pn), Some(ref sn)) = (part_name, state_name) {
                            if let Some(ref mut project) = state.project {
                                if let Some(character) = project.get_character_mut(&char_name) {
                                    if let Some(part) = character.get_part_mut(pn) {
                                        if let Some(state_obj) =
                                            part.states.iter_mut().find(|s| &s.name == sn)
                                        {
                                            if let Some(rotation) =
                                                state_obj.rotations.get_mut(&angle)
                                            {
                                                rotation.image_data = Some(base64_data);
                                                state.set_status(format!(
                                                    "Imported image for {}°",
                                                    angle
                                                ));
                                                state.texture_cache.clear();
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        state.set_status(format!("Import failed: {}", e));
                    }
                }
            }
        }
        state.pending_rotation_import = None;
    }
}
