use bevy::prelude::*;
use bevy_egui::egui;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::model::{Animation, Character, EditorState, PlacedPart, Project};
use super::config::AppConfig;
use super::types::{ActiveTab, ContextMenuTarget, DraggedPart, GalleryDrag, PendingAction, ZOOM_LEVELS};

#[derive(Resource, Default)]
pub struct AppState {
    pub project: Option<Project>,
    pub project_path: Option<PathBuf>,
    pub config: AppConfig,
    pub last_saved_json: Option<String>, // JSON of last saved state for dirty checking
    pub last_saved_time: Option<std::time::Instant>, // When we last saved

    // Pending action for unsaved changes dialog
    pub pending_action: Option<PendingAction>,

    // UI state
    pub show_grid: bool,
    pub show_labels: bool,
    pub show_overlay_info: bool,
    pub zoom_level: f32,
    pub current_animation: usize,
    pub current_frame: usize,
    pub is_playing: bool,
    pub playback_time: f32, // Accumulated time in current frame (seconds)
    pub selected_part_id: Option<u64>,
    pub selection_time: Option<std::time::Instant>, // When part was selected (for flash effect)
    pub last_clicked_part_id: Option<u64>, // Track part clicked for double-click validation
    pub pixel_aligned: bool,
    pub canvas_offset: (f32, f32), // Pan offset for canvas
    pub is_panning: bool, // True when space or middle mouse is held
    pub pan_started_in_canvas: bool, // True if panning was initiated with mouse inside canvas
    pub needs_zoom_fit: bool, // True when zoom should auto-fit to canvas size

    // Per-character animation state
    pub active_character: Option<String>, // Currently selected character
    pub active_tab: ActiveTab, // Canvas or CharacterEditor

    // Character editor state
    pub editor_selected_part: Option<String>,
    pub editor_selected_state: Option<String>,

    // Dragging state (for canvas parts)
    pub dragging_part: Option<DraggedPart>,
    pub drag_offset: (f32, f32),
    pub drag_accumulator: (f32, f32), // Accumulates true position during pixel-aligned drag

    // Drag from gallery state
    pub gallery_drag: Option<GalleryDrag>,

    // Menu state
    pub reopen_view_menu: bool,

    // Dialogs
    pub show_new_character_dialog: bool,
    pub show_new_part_dialog: bool,
    pub show_new_state_dialog: bool,
    pub show_new_animation_dialog: bool,
    pub show_import_image_dialog: bool,
    pub show_import_rotation_dialog: bool,
    pub show_rename_dialog: bool,
    pub show_delete_confirm_dialog: bool,
    pub show_clone_character_dialog: bool,

    // Rename/delete context
    pub context_menu_target: Option<ContextMenuTarget>,
    pub rename_new_name: String,

    // Clone character state
    pub clone_source_character: Option<String>,
    pub clone_character_name: String,

    // Rotation import state (from character editor)
    pub pending_rotation_import: Option<u16>,

    // Dialog input buffers
    pub new_character_name: String,
    pub new_part_name: String,
    pub new_state_name: String,
    pub new_animation_name: String,
    pub selected_character_for_part: Option<String>,
    pub selected_part_for_state: Option<String>,
    pub selected_state_for_import: Option<String>,
    pub selected_rotation_for_import: u16,
    pub import_image_path: String,

    // Status message
    pub status_message: Option<(String, std::time::Instant)>, // (message, when set)

    // Loaded textures cache (texture_id -> egui::TextureHandle)
    pub texture_cache: HashMap<String, egui::TextureHandle>,

    // Reference image state
    pub dragging_reference: bool,
    pub reference_drag_start: (f32, f32),
    pub reference_initial_pos: (f32, f32),
    pub reference_texture_cache: HashMap<String, (egui::TextureHandle, (u32, u32))>, // path -> (texture, original_size)
    pub reference_using_fallback: HashMap<String, bool>, // path -> whether using thumbnail fallback

    // Reference view settings (global, not per-frame)
    pub reference_opacity: f32,
    pub reference_show_on_top: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            project: None,
            project_path: None,
            config: AppConfig::load(),
            last_saved_json: None,
            last_saved_time: None,
            pending_action: None,
            show_grid: true,
            show_labels: true,
            show_overlay_info: true,
            zoom_level: 16.0,
            current_animation: 0,
            current_frame: 0,
            is_playing: false,
            playback_time: 0.0,
            selected_part_id: None,
            selection_time: None,
            last_clicked_part_id: None,
            pixel_aligned: true,
            canvas_offset: (0.0, 0.0),
            is_panning: false,
            pan_started_in_canvas: false,
            needs_zoom_fit: true,
            active_character: None,
            active_tab: ActiveTab::Canvas,
            editor_selected_part: None,
            editor_selected_state: None,
            dragging_part: None,
            drag_offset: (0.0, 0.0),
            drag_accumulator: (0.0, 0.0),
            gallery_drag: None,
            reopen_view_menu: false,
            show_new_character_dialog: false,
            show_new_part_dialog: false,
            show_new_state_dialog: false,
            show_new_animation_dialog: false,
            show_import_image_dialog: false,
            show_import_rotation_dialog: false,
            show_rename_dialog: false,
            show_delete_confirm_dialog: false,
            show_clone_character_dialog: false,
            context_menu_target: None,
            rename_new_name: String::new(),
            clone_source_character: None,
            clone_character_name: String::new(),
            pending_rotation_import: None,
            new_character_name: String::new(),
            new_part_name: String::new(),
            new_state_name: String::new(),
            new_animation_name: String::new(),
            selected_character_for_part: None,
            selected_part_for_state: None,
            selected_state_for_import: None,
            selected_rotation_for_import: 0,
            import_image_path: String::new(),
            status_message: None,
            texture_cache: HashMap::new(),
            dragging_reference: false,
            reference_drag_start: (0.0, 0.0),
            reference_initial_pos: (0.0, 0.0),
            reference_texture_cache: HashMap::new(),
            reference_using_fallback: HashMap::new(),
            reference_opacity: 0.5,
            reference_show_on_top: false,
        }
    }

    pub fn place_part_on_canvas(&mut self, character_id: u64, part: &str, state: &str, x: f32, y: f32) {
        let current_anim = self.current_animation;
        let current_frame = self.current_frame;
        let char_name = self.active_character.clone();

        if let Some(ref mut project) = self.project {
            let id = project.next_id();

            // Generate unique layer name
            let layer_name = if let Some(ref char_name) = char_name {
                if let Some(character_obj) = project.get_character(char_name) {
                    if let Some(anim) = character_obj.animations.get(current_anim) {
                        if let Some(frame) = anim.frames.get(current_frame) {
                            // Check existing layer names (fall back to part_name for old projects)
                            let base_name = part;
                            let existing_names: std::collections::HashSet<String> = frame
                                .placed_parts
                                .iter()
                                .map(|p| {
                                    if p.layer_name.is_empty() {
                                        p.part_name.clone()
                                    } else {
                                        p.layer_name.clone()
                                    }
                                })
                                .collect();

                            if !existing_names.contains(base_name) {
                                base_name.to_string()
                            } else {
                                // Find next available number
                                let mut n = 2;
                                loop {
                                    let candidate = format!("{} {}", base_name, n);
                                    if !existing_names.contains(&candidate) {
                                        break candidate;
                                    }
                                    n += 1;
                                }
                            }
                        } else {
                            part.to_string()
                        }
                    } else {
                        part.to_string()
                    }
                } else {
                    part.to_string()
                }
            } else {
                part.to_string()
            };

            let mut placed = PlacedPart::new(id, character_id, part, state)
                .with_layer_name(&layer_name);
            placed.position = (x, y);

            if let Some(ref char_name) = char_name {
                if let Some(character_obj) = project.get_character_mut(char_name) {
                    if let Some(anim) = character_obj.animations.get_mut(current_anim) {
                        if let Some(frame) = anim.frames.get_mut(current_frame) {
                            frame.placed_parts.push(placed);
                            self.selected_part_id = Some(id);
                            self.selection_time = Some(std::time::Instant::now());
                        }
                    }
                }
            }
        }
    }

    pub fn get_selected_placed_part(&self) -> Option<&PlacedPart> {
        let id = self.selected_part_id?;
        let anim = self.current_animation()?;
        let frame = anim.frames.get(self.current_frame)?;
        frame.placed_parts.iter().find(|p| p.id == id)
    }

    pub fn get_selected_placed_part_mut(&mut self) -> Option<&mut PlacedPart> {
        let id = self.selected_part_id?;
        let frame_idx = self.current_frame;
        let anim = self.current_animation_mut()?;
        let frame = anim.frames.get_mut(frame_idx)?;
        frame.placed_parts.iter_mut().find(|p| p.id == id)
    }

    pub fn delete_selected_part(&mut self) {
        if let Some(id) = self.selected_part_id {
            let frame_idx = self.current_frame;
            if let Some(anim) = self.current_animation_mut() {
                if let Some(frame) = anim.frames.get_mut(frame_idx) {
                    frame.placed_parts.retain(|p| p.id != id);
                }
            }
            self.selected_part_id = None;
        }
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), std::time::Instant::now()));
    }

    pub fn save_project(&mut self) -> Result<(), String> {
        let project = self.project.as_mut().ok_or("No project to save")?;
        let path = self.project_path.as_ref().ok_or("No file path set")?;

        // Save editor state before serializing
        project.editor_state = EditorState {
            active_character: self.active_character.clone(),
            current_animation: self.current_animation,
            current_frame: self.current_frame,
            active_tab: match &self.active_tab {
                ActiveTab::Canvas => "canvas".to_string(),
                ActiveTab::CharacterEditor(_) => "editor".to_string(),
            },
            zoom_level: self.zoom_level,
            show_grid: self.show_grid,
            show_labels: self.show_labels,
            reference_opacity: self.reference_opacity,
            reference_show_on_top: self.reference_show_on_top,
        };

        let json = project.to_json().map_err(|e| format!("Serialize error: {}", e))?;
        fs::write(path, &json).map_err(|e| format!("Write error: {}", e))?;

        // Track saved state for dirty checking
        self.last_saved_json = Some(json);
        self.last_saved_time = Some(std::time::Instant::now());

        Ok(())
    }

    pub fn save_project_as(&mut self, path: &str) -> Result<(), String> {
        self.project_path = Some(PathBuf::from(path));
        let result = self.save_project();
        if result.is_ok() {
            self.config.add_recent(path);
        }
        result
    }

    pub fn load_project(&mut self, path: &str) -> Result<(), String> {
        let json = fs::read_to_string(path).map_err(|e| format!("Read error: {}", e))?;
        let project = Project::from_json(&json).map_err(|e| format!("Parse error: {}", e))?;

        // Restore editor state from project
        let editor_state = &project.editor_state;
        self.active_character = editor_state.active_character.clone();
        self.current_animation = editor_state.current_animation;
        self.current_frame = editor_state.current_frame;
        self.zoom_level = editor_state.zoom_level;
        self.show_grid = editor_state.show_grid;
        self.show_labels = editor_state.show_labels;
        self.reference_opacity = editor_state.reference_opacity;
        self.reference_show_on_top = editor_state.reference_show_on_top;

        // Restore active tab
        self.active_tab = if editor_state.active_tab == "editor" {
            if let Some(ref char_name) = self.active_character {
                ActiveTab::CharacterEditor(char_name.clone())
            } else {
                ActiveTab::Canvas
            }
        } else {
            ActiveTab::Canvas
        };

        // Track saved state
        self.last_saved_json = project.to_json().ok();
        self.last_saved_time = Some(std::time::Instant::now());

        self.project = Some(project);
        self.project_path = Some(PathBuf::from(path));
        self.selected_part_id = None;
        self.needs_zoom_fit = true;
        self.config.add_recent(path);

        Ok(())
    }

    pub fn new_project(&mut self) {
        let project = Project::new("Untitled");
        self.last_saved_json = project.to_json().ok();
        self.last_saved_time = None; // New project hasn't been saved yet
        self.project = Some(project);
        self.project_path = None;
        self.current_animation = 0;
        self.current_frame = 0;
        self.selected_part_id = None;
        self.active_character = None;
        self.needs_zoom_fit = true;
    }

    pub fn close_project(&mut self) {
        self.project = None;
        self.project_path = None;
        self.last_saved_json = None;
        self.last_saved_time = None;
        self.current_animation = 0;
        self.current_frame = 0;
        self.selected_part_id = None;
        self.active_character = None;
        self.active_tab = ActiveTab::Canvas;
        self.texture_cache.clear();
    }

    pub fn has_unsaved_changes(&self) -> bool {
        match (&self.project, &self.last_saved_json) {
            (Some(project), Some(saved_json)) => {
                project.to_json().ok().as_ref() != Some(saved_json)
            }
            (Some(_), None) => true, // Project exists but never saved
            _ => false,
        }
    }

    pub fn time_since_save(&self) -> Option<std::time::Duration> {
        self.last_saved_time.map(|t| t.elapsed())
    }

    pub fn active_character_ref(&self) -> Option<&Character> {
        let char_name = self.active_character.as_ref()?;
        self.project.as_ref()?.get_character(char_name)
    }

    pub fn active_character_mut(&mut self) -> Option<&mut Character> {
        let char_name = self.active_character.clone()?;
        self.project.as_mut()?.get_character_mut(&char_name)
    }

    pub fn current_animation(&self) -> Option<&Animation> {
        self.active_character_ref()?.animations.get(self.current_animation)
    }

    pub fn current_animation_mut(&mut self) -> Option<&mut Animation> {
        let anim_idx = self.current_animation;
        self.active_character_mut()?.animations.get_mut(anim_idx)
    }

    pub fn total_frames(&self) -> usize {
        self.current_animation().map(|a| a.frames.len()).unwrap_or(1)
    }

    pub fn zoom_in(&mut self) {
        // Find next higher zoom level
        for &level in &ZOOM_LEVELS {
            if level > self.zoom_level {
                self.zoom_level = level;
                return;
            }
        }
    }

    pub fn zoom_out(&mut self) {
        // Find next lower zoom level
        for &level in ZOOM_LEVELS.iter().rev() {
            if level < self.zoom_level {
                self.zoom_level = level;
                return;
            }
        }
    }
}
