mod model;

use bevy::prelude::*;
use bevy::window::WindowCloseRequested;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use model::{Animation, Character, Part, Project, State, RotationMode, PlacedPart};
use std::path::PathBuf;
use std::fs;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

const MAX_RECENT_PROJECTS: usize = 10;

#[cfg(target_os = "windows")]
use rfd::FileDialog;

const ZOOM_LEVELS: [f32; 10] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0];

/// App configuration stored on disk
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppConfig {
    recent_projects: Vec<String>,
    #[serde(default = "default_ui_scale")]
    ui_scale: f32,
}

fn default_ui_scale() -> f32 {
    1.0
}

/// Get a scaled font size with minimum of 12
fn scaled_font(base_size: f32, scale: f32) -> f32 {
    (base_size.max(12.0) * scale).max(12.0)
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            recent_projects: Vec::new(),
            ui_scale: 1.0,
        }
    }
}

impl AppConfig {
    fn config_path() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            std::env::var("APPDATA").ok().map(|appdata| {
                PathBuf::from(appdata).join("SpriteAnimator").join("config.json")
            })
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::env::var("HOME").ok().map(|home| {
                PathBuf::from(home).join(".config").join("sprite-animator").join("config.json")
            })
        }
    }

    fn load() -> Self {
        Self::config_path()
            .and_then(|path| fs::read_to_string(&path).ok())
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        if let Some(path) = Self::config_path() {
            // Create parent directories if needed
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = fs::write(&path, json);
            }
        }
    }

    fn add_recent(&mut self, path: &str) {
        // Remove if already exists (to move to front)
        self.recent_projects.retain(|p| p != path);
        // Add to front
        self.recent_projects.insert(0, path.to_string());
        // Trim to max size
        self.recent_projects.truncate(MAX_RECENT_PROJECTS);
        self.save();
    }

    fn remove_recent(&mut self, path: &str) {
        self.recent_projects.retain(|p| p != path);
        self.save();
    }
}

/// Active tab in the central panel
#[derive(Debug, Clone, PartialEq, Default)]
enum ActiveTab {
    #[default]
    Canvas,
    CharacterEditor(String), // Character name being edited
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sprite Animator".into(),
                resolution: (1920., 1080.).into(),
                resizable: true,
                ..default()
            }),
            close_when_requested: false, // Handle close ourselves for unsaved changes check
            ..default()
        }))
        .add_plugins(EguiPlugin)
        .init_resource::<AppState>()
        .add_systems(Startup, (setup, configure_fonts))
        .add_systems(Update, (ui_system, handle_window_close))
        .run();
}

/// Action pending confirmation due to unsaved changes
#[derive(Clone, Debug)]
enum PendingAction {
    CloseProject,
    NewProject,
    OpenProject,
    Exit,
}

#[derive(Resource, Default)]
struct AppState {
    project: Option<Project>,
    project_path: Option<PathBuf>,
    config: AppConfig,
    last_saved_json: Option<String>, // JSON of last saved state for dirty checking
    last_saved_time: Option<std::time::Instant>, // When we last saved

    // Pending action for unsaved changes dialog
    pending_action: Option<PendingAction>,

    // UI state
    show_grid: bool,
    show_labels: bool,
    show_overlay_info: bool,
    zoom_level: f32,
    current_animation: usize,
    current_frame: usize,
    is_playing: bool,
    playback_time: f32, // Accumulated time in current frame (seconds)
    selected_part_id: Option<u64>,
    pixel_aligned: bool,
    canvas_offset: (f32, f32), // Pan offset for canvas
    is_panning: bool, // True when space or middle mouse is held

    // Per-character animation state
    active_character: Option<String>, // Currently selected character
    active_tab: ActiveTab, // Canvas or CharacterEditor

    // Character editor state
    editor_selected_part: Option<String>,
    editor_selected_state: Option<String>,

    // Dragging state (for canvas parts)
    dragging_part: Option<DraggedPart>,
    drag_offset: (f32, f32),
    drag_accumulator: (f32, f32), // Accumulates true position during pixel-aligned drag

    // Drag from gallery state
    gallery_drag: Option<GalleryDrag>,

    // Dialogs
    show_new_character_dialog: bool,
    show_new_part_dialog: bool,
    show_new_state_dialog: bool,
    show_new_animation_dialog: bool,
    show_load_dialog: bool,
    show_import_image_dialog: bool,
    show_import_rotation_dialog: bool,
    show_rename_dialog: bool,
    show_delete_confirm_dialog: bool,

    // Rename/delete context
    context_menu_target: Option<ContextMenuTarget>,
    rename_new_name: String,

    // Rotation import state (from character editor)
    pending_rotation_import: Option<u16>,

    // Dialog input buffers
    new_character_name: String,
    new_part_name: String,
    new_state_name: String,
    new_animation_name: String,
    selected_character_for_part: Option<String>,
    selected_part_for_state: Option<String>,
    selected_state_for_import: Option<String>,
    selected_rotation_for_import: u16,
    file_path_input: String,
    import_image_path: String,

    // Status message
    status_message: Option<(String, f64)>, // (message, timestamp)

    // Loaded textures cache (texture_id -> egui::TextureHandle)
    texture_cache: HashMap<String, egui::TextureHandle>,
}

#[derive(Clone)]
struct DraggedPart {
    character_name: String,
    part_name: String,
    state_name: String,
}

#[derive(Clone)]
struct GalleryDrag {
    character_name: String,
    part_name: String,
    state_name: String,
}

#[derive(Clone, Debug)]
enum ContextMenuTarget {
    Character { char_name: String },
    Part { char_name: String, part_name: String },
    Animation { char_name: String, anim_index: usize, anim_name: String },
    Frame { char_name: String, anim_index: usize, frame_index: usize },
    Layer { layer_id: u64, layer_name: String },
}

impl AppState {
    fn new() -> Self {
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
            pixel_aligned: true,
            canvas_offset: (0.0, 0.0),
            is_panning: false,
            active_character: None,
            active_tab: ActiveTab::Canvas,
            editor_selected_part: None,
            editor_selected_state: None,
            dragging_part: None,
            drag_offset: (0.0, 0.0),
            drag_accumulator: (0.0, 0.0),
            gallery_drag: None,
            show_new_character_dialog: false,
            show_new_part_dialog: false,
            show_new_state_dialog: false,
            show_new_animation_dialog: false,
            show_load_dialog: false,
            show_import_image_dialog: false,
            show_import_rotation_dialog: false,
            show_rename_dialog: false,
            show_delete_confirm_dialog: false,
            context_menu_target: None,
            rename_new_name: String::new(),
            pending_rotation_import: None,
            new_character_name: String::new(),
            new_part_name: String::new(),
            new_state_name: String::new(),
            new_animation_name: String::new(),
            selected_character_for_part: None,
            selected_part_for_state: None,
            selected_state_for_import: None,
            selected_rotation_for_import: 0,
            file_path_input: String::new(),
            import_image_path: String::new(),
            status_message: None,
            texture_cache: HashMap::new(),
        }
    }

    fn place_part_on_canvas(&mut self, character: &str, part: &str, state: &str, x: f32, y: f32) {
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

            let mut placed = PlacedPart::new(id, character, part, state)
                .with_layer_name(&layer_name);
            placed.position = (x, y);

            if let Some(ref char_name) = char_name {
                if let Some(character_obj) = project.get_character_mut(char_name) {
                    if let Some(anim) = character_obj.animations.get_mut(current_anim) {
                        if let Some(frame) = anim.frames.get_mut(current_frame) {
                            frame.placed_parts.push(placed);
                            self.selected_part_id = Some(id);
                        }
                    }
                }
            }
        }
    }

    fn get_selected_placed_part(&self) -> Option<&PlacedPart> {
        let id = self.selected_part_id?;
        let anim = self.current_animation()?;
        let frame = anim.frames.get(self.current_frame)?;
        frame.placed_parts.iter().find(|p| p.id == id)
    }

    fn get_selected_placed_part_mut(&mut self) -> Option<&mut PlacedPart> {
        let id = self.selected_part_id?;
        let frame_idx = self.current_frame;
        let anim = self.current_animation_mut()?;
        let frame = anim.frames.get_mut(frame_idx)?;
        frame.placed_parts.iter_mut().find(|p| p.id == id)
    }

    fn delete_selected_part(&mut self) {
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

    fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = Some((message.into(), 0.0)); // timestamp will be set in UI
    }

    fn save_project(&mut self) -> Result<(), String> {
        let project = self.project.as_ref().ok_or("No project to save")?;
        let path = self.project_path.as_ref().ok_or("No file path set")?;

        let json = project.to_json().map_err(|e| format!("Serialize error: {}", e))?;
        fs::write(path, &json).map_err(|e| format!("Write error: {}", e))?;

        // Track saved state for dirty checking
        self.last_saved_json = Some(json);
        self.last_saved_time = Some(std::time::Instant::now());

        Ok(())
    }

    fn save_project_as(&mut self, path: &str) -> Result<(), String> {
        self.project_path = Some(PathBuf::from(path));
        let result = self.save_project();
        if result.is_ok() {
            self.config.add_recent(path);
        }
        result
    }

    fn load_project(&mut self, path: &str) -> Result<(), String> {
        let json = fs::read_to_string(path).map_err(|e| format!("Read error: {}", e))?;
        let project = Project::from_json(&json).map_err(|e| format!("Parse error: {}", e))?;

        // Track saved state
        self.last_saved_json = project.to_json().ok();
        self.last_saved_time = Some(std::time::Instant::now());

        self.project = Some(project);
        self.project_path = Some(PathBuf::from(path));
        self.current_animation = 0;
        self.current_frame = 0;
        self.selected_part_id = None;
        self.active_character = None;
        self.config.add_recent(path);

        Ok(())
    }

    fn new_project(&mut self) {
        let project = Project::new("Untitled");
        self.last_saved_json = project.to_json().ok();
        self.last_saved_time = None; // New project hasn't been saved yet
        self.project = Some(project);
        self.project_path = None;
        self.current_animation = 0;
        self.current_frame = 0;
        self.selected_part_id = None;
        self.active_character = None;
    }

    fn close_project(&mut self) {
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

    fn has_unsaved_changes(&self) -> bool {
        match (&self.project, &self.last_saved_json) {
            (Some(project), Some(saved_json)) => {
                project.to_json().ok().as_ref() != Some(saved_json)
            }
            (Some(_), None) => true, // Project exists but never saved
            _ => false,
        }
    }

    fn time_since_save(&self) -> Option<std::time::Duration> {
        self.last_saved_time.map(|t| t.elapsed())
    }

    fn active_character_ref(&self) -> Option<&Character> {
        let char_name = self.active_character.as_ref()?;
        self.project.as_ref()?.get_character(char_name)
    }

    fn active_character_mut(&mut self) -> Option<&mut Character> {
        let char_name = self.active_character.clone()?;
        self.project.as_mut()?.get_character_mut(&char_name)
    }

    fn current_animation(&self) -> Option<&Animation> {
        self.active_character_ref()?.animations.get(self.current_animation)
    }

    fn current_animation_mut(&mut self) -> Option<&mut Animation> {
        let anim_idx = self.current_animation;
        self.active_character_mut()?.animations.get_mut(anim_idx)
    }

    fn total_frames(&self) -> usize {
        self.current_animation().map(|a| a.frames.len()).unwrap_or(1)
    }

    fn zoom_in(&mut self) {
        // Find next higher zoom level
        for &level in &ZOOM_LEVELS {
            if level > self.zoom_level {
                self.zoom_level = level;
                return;
            }
        }
    }

    fn zoom_out(&mut self) {
        // Find next lower zoom level
        for &level in ZOOM_LEVELS.iter().rev() {
            if level < self.zoom_level {
                self.zoom_level = level;
                return;
            }
        }
    }
}

// Native file dialog functions (Windows only)
#[cfg(target_os = "windows")]
fn pick_save_file() -> Option<PathBuf> {
    FileDialog::new()
        .add_filter("Sprite Animator Project", &["sprite-animator.json", "json"])
        .set_file_name("project.sprite-animator.json")
        .save_file()
}

#[cfg(target_os = "windows")]
fn pick_open_file() -> Option<PathBuf> {
    FileDialog::new()
        .add_filter("Sprite Animator Project", &["sprite-animator.json", "json"])
        .pick_file()
}

#[cfg(target_os = "windows")]
fn pick_image_file() -> Option<PathBuf> {
    FileDialog::new()
        .add_filter("PNG Images", &["png"])
        .add_filter("All Images", &["png", "jpg", "jpeg", "gif", "bmp"])
        .pick_file()
}

// Export file dialogs
#[cfg(target_os = "windows")]
fn pick_export_file() -> Option<PathBuf> {
    FileDialog::new()
        .add_filter("PNG Image", &["png"])
        .set_file_name("spritesheet.png")
        .save_file()
}

#[cfg(target_os = "windows")]
fn pick_export_folder() -> Option<PathBuf> {
    FileDialog::new().pick_folder()
}

// Fallback for non-Windows (returns None, uses text input instead)
#[cfg(not(target_os = "windows"))]
fn pick_save_file() -> Option<PathBuf> { None }
#[cfg(not(target_os = "windows"))]
fn pick_open_file() -> Option<PathBuf> { None }
#[cfg(not(target_os = "windows"))]
fn pick_image_file() -> Option<PathBuf> { None }
#[cfg(not(target_os = "windows"))]
fn pick_export_file() -> Option<PathBuf> { None }
#[cfg(not(target_os = "windows"))]
fn pick_export_folder() -> Option<PathBuf> { None }

fn setup(mut commands: Commands, mut state: ResMut<AppState>) {
    commands.spawn(Camera2d);
    *state = AppState::new();
}

fn configure_fonts(mut contexts: EguiContexts) {
    let mut fonts = egui::FontDefinitions::default();

    // Embed Noto Sans font for better Unicode support
    fonts.font_data.insert(
        "noto_sans".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/NotoSans-Regular.ttf")),
    );

    // Add as primary proportional font
    fonts.families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "noto_sans".to_owned());

    contexts.ctx_mut().set_fonts(fonts);
}

fn handle_window_close(
    mut events: EventReader<WindowCloseRequested>,
    mut state: ResMut<AppState>,
    mut app_exit: EventWriter<bevy::app::AppExit>,
) {
    for _event in events.read() {
        if state.has_unsaved_changes() {
            // Show the unsaved changes dialog instead of closing
            state.pending_action = Some(PendingAction::Exit);
        } else {
            // No unsaved changes, exit immediately
            app_exit.send(bevy::app::AppExit::Success);
        }
    }
}

fn ui_system(mut contexts: EguiContexts, mut state: ResMut<AppState>, time: Res<Time>) {
    let ctx = contexts.ctx_mut();

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
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
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
                    } else {
                        // Try native file dialog first
                        if let Some(path) = pick_open_file() {
                            let path_str = path.to_string_lossy().to_string();
                            match state.load_project(&path_str) {
                                Ok(()) => state.set_status(format!("Loaded {}", path_str)),
                                Err(e) => state.set_status(format!("Load failed: {}", e)),
                            }
                        } else {
                            // Fallback to text input dialog
                            state.show_load_dialog = true;
                            state.file_path_input.clear();
                        }
                    }
                    ui.close_menu();
                }
                ui.separator();
                let has_project = state.project.is_some();
                let has_path = state.project_path.is_some();

                // Save - only if we have a path already
                if ui.add_enabled(has_project && has_path, egui::Button::new("Save")).clicked() {
                    match state.save_project() {
                        Ok(()) => state.set_status("Project saved"),
                        Err(e) => state.set_status(format!("Save failed: {}", e)),
                    }
                    ui.close_menu();
                }
                if ui.add_enabled(has_project, egui::Button::new("Save As...")).clicked() {
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
                if ui.add_enabled(has_project, egui::Button::new("Close Project")).clicked() {
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
            });

            ui.menu_button("Edit", |ui| {
                if ui.button("Undo").clicked() {
                    ui.close_menu();
                }
                if ui.button("Redo").clicked() {
                    ui.close_menu();
                }
            });

            ui.menu_button("View", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Zoom:");
                    egui::ComboBox::from_id_salt("zoom_level")
                        .selected_text(format!("{:.2}x", state.zoom_level))
                        .show_ui(ui, |ui| {
                            for &level in &ZOOM_LEVELS {
                                let label = format!("{:.2}x", level);
                                if ui.selectable_value(&mut state.zoom_level, level, &label).clicked() {
                                    ui.close_menu();
                                }
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("UI Scale:");
                    let old_scale = state.config.ui_scale;
                    ui.add(egui::Slider::new(&mut state.config.ui_scale, 0.75..=2.0).step_by(0.25));
                    if state.config.ui_scale != old_scale {
                        state.config.save();
                    }
                });
                ui.separator();
                ui.checkbox(&mut state.show_grid, "Show Grid");
                ui.checkbox(&mut state.show_labels, "Show Labels");
                ui.checkbox(&mut state.show_overlay_info, "Show Overlay Info");
            });

            let has_project = state.project.is_some();
            ui.menu_button("Character", |ui| {
                if ui.add_enabled(has_project, egui::Button::new("New Character...")).clicked() {
                    state.show_new_character_dialog = true;
                    state.new_character_name.clear();
                    ui.close_menu();
                }

                let has_characters = state.project.as_ref()
                    .map(|p| !p.characters.is_empty())
                    .unwrap_or(false);

                if ui.add_enabled(has_characters, egui::Button::new("Add Part...")).clicked() {
                    state.show_new_part_dialog = true;
                    state.new_part_name.clear();
                    ui.close_menu();
                }
                if ui.add_enabled(has_characters, egui::Button::new("Add State...")).clicked() {
                    state.show_new_state_dialog = true;
                    state.new_state_name.clear();
                    ui.close_menu();
                }
            });

            ui.menu_button("Animation", |ui| {
                if ui.add_enabled(has_project, egui::Button::new("New Animation...")).clicked() {
                    state.show_new_animation_dialog = true;
                    state.new_animation_name.clear();
                    ui.close_menu();
                }
                ui.separator();
                if ui.add_enabled(has_project, egui::Button::new("Add Frame")).clicked() {
                    if let Some(anim) = state.current_animation_mut() {
                        anim.add_frame();
                    }
                    ui.close_menu();
                }
                if ui.add_enabled(has_project, egui::Button::new("Delete Frame")).clicked() {
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
            });

            ui.menu_button("Export", |ui| {
                let has_animation = state.current_animation().map(|a| !a.frames.is_empty()).unwrap_or(false);
                if ui.add_enabled(has_project && has_animation, egui::Button::new("Export Current Animation...")).clicked() {
                    if let Some(path) = pick_export_file() {
                        let path_str = path.to_string_lossy().to_string();
                        match export_current_animation(&state, &path_str) {
                            Ok((png_path, json_path)) => {
                                state.set_status(format!("Exported to {} and {}", png_path, json_path));
                            }
                            Err(e) => {
                                state.set_status(format!("Export failed: {}", e));
                            }
                        }
                    }
                    ui.close_menu();
                }
                if ui.add_enabled(has_project, egui::Button::new("Export All Animations...")).clicked() {
                    if let Some(path) = pick_export_folder() {
                        let path_str = path.to_string_lossy().to_string();
                        match export_all_animations(&state, &path_str) {
                            Ok(count) => {
                                state.set_status(format!("Exported {} animations to {}", count, path_str));
                            }
                            Err(e) => {
                                state.set_status(format!("Export failed: {}", e));
                            }
                        }
                    }
                    ui.close_menu();
                }
            });
        });

    });

    // Status bar (bottom-most panel, before timeline)
    egui::TopBottomPanel::bottom("status_bar")
        .max_height(24.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some((ref msg, _)) = state.status_message {
                    ui.label(msg);
                } else {
                    ui.label("Ready");
                }
            });
        });

    // Asset browser (left panel) - Simplified: flat character list + animations
    egui::SidePanel::left("asset_browser")
        .default_width(250.0)
        .min_width(200.0)
        .resizable(true)
        .show(ctx, |ui| {
            if let Some(ref project) = state.project.clone() {
                // Character dropdown
                ui.heading("Character");
                let current_char_name = state.active_character.clone().unwrap_or_else(|| "(None)".to_string());
                egui::ComboBox::from_id_salt("character_selector")
                    .selected_text(&current_char_name)
                    .width(ui.available_width() - 10.0)
                    .show_ui(ui, |ui| {
                        for character in &project.characters {
                            let is_selected = state.active_character.as_ref() == Some(&character.name);
                            if ui.selectable_label(is_selected, &character.name).clicked() {
                                state.active_character = Some(character.name.clone());
                                state.editor_selected_part = character.parts.first().map(|p| p.name.clone());
                                state.editor_selected_state = None;
                                state.current_animation = 0;
                                state.current_frame = 0;
                            }
                        }
                        ui.separator();
                        if ui.selectable_label(false, "+ New Character...").clicked() {
                            state.show_new_character_dialog = true;
                            state.new_character_name.clear();
                        }
                    });

                // Character actions (only if a character is selected)
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
                            if let Some(ref mut project) = state.project {
                                if let Some(original) = project.get_character(&active_char) {
                                    let mut cloned = original.clone();
                                    cloned.name = format!("{} (Copy)", original.name);
                                    project.add_character(cloned);
                                    state.set_status(format!("Cloned character '{}'", active_char));
                                }
                            }
                        }
                        if ui.small_button("Delete").clicked() {
                            state.context_menu_target = Some(ContextMenuTarget::Character {
                                char_name: active_char.clone(),
                            });
                            state.show_delete_confirm_dialog = true;
                        }
                    });

                    ui.separator();

                    // Animations list
                    ui.heading("Animations");
                    if let Some(character) = project.get_character(&active_char) {
                        for (i, anim) in character.animations.iter().enumerate() {
                            let is_selected_anim = i == state.current_animation;
                            let anim_text = if is_selected_anim {
                                egui::RichText::new(&anim.name).strong()
                            } else {
                                egui::RichText::new(&anim.name)
                            };

                            let response = ui.selectable_label(is_selected_anim, anim_text);
                            if response.clicked() {
                                state.current_animation = i;
                                state.current_frame = 0;
                            }

                            // Right-click context menu for animations
                            let anim_name = anim.name.clone();
                            let char_name_for_menu = active_char.clone();
                            response.context_menu(|ui| {
                                if ui.button("Rename...").clicked() {
                                    state.context_menu_target = Some(ContextMenuTarget::Animation {
                                        char_name: char_name_for_menu.clone(),
                                        anim_index: i,
                                        anim_name: anim_name.clone(),
                                    });
                                    state.rename_new_name = anim_name.clone();
                                    state.show_rename_dialog = true;
                                    ui.close_menu();
                                }
                                if ui.button("Delete").clicked() {
                                    state.context_menu_target = Some(ContextMenuTarget::Animation {
                                        char_name: char_name_for_menu.clone(),
                                        anim_index: i,
                                        anim_name: anim_name.clone(),
                                    });
                                    state.show_delete_confirm_dialog = true;
                                    ui.close_menu();
                                }
                            });
                        }

                        if ui.small_button("+ Animation").clicked() {
                            state.show_new_animation_dialog = true;
                            state.new_animation_name.clear();
                        }
                    }
                } else if project.characters.is_empty() {
                    ui.label("No characters yet. Create one above.");
                } else {
                    ui.label("Select a character above.");
                }

                // Parts Gallery - show when a character is active
                if let Some(ref active_char_name) = state.active_character.clone() {
                    if let Some(character) = project.get_character(&active_char_name) {
                        ui.add_space(10.0);
                        ui.separator();
                        ui.heading("Parts Gallery");
                        ui.label("(Drag to canvas)");
                        ui.separator();

                        // Show parts with thumbnails in a grid
                        let gallery_parts: Vec<(String, String, Option<String>)> = character.parts.iter()
                            .map(|p| {
                                // Get the 0° rotation of the first state for thumbnail
                                let thumb_data = p.states.first()
                                    .and_then(|s| s.rotations.get(&0))
                                    .and_then(|r| r.image_data.clone());
                                (p.name.clone(), p.states.first().map(|s| s.name.clone()).unwrap_or_else(|| "default".to_string()), thumb_data)
                            })
                            .collect();

                        let char_name_for_gallery = active_char_name.clone();
                        let gallery_size = 48.0;
                        let items_per_row = ((ui.available_width() - 20.0) / (gallery_size + 8.0)).max(1.0) as usize;

                        egui::Grid::new("parts_gallery_grid")
                            .spacing([4.0, 4.0])
                            .show(ui, |ui| {
                                for (idx, (part_name, state_name, thumb_data)) in gallery_parts.iter().enumerate() {
                                    let texture_key = format!("gallery/{}/{}", char_name_for_gallery, part_name);

                                    // Load thumbnail texture if needed
                                    if let Some(ref data) = thumb_data {
                                        if !state.texture_cache.contains_key(&texture_key) {
                                            if let Ok(tex) = decode_base64_to_texture(ui.ctx(), &texture_key, data) {
                                                state.texture_cache.insert(texture_key.clone(), tex);
                                            }
                                        }
                                    }

                                    // Draw gallery item
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(gallery_size, gallery_size + 14.0),
                                        egui::Sense::drag(),
                                    );

                                    let image_rect = egui::Rect::from_min_size(
                                        rect.min,
                                        egui::vec2(gallery_size, gallery_size),
                                    );

                                    // Background
                                    let bg_color = if response.dragged() || response.hovered() {
                                        egui::Color32::from_rgb(80, 80, 100)
                                    } else {
                                        egui::Color32::from_rgb(50, 50, 60)
                                    };
                                    ui.painter().rect_filled(image_rect, 4.0, bg_color);

                                    // Draw thumbnail or placeholder
                                    if let Some(texture) = state.texture_cache.get(&texture_key) {
                                        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                                        ui.painter().image(texture.id(), image_rect.shrink(2.0), uv, egui::Color32::WHITE);
                                    } else {
                                        // Placeholder with part name
                                        ui.painter().text(
                                            image_rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            &part_name.chars().take(3).collect::<String>(),
                                            egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale)),
                                            egui::Color32::GRAY,
                                        );
                                    }

                                    // Border
                                    ui.painter().rect_stroke(
                                        image_rect,
                                        4.0,
                                        egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 100, 120)),
                                    );

                                    // Part name below
                                    let label_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x, rect.min.y + gallery_size + 1.0),
                                        egui::vec2(gallery_size, scaled_font(12.0, state.config.ui_scale)),
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
                                        egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale)),
                                        egui::Color32::WHITE,
                                    );

                                    // Change cursor for draggable items
                                    if response.dragged() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                                    } else if response.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                                    }

                                    // Handle drag start
                                    if response.drag_started() {
                                        state.gallery_drag = Some(GalleryDrag {
                                            character_name: char_name_for_gallery.clone(),
                                            part_name: part_name.clone(),
                                            state_name: state_name.clone(),
                                        });
                                    }

                                    // End row after items_per_row
                                    if (idx + 1) % items_per_row == 0 {
                                        ui.end_row();
                                    }
                                }
                            });
                    }
                } else {
                    ui.label("");
                    ui.label("Select a character to see animations");
                }
            }
        });

    // Inspector (right panel)
    egui::SidePanel::right("inspector")
        .default_width(280.0)
        .min_width(200.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("Inspector");
            ui.separator();

            // Show selected part properties
            let selected_info = state.get_selected_placed_part().map(|p| {
                (p.character_name.clone(), p.part_name.clone(), p.state_name.clone(), p.position, p.rotation, p.z_override)
            });

            // Get available states for the selected part
            let available_states: Vec<String> = if let Some((ref char_name, ref part_name, _, _, _, _)) = selected_info {
                state.project.as_ref()
                    .and_then(|p| p.get_character(char_name))
                    .and_then(|c| c.get_part(part_name))
                    .map(|p| p.states.iter().map(|s| s.name.clone()).collect())
                    .unwrap_or_default()
            } else {
                vec![]
            };

            if let Some((_character_name, part_name, current_state, position, rotation, _z_override)) = selected_info {
                ui.label(format!("Selected part: {}", part_name));
                ui.separator();

                // State selector
                let mut selected_state = current_state.clone();
                ui.horizontal(|ui| {
                    ui.label("State:");
                    egui::ComboBox::from_id_salt("part_state")
                        .selected_text(&selected_state)
                        .show_ui(ui, |ui| {
                            for state_name in &available_states {
                                if ui.selectable_value(&mut selected_state, state_name.clone(), state_name).changed() {
                                    if let Some(part) = state.get_selected_placed_part_mut() {
                                        part.state_name = selected_state.clone();
                                        // Clear texture cache to reload with new state
                                        state.texture_cache.clear();
                                    }
                                }
                            }
                        });
                });

                // Position
                ui.horizontal(|ui| {
                    ui.label("Position:");
                    let was_pixel_aligned = state.pixel_aligned;
                    if ui.checkbox(&mut state.pixel_aligned, "Pixel aligned").changed() {
                        // Snap to nearest integer when enabling pixel alignment
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
                    if ui.add(egui::DragValue::new(&mut pos_x).speed(1.0)).changed() {
                        if let Some(part) = state.get_selected_placed_part_mut() {
                            part.position.0 = if pixel_aligned { pos_x.round() } else { pos_x };
                        }
                    }
                    ui.label("  Y:");
                    if ui.add(egui::DragValue::new(&mut pos_y).speed(1.0)).changed() {
                        if let Some(part) = state.get_selected_placed_part_mut() {
                            part.position.1 = if pixel_aligned { pos_y.round() } else { pos_y };
                        }
                    }
                });

                // Rotation
                let mut rot = rotation;
                ui.horizontal(|ui| {
                    ui.label("Rotation:");
                    egui::ComboBox::from_id_salt("part_rotation")
                        .selected_text(format!("{}°", rot))
                        .show_ui(ui, |ui| {
                            for angle in [0, 45, 90, 135, 180, 225, 270, 315] {
                                if ui.selectable_value(&mut rot, angle, format!("{}°", angle)).changed() {
                                    if let Some(part) = state.get_selected_placed_part_mut() {
                                        part.rotation = rot;
                                    }
                                }
                            }
                        });
                });

            } else {
                ui.label("No selected part");
            }

            ui.separator();

            // Layers panel - shows all placed parts in current frame
            ui.heading("Layers");

            // Get layers for current frame (need to collect info for UI)
            let layers: Vec<(u64, String, usize)> = {
                if let Some(anim) = state.current_animation() {
                    if let Some(frame) = anim.frames.get(state.current_frame) {
                        frame.placed_parts.iter().enumerate()
                            .map(|(idx, p)| (p.id, if p.layer_name.is_empty() { p.part_name.clone() } else { p.layer_name.clone() }, idx))
                            .collect()
                    } else { vec![] }
                } else { vec![] }
            };

            if layers.is_empty() {
                ui.label("(No layers)");
            } else {
                // Show layers in reverse order (top layer first in UI)
                let mut move_up: Option<usize> = None;
                let mut move_down: Option<usize> = None;

                for (id, name, idx) in layers.iter().rev() {
                    let is_selected = state.selected_part_id == Some(*id);
                    let layer_id = *id;
                    let layer_name = name.clone();

                    ui.horizontal(|ui| {
                        // Layer selection
                        let response = ui.selectable_label(is_selected, &format!("{}", name));
                        if response.clicked() {
                            state.selected_part_id = Some(*id);
                        }

                        // Context menu for delete
                        response.context_menu(|ui| {
                            if ui.button("Delete").clicked() {
                                state.context_menu_target = Some(ContextMenuTarget::Layer {
                                    layer_id,
                                    layer_name: layer_name.clone(),
                                });
                                state.show_delete_confirm_dialog = true;
                                ui.close_menu();
                            }
                        });

                        // Move up (toward end of list = drawn on top)
                        if ui.small_button("⏶").clicked() && *idx < layers.len() - 1 {
                            move_up = Some(*idx);
                        }
                        // Move down (toward start of list = drawn below)
                        if ui.small_button("⏷").clicked() && *idx > 0 {
                            move_down = Some(*idx);
                        }

                        // Delete button
                        if ui.small_button("×").clicked() {
                            state.context_menu_target = Some(ContextMenuTarget::Layer {
                                layer_id,
                                layer_name: layer_name.clone(),
                            });
                            state.show_delete_confirm_dialog = true;
                        }
                    });
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
            }

            ui.separator();

            // Canvas Settings
            ui.heading("Canvas Settings");
            if let Some(ref mut project) = state.project {
                ui.horizontal(|ui| {
                    ui.label("Size:");
                    let mut w = project.canvas_size.0 as i32;
                    let mut h = project.canvas_size.1 as i32;
                    ui.add(egui::DragValue::new(&mut w).speed(1).range(8..=512));
                    ui.label("x");
                    ui.add(egui::DragValue::new(&mut h).speed(1).range(8..=512));
                    project.canvas_size = (w.max(8) as u32, h.max(8) as u32);
                });
            }
            ui.horizontal(|ui| {
                ui.label("Zoom:");
                egui::ComboBox::from_id_salt("zoom_level_canvas")
                    .selected_text(format!("{:.2}x", state.zoom_level))
                    .show_ui(ui, |ui| {
                        for &level in &ZOOM_LEVELS {
                            ui.selectable_value(&mut state.zoom_level, level, format!("{:.2}x", level));
                        }
                    });
            });
            ui.checkbox(&mut state.show_grid, "Show grid");
            ui.checkbox(&mut state.show_labels, "Show labels");

            ui.separator();

            // Reference Layer
            ui.heading("Reference Layer");
            if let Some(ref mut project) = state.project {
                ui.checkbox(&mut project.reference_layer.visible, "Visible");
                if ui.button("Load Image...").clicked() {
                    // TODO: Load reference image
                }
                ui.horizontal(|ui| {
                    ui.label("Opacity:");
                    ui.add(egui::Slider::new(&mut project.reference_layer.opacity, 0.0..=1.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Scale:");
                    ui.add(egui::DragValue::new(&mut project.reference_layer.scale).speed(0.1).range(0.1..=10.0));
                });
            }
        });

    // Timeline (bottom panel) - only show if an animation is selected
    if state.current_animation().is_some() {
        let total_frames = state.total_frames();
        egui::TopBottomPanel::bottom("timeline")
            .exact_height(120.0)
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Timeline");

                // Show current animation name if one is selected
                if let Some(anim) = state.current_animation() {
                    ui.separator();
                    ui.label(&anim.name);
                }

                ui.separator();

                // Playback controls
                let play_text = if state.is_playing { "⏸" } else { "▶" };
                let play_tooltip = if state.is_playing { "Pause (Enter)" } else { "Play (Enter)" };
                if ui.button(play_text).on_hover_text(play_tooltip).clicked() {
                    state.is_playing = !state.is_playing;
                    if state.is_playing {
                        state.playback_time = 0.0; // Reset timer when starting
                        state.selected_part_id = None; // Deselect parts during playback
                    }
                }
                // Ctrl+S (Cmd+S on Mac) saves the project
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
                // Enter key toggles play/pause
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    state.is_playing = !state.is_playing;
                    if state.is_playing {
                        state.playback_time = 0.0;
                        state.selected_part_id = None; // Deselect parts during playback
                    }
                }
                // Delete key deletes selected part
                if ui.input(|i| i.key_pressed(egui::Key::Delete)) && state.selected_part_id.is_some() {
                    state.delete_selected_part();
                    state.set_status("Part deleted");
                }
                // Left/Right arrow keys navigate frames with wrap-around
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
                ui.label(format!("Frame: {} / {}", state.current_frame + 1, total_frames));

                ui.separator();
                ui.label("FPS:");
                let mut fps = state.current_animation().map(|a| a.fps).unwrap_or(12);
                if ui.add(egui::DragValue::new(&mut fps).speed(1).range(1..=60)).changed() {
                    if let Some(anim) = state.current_animation_mut() {
                        anim.fps = fps;
                    }
                }
            });

            ui.separator();

            // Frame buttons
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
                            state.selected_part_id = None; // Deselect on frame change
                        }
                        // Right-click context menu for delete
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
                    // Add frame buttons at the end (stacked vertically)
                    let mut add_blank = false;
                    let mut add_copy = false;
                    ui.vertical(|ui| {
                        if ui.add_sized([40.0, 28.0], egui::Button::new("+")).on_hover_text("Add blank frame").clicked() {
                            add_blank = true;
                        }
                        if ui.add_sized([40.0, 28.0], egui::Button::new("+cp")).on_hover_text("Copy last frame").clicked() {
                            add_copy = true;
                        }
                    });
                    if add_blank {
                        if let Some(anim) = state.current_animation_mut() {
                            anim.add_frame();
                        }
                    }
                    if add_copy {
                        // Clone the last frame
                        let cloned_frame = state.current_animation()
                            .and_then(|a| a.frames.last())
                            .cloned();
                        if let Some(mut new_frame) = cloned_frame {
                            // Assign new IDs to avoid conflicts
                            if let Some(ref mut project) = state.project {
                                for part in &mut new_frame.placed_parts {
                                    part.id = project.next_id();
                                }
                            }
                            // Add the frame
                            if let Some(anim) = state.current_animation_mut() {
                                anim.frames.push(new_frame);
                            }
                        }
                    }
                });
            });
        });
    }

    // Central canvas area with tabs
    egui::CentralPanel::default().show(ctx, |ui| {
        if state.project.is_none() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.heading("Welcome to Sprite Animator");
                ui.add_space(10.0);
                ui.label("Create or open a project to begin.");
                ui.add_space(20.0);

                if ui.button("New Project").clicked() {
                    state.new_project();
                }

                ui.add_space(10.0);

                if ui.button("Open Project...").clicked() {
                    if let Some(path) = pick_open_file() {
                        let path_str = path.to_string_lossy().to_string();
                        match state.load_project(&path_str) {
                            Ok(()) => state.set_status(format!("Loaded {}", path_str)),
                            Err(e) => state.set_status(format!("Load failed: {}", e)),
                        }
                    } else {
                        state.show_load_dialog = true;
                        state.file_path_input.clear();
                    }
                }

                // Recent Projects
                let recent = state.config.recent_projects.clone();
                let ui_scale = state.config.ui_scale;
                if !recent.is_empty() {
                    ui.add_space(30.0);
                    ui.heading("Recent Projects");
                    ui.add_space(15.0);

                    let mut project_to_open: Option<String> = None;
                    let mut project_to_remove: Option<String> = None;

                    for path in &recent {
                        let path_buf = PathBuf::from(path);
                        let filename = path_buf
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Unknown".to_string());

                        // Try to read project to get character names
                        let characters: Vec<String> = std::fs::read_to_string(&path_buf)
                            .ok()
                            .and_then(|json| Project::from_json(&json).ok())
                            .map(|p| p.characters.iter().map(|c| c.name.clone()).collect())
                            .unwrap_or_default();

                        // Card frame
                        let card_response = egui::Frame::none()
                            .fill(egui::Color32::from_rgb(45, 45, 55))
                            .rounding(8.0)
                            .inner_margin(12.0)
                            .show(ui, |ui| {
                                ui.set_min_width(300.0);

                                // Header row with title and remove button
                                ui.horizontal(|ui| {
                                    ui.heading(&filename);
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.small_button("×").on_hover_text("Remove from recent").clicked() {
                                            project_to_remove = Some(path.clone());
                                        }
                                    });
                                });

                                // Path as subheading
                                ui.label(
                                    egui::RichText::new(path)
                                        .size(scaled_font(12.0, ui_scale))
                                        .color(egui::Color32::GRAY)
                                );

                                // Characters list
                                if !characters.is_empty() {
                                    ui.add_space(8.0);
                                    ui.label(egui::RichText::new("Characters:").size(scaled_font(12.0, ui_scale)).strong());
                                    for char_name in &characters {
                                        ui.label(egui::RichText::new(format!("  • {}", char_name)).size(scaled_font(12.0, ui_scale)));
                                    }
                                }
                            });

                        // Highlight on hover and handle click
                        let card_rect = card_response.response.rect;
                        if ui.rect_contains_pointer(card_rect) {
                            // Draw highlight
                            ui.painter().rect_stroke(
                                card_rect,
                                8.0,
                                egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 150, 255)),
                            );
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);

                            // Check for click
                            if ui.input(|i| i.pointer.primary_clicked()) {
                                project_to_open = Some(path.clone());
                            }
                        }

                        ui.add_space(10.0);
                    }

                    // Handle actions after UI loop
                    if let Some(path) = project_to_open {
                        match state.load_project(&path) {
                            Ok(()) => state.set_status(format!("Loaded {}", path)),
                            Err(e) => {
                                state.set_status(format!("Load failed: {}", e));
                                // Remove from recent if file doesn't exist
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
        } else {
            // Only show tabs if a character is selected
            if state.active_character.is_some() {
                // Tab bar
                ui.horizontal(|ui| {
                    // Canvas tab
                    let is_canvas = matches!(state.active_tab, ActiveTab::Canvas);
                    if ui.selectable_label(is_canvas, "Canvas").clicked() {
                        state.active_tab = ActiveTab::Canvas;
                    }

                    // Character editor tab
                    if let Some(ref char_name) = state.active_character {
                        ui.separator();
                        let is_editor = matches!(state.active_tab, ActiveTab::CharacterEditor(_));
                        if ui.selectable_label(is_editor, format!("Edit Character: {}", char_name)).clicked() {
                            state.active_tab = ActiveTab::CharacterEditor(char_name.clone());
                        }
                    }
                });
                ui.separator();

                // Tab content
                match &state.active_tab {
                    ActiveTab::Canvas => {
                        render_canvas(ui, &mut state);
                    }
                    ActiveTab::CharacterEditor(_) => {
                        // Always use the currently selected character
                        if let Some(char_name) = state.active_character.clone() {
                            render_character_editor(ui, &mut state, &char_name);
                        }
                    }
                }
            } else {
                // No character selected - show helpful message
                let ui_scale = state.config.ui_scale;
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() / 3.0);
                        ui.label(
                            egui::RichText::new("Select or create a character in the left panel.")
                                .size(scaled_font(16.0, ui_scale))
                                .color(egui::Color32::GRAY)
                        );
                    });
                });
            }
        }
    });
}

/// Character editor with parts, states, and circular rotation wheel
fn render_character_editor(ui: &mut egui::Ui, state: &mut AppState, char_name: &str) {
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

        let selected_part_states: Vec<(String, bool)> = state.editor_selected_part.as_ref()
            .and_then(|pn| character.get_part(pn))
            .map(|p| p.states.iter().map(|s| {
                let has_images = s.rotations.values().any(|r| r.image_data.is_some());
                (s.name.clone(), has_images)
            }).collect())
            .unwrap_or_default();

        let selected_state_rotations: Vec<(u16, bool)> = state.editor_selected_part.as_ref()
            .and_then(|pn| character.get_part(pn))
            .and_then(|p| {
                let state_name = state.editor_selected_state.as_ref()
                    .or(p.states.first().map(|s| &s.name))?;
                p.states.iter().find(|s| &s.name == state_name)
            })
            .map(|s| s.rotations.iter().map(|(angle, r)| (*angle, r.image_data.is_some())).collect())
            .unwrap_or_default();

        (parts, selected_part_states, selected_state_rotations)
    };

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

/// Renders a circular rotation wheel for importing/viewing rotation sprites
fn render_rotation_wheel(ui: &mut egui::Ui, state: &mut AppState, char_name: &str, rotations: &[(u16, bool)]) {
    // Push a unique ID scope for this wheel instance
    let part_name = state.editor_selected_part.as_deref().unwrap_or("none");
    let state_name = state.editor_selected_state.as_deref().unwrap_or("default");
    ui.push_id(format!("rot_wheel_{}_{}", part_name, state_name), |ui| {
    let available = ui.available_size();
    let wheel_size = available.x.min(500.0);
    let center_y = 250.0; // Fixed height for the wheel area
    let radius = 120.0;  // Increased radius
    let slot_size = 64.0; // Larger slots for sprites

    // Reserve space for the wheel
    let (response, painter) = ui.allocate_painter(
        egui::vec2(wheel_size, center_y * 2.0),
        egui::Sense::hover(),
    );

    let center = response.rect.center();

    // Get the angles - default to 8 rotations (45 degree mode)
    let angles: Vec<u16> = if rotations.is_empty() {
        vec![0, 45, 90, 135, 180, 225, 270, 315]
    } else {
        let mut sorted: Vec<u16> = rotations.iter().map(|(a, _)| *a).collect();
        sorted.sort();
        sorted
    };

    // Create a map of angle -> has_image for quick lookup
    let rotation_map: std::collections::HashMap<u16, bool> = rotations.iter().cloned().collect();

    // Draw slots in a circle
    for angle in &angles {
        // Convert angle to radians - 0° = East (right), counterclockwise
        // 0° = E, 90° = N, 180° = W, 270° = S
        // In screen coordinates, Y increases downward, so negate sin
        let rad = (*angle as f32).to_radians();
        let slot_center = center + egui::vec2(rad.cos() * radius, -rad.sin() * radius);

        // Slot rectangle
        let slot_rect = egui::Rect::from_center_size(
            slot_center,
            egui::vec2(slot_size, slot_size),
        );

        let has_image = rotation_map.get(angle).copied().unwrap_or(false);

        // Draw slot background
        let bg_color = if has_image {
            egui::Color32::from_rgb(40, 40, 40) // Dark bg for images
        } else {
            egui::Color32::from_rgb(60, 60, 60) // Gray for empty
        };
        painter.rect_filled(slot_rect, 4.0, bg_color);

        // Try to draw the sprite image if it exists
        if has_image {
            let texture_key = format!("{}/{}/{}/{}", char_name, part_name, state_name, angle);

            // Get or create texture
            if !state.texture_cache.contains_key(&texture_key) {
                // Try to load the image data
                if let Some(ref project) = state.project {
                    if let Some(character) = project.get_character(char_name) {
                        if let Some(part) = character.get_part(part_name) {
                            if let Some(state_obj) = part.states.iter().find(|s| s.name == state_name) {
                                if let Some(rotation) = state_obj.rotations.get(angle) {
                                    if let Some(ref base64_data) = rotation.image_data {
                                        if let Ok(texture) = decode_base64_to_texture(ui.ctx(), &texture_key, base64_data) {
                                            state.texture_cache.insert(texture_key.clone(), texture);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Draw the texture if we have it
            if let Some(texture) = state.texture_cache.get(&texture_key) {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                painter.image(texture.id(), slot_rect.shrink(2.0), uv, egui::Color32::WHITE);
            }
        }

        painter.rect_stroke(slot_rect, 4.0, egui::Stroke::new(1.0, egui::Color32::WHITE));

        // Draw angle label below the slot
        let label_pos = slot_center + egui::vec2(0.0, slot_size / 2.0 + 10.0);
        painter.text(
            label_pos,
            egui::Align2::CENTER_CENTER,
            format!("{}°", angle),
            egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale)),
            egui::Color32::WHITE,
        );

        // Check for click on this slot
        let slot_response = ui.interact(slot_rect, ui.id().with(("rot_slot", *angle)), egui::Sense::click());
        if slot_response.clicked() {
            // Store angle for pending import (file picker will be called after render)
            state.pending_rotation_import = Some(*angle);
        }

        if slot_response.hovered() {
            painter.rect_stroke(slot_rect, 4.0, egui::Stroke::new(2.0, egui::Color32::YELLOW));
        }
    }

    // Draw center label
    let part_name = state.editor_selected_part.as_deref().unwrap_or("?");
    let state_name = state.editor_selected_state.as_deref().unwrap_or("default");
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        format!("{}\n{}", part_name, state_name),
        egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale)),
        egui::Color32::WHITE,
    );

    // Draw compass labels (0° = East, counterclockwise)
    let compass_radius = radius + slot_size / 2.0 + 25.0;
    let compass_labels = [
        (0.0_f32, "E (0°)"),
        (90.0_f32, "N (90°)"),
        (180.0_f32, "W (180°)"),
        (270.0_f32, "S (270°)"),
    ];
    for (deg, label) in compass_labels {
        let rad = deg.to_radians();
        // Same formula: x = cos, y = -sin for counterclockwise from East
        let pos = center + egui::vec2(rad.cos() * compass_radius, -rad.sin() * compass_radius);
        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale)),
            egui::Color32::GRAY,
        );
    }
    }); // close push_id
}

// Info needed for rendering a placed part
struct PlacedPartRenderInfo {
    id: u64,
    part_name: String,
    layer_name: String,
    character_name: String,
    state_name: String,
    rotation: u16,
    position: (f32, f32),
    image_data: Option<String>,
}

fn render_canvas(ui: &mut egui::Ui, state: &mut AppState) {
    // Capture values from project upfront to avoid borrow conflicts
    let (canvas_size, placed_parts, char_name, anim_name) = {
        let Some(ref project) = state.project else { return };
        let active_char = state.active_character.as_ref();

        let char_name = active_char.cloned().unwrap_or_default();
        let anim_name = active_char
            .and_then(|name| project.get_character(name))
            .and_then(|c| c.animations.get(state.current_animation))
            .map(|a| a.name.clone())
            .unwrap_or_default();

        let parts: Vec<PlacedPartRenderInfo> = active_char
            .and_then(|name| project.get_character(name))
            .and_then(|c| c.animations.get(state.current_animation))
            .and_then(|anim| anim.frames.get(state.current_frame))
            .map(|frame| {
                frame.placed_parts.iter()
                    .map(|p| {
                        // Look up image data for this part
                        let image_data = project.get_character(&p.character_name)
                            .and_then(|c| c.get_part(&p.part_name))
                            .and_then(|part| part.states.iter().find(|s| s.name == p.state_name))
                            .and_then(|s| s.rotations.get(&p.rotation))
                            .and_then(|r| r.image_data.clone());

                        PlacedPartRenderInfo {
                            id: p.id,
                            part_name: p.part_name.clone(),
                            layer_name: if p.layer_name.is_empty() { p.part_name.clone() } else { p.layer_name.clone() },
                            character_name: p.character_name.clone(),
                            state_name: p.state_name.clone(),
                            rotation: p.rotation,
                            position: p.position,
                            image_data,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        (project.canvas_size, parts, char_name, anim_name)
    };

    let available = ui.available_size();
    // Account for DPI scaling to get true 1:1 pixel rendering at zoom 1.0
    let ppp = ui.ctx().pixels_per_point();
    let effective_zoom = state.zoom_level / ppp;
    let canvas_w = canvas_size.0 as f32 * effective_zoom;
    let canvas_h = canvas_size.1 as f32 * effective_zoom;

    // Center the canvas with pan offset
    let offset_x = (available.x - canvas_w) / 2.0 + state.canvas_offset.0;
    let offset_y = (available.y - canvas_h) / 2.0 + state.canvas_offset.1;

    let (response, painter) = ui.allocate_painter(available, egui::Sense::click_and_drag());
    let canvas_rect = egui::Rect::from_min_size(
        response.rect.min + egui::vec2(offset_x, offset_y),
        egui::vec2(canvas_w, canvas_h),
    );

    // Check for panning input (space key or middle mouse button)
    let space_held = ui.input(|i| i.key_down(egui::Key::Space));
    let middle_mouse_held = ui.input(|i| i.pointer.middle_down());
    let is_panning = space_held || middle_mouse_held;
    state.is_panning = is_panning;

    // Set cursor to grabbing hand when panning
    if is_panning {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }

    // Draw canvas background
    painter.rect_filled(canvas_rect, 0.0, egui::Color32::from_rgb(40, 40, 50));

    // Draw grid if enabled
    if state.show_grid {
        let grid_color = egui::Color32::from_rgba_unmultiplied(100, 100, 100, 60);
        let cell_size = effective_zoom;

        let mut x = canvas_rect.min.x;
        while x <= canvas_rect.max.x {
            painter.line_segment(
                [egui::pos2(x, canvas_rect.min.y), egui::pos2(x, canvas_rect.max.y)],
                egui::Stroke::new(1.0, grid_color),
            );
            x += cell_size;
        }

        let mut y = canvas_rect.min.y;
        while y <= canvas_rect.max.y {
            painter.line_segment(
                [egui::pos2(canvas_rect.min.x, y), egui::pos2(canvas_rect.max.x, y)],
                egui::Stroke::new(1.0, grid_color),
            );
            y += cell_size;
        }
    }

    // Canvas border (draw before sprites so it appears underneath)
    painter.rect_stroke(
        canvas_rect,
        0.0,
        egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 100, 120)),
    );

    // Draw placed parts (in list order - later items drawn on top)
    let show_labels = state.show_labels;
    for part_info in &placed_parts {
        let screen_x = canvas_rect.min.x + part_info.position.0 * effective_zoom;
        let screen_y = canvas_rect.min.y + part_info.position.1 * effective_zoom;

        let is_selected = state.selected_part_id == Some(part_info.id);

        // Try to get or create texture for this part
        let texture_key = format!("{}/{}/{}/{}",
            part_info.character_name, part_info.part_name, part_info.state_name, part_info.rotation);

        let mut rendered_texture = false;
        let mut image_size = (16.0_f32, 16.0_f32);
        let mut part_rect = egui::Rect::NOTHING;

        if let Some(ref base64_data) = part_info.image_data {
            // Check if texture is already cached
            if !state.texture_cache.contains_key(&texture_key) {
                // Decode and create texture
                if let Ok(texture) = decode_base64_to_texture(ui.ctx(), &texture_key, base64_data) {
                    state.texture_cache.insert(texture_key.clone(), texture);
                }
            }

            if let Some(texture) = state.texture_cache.get(&texture_key) {
                let tex_size = texture.size_vec2();
                image_size = (tex_size.x, tex_size.y);
                let scaled_size = egui::vec2(tex_size.x * effective_zoom, tex_size.y * effective_zoom);
                part_rect = egui::Rect::from_min_size(
                    egui::pos2(screen_x, screen_y),
                    scaled_size,
                );

                // Draw the texture
                painter.image(
                    texture.id(),
                    part_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );

                rendered_texture = true;
            }
        }

        // Fallback: draw colored rectangle if no texture
        if !rendered_texture {
            let part_size_x = image_size.0 * effective_zoom;
            let part_size_y = image_size.1 * effective_zoom;
            part_rect = egui::Rect::from_min_size(
                egui::pos2(screen_x, screen_y),
                egui::vec2(part_size_x, part_size_y),
            );

            // Color based on part name hash for variety
            let hash = part_info.part_name.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32));
            let r = ((hash * 17) % 200 + 55) as u8;
            let g = ((hash * 31) % 200 + 55) as u8;
            let b = ((hash * 47) % 200 + 55) as u8;

            let fill_color = egui::Color32::from_rgba_unmultiplied(r, g, b, 180);
            painter.rect_filled(part_rect, 2.0, fill_color);

            // Draw part name in center
            painter.text(
                part_rect.center(),
                egui::Align2::CENTER_CENTER,
                &part_info.layer_name,
                egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale)),
                egui::Color32::WHITE,
            );
        }

        // Draw selection border (yellow) or label border (red)
        if is_selected {
            painter.rect_stroke(part_rect, 0.0, egui::Stroke::new(2.0, egui::Color32::YELLOW));
        } else if show_labels {
            painter.rect_stroke(part_rect, 0.0, egui::Stroke::new(1.0, egui::Color32::RED));
        }

        // Draw label box in top-left corner if show_labels is enabled
        if show_labels {
            let label_text = &part_info.layer_name;
            let font = egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale));
            let text_color = egui::Color32::WHITE;
            let bg_color = egui::Color32::from_rgba_unmultiplied(200, 0, 0, 220);

            // Measure text size
            let galley = painter.layout_no_wrap(label_text.clone(), font.clone(), text_color);
            let text_size = galley.size();
            let padding = 2.0;

            let label_height = text_size.y + padding * 2.0;
            let label_rect = egui::Rect::from_min_size(
                egui::pos2(part_rect.min.x, part_rect.min.y - label_height),
                egui::vec2(text_size.x + padding * 2.0, label_height),
            );

            // Draw label background
            painter.rect_filled(label_rect, 0.0, bg_color);

            // Draw label text
            painter.galley(
                egui::pos2(label_rect.min.x + padding, label_rect.min.y + padding),
                galley,
                text_color,
            );
        }
    }

    // Handle panning - space uses pointer movement, middle mouse uses drag
    if space_held {
        // Space: pan by just moving the mouse (no click needed)
        let delta = ui.input(|i| i.pointer.delta());
        state.canvas_offset.0 += delta.x;
        state.canvas_offset.1 += delta.y;
    } else if middle_mouse_held && response.dragged() {
        // Middle mouse: pan while dragging
        let delta = response.drag_delta();
        state.canvas_offset.0 += delta.x;
        state.canvas_offset.1 += delta.y;
    }

    // Handle gallery drag drop onto canvas
    let mouse_released = ui.input(|i| i.pointer.any_released());
    if mouse_released && state.gallery_drag.is_some() {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            // Check if dropped within canvas bounds
            if canvas_rect.contains(pos) {
                // Convert screen position to canvas coordinates
                let canvas_x = (pos.x - canvas_rect.min.x) / effective_zoom;
                let canvas_y = (pos.y - canvas_rect.min.y) / effective_zoom;

                // Place the part (centered on drop position)
                if let Some(gallery_drag) = state.gallery_drag.take() {
                    // Get sprite size to center it
                    let texture_key = format!("gallery/{}/{}", gallery_drag.character_name, gallery_drag.part_name);
                    let sprite_size = state.texture_cache.get(&texture_key)
                        .map(|t| t.size_vec2())
                        .unwrap_or(egui::vec2(16.0, 16.0));

                    // Offset by half sprite size to center on cursor
                    let centered_x = canvas_x - sprite_size.x / 2.0;
                    let centered_y = canvas_y - sprite_size.y / 2.0;

                    let pixel_aligned = state.pixel_aligned;
                    let (x, y) = if pixel_aligned {
                        (centered_x.round(), centered_y.round())
                    } else {
                        (centered_x, centered_y)
                    };
                    state.place_part_on_canvas(
                        &gallery_drag.character_name,
                        &gallery_drag.part_name,
                        &gallery_drag.state_name,
                        x,
                        y,
                    );
                    state.set_status(format!("Placed {} at ({:.0}, {:.0})", gallery_drag.part_name, x, y));
                }
            }
        }
        state.gallery_drag = None;
    }

    // Change cursor when hovering over draggable parts
    if !is_panning && response.hovered() {
        if let Some(pos) = response.hover_pos() {
            let mut hovering_part = false;
            for part_info in placed_parts.iter().rev() {
                let screen_x = canvas_rect.min.x + part_info.position.0 * effective_zoom;
                let screen_y = canvas_rect.min.y + part_info.position.1 * effective_zoom;
                let part_size = if let Some(texture) = state.texture_cache.get(&format!(
                    "{}/{}/{}/{}", part_info.character_name, part_info.part_name,
                    part_info.state_name, part_info.rotation
                )) {
                    texture.size_vec2() * effective_zoom
                } else {
                    egui::vec2(16.0, 16.0) * effective_zoom
                };
                let part_rect = egui::Rect::from_min_size(
                    egui::pos2(screen_x, screen_y),
                    part_size,
                );
                if part_rect.contains(pos) {
                    hovering_part = true;
                    break;
                }
            }
            if hovering_part {
                if response.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                } else {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }
            }
        }
    }

    // Handle mouse interactions - select on mousedown (not mouseup)
    // Skip if we're panning
    let should_check_selection = !is_panning && ui.input(|i| i.pointer.any_pressed());
    if should_check_selection {
        if let Some(pos) = response.interact_pointer_pos() {
            // Collect all parts whose bounding boxes contain the click (top to bottom)
            let mut candidates: Vec<(u64, Option<&String>, f32, f32, egui::Vec2)> = Vec::new();

            for part_info in placed_parts.iter().rev() { // Reverse for top-to-bottom
                let screen_x = canvas_rect.min.x + part_info.position.0 * effective_zoom;
                let screen_y = canvas_rect.min.y + part_info.position.1 * effective_zoom;
                // Use cached texture size if available, otherwise default 16x16
                let part_size = if let Some(texture) = state.texture_cache.get(&format!(
                    "{}/{}/{}/{}", part_info.character_name, part_info.part_name,
                    part_info.state_name, part_info.rotation
                )) {
                    texture.size_vec2() * effective_zoom
                } else {
                    egui::vec2(16.0, 16.0) * effective_zoom
                };
                let part_rect = egui::Rect::from_min_size(
                    egui::pos2(screen_x, screen_y),
                    part_size,
                );
                if part_rect.contains(pos) {
                    candidates.push((
                        part_info.id,
                        part_info.image_data.as_ref(),
                        screen_x,
                        screen_y,
                        part_size,
                    ));
                }
            }

            // Find the topmost part with a non-transparent pixel at click location
            let mut clicked_part = None;
            let mut topmost_fallback = None;

            for (id, image_data, screen_x, screen_y, _part_size) in &candidates {
                // Remember the topmost as fallback
                if topmost_fallback.is_none() {
                    topmost_fallback = Some(*id);
                }

                // Calculate pixel coordinates within the part
                let pixel_x = ((pos.x - screen_x) / effective_zoom) as u32;
                let pixel_y = ((pos.y - screen_y) / effective_zoom) as u32;

                // Check if pixel is opaque
                if let Some(data) = image_data {
                    if is_pixel_opaque(data, pixel_x, pixel_y) {
                        clicked_part = Some(*id);
                        break;
                    }
                } else {
                    // No image data means we can't check transparency, treat as opaque
                    clicked_part = Some(*id);
                    break;
                }
            }

            // Use the first opaque hit, or fall back to topmost if all transparent
            state.selected_part_id = clicked_part.or(topmost_fallback);

            // Initialize drag accumulator if we selected a part
            if let Some(part) = state.get_selected_placed_part() {
                state.drag_accumulator = part.position;
            }
        }
    }

    if !is_panning && response.dragged() && state.selected_part_id.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        let delta = response.drag_delta();
        let zoom = effective_zoom;
        let pixel_aligned = state.pixel_aligned;

        // Accumulate the true position
        state.drag_accumulator.0 += delta.x / zoom;
        state.drag_accumulator.1 += delta.y / zoom;

        // Capture values before mutable borrow
        let new_pos = if pixel_aligned {
            (state.drag_accumulator.0.round(), state.drag_accumulator.1.round())
        } else {
            state.drag_accumulator
        };

        // Set the displayed position
        if let Some(part) = state.get_selected_placed_part_mut() {
            part.position = new_pos;
        }
    }

    // Handle mouse wheel for zooming
    if response.hovered() {
        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if scroll_delta > 0.0 {
            state.zoom_in();
        } else if scroll_delta < 0.0 {
            state.zoom_out();
        }
    }

    // Overlay info (character name, animation name, canvas info)
    if state.show_overlay_info {
        // Character and animation name at top left corner
        if !char_name.is_empty() {
            painter.text(
                response.rect.min + egui::vec2(10.0, 10.0),
                egui::Align2::LEFT_TOP,
                &char_name,
                egui::FontId::proportional(scaled_font(20.0, state.config.ui_scale)),
                egui::Color32::WHITE,
            );
            painter.text(
                response.rect.min + egui::vec2(10.0, 34.0),
                egui::Align2::LEFT_TOP,
                &anim_name,
                egui::FontId::proportional(scaled_font(14.0, state.config.ui_scale)),
                egui::Color32::GRAY,
            );
        }

        // Canvas info at bottom left corner
        let parts_count = placed_parts.len();
        painter.text(
            egui::pos2(response.rect.min.x + 10.0, response.rect.max.y - 10.0),
            egui::Align2::LEFT_BOTTOM,
            format!("{}x{} @ {:.1}x | {} parts", canvas_size.0, canvas_size.1, state.zoom_level, parts_count),
            egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale)),
            egui::Color32::GRAY,
        );
    }

    // Draw drag indicator when dragging from gallery
    if let Some(ref gallery_drag) = state.gallery_drag {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            let drag_size = 48.0;
            let drag_rect = egui::Rect::from_center_size(
                pos,
                egui::vec2(drag_size, drag_size),
            );

            // Draw semi-transparent background
            painter.rect_filled(drag_rect, 4.0, egui::Color32::from_rgba_unmultiplied(60, 60, 80, 200));

            // Try to draw the thumbnail
            let texture_key = format!("gallery/{}/{}", gallery_drag.character_name, gallery_drag.part_name);
            if let Some(texture) = state.texture_cache.get(&texture_key) {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                painter.image(texture.id(), drag_rect.shrink(2.0), uv, egui::Color32::WHITE);
            } else {
                // Fallback: show part name
                painter.text(
                    drag_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &gallery_drag.part_name,
                    egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale)),
                    egui::Color32::WHITE,
                );
            }

            painter.rect_stroke(drag_rect, 4.0, egui::Stroke::new(2.0, egui::Color32::YELLOW));

            // Show "drop here" indicator if over canvas
            if canvas_rect.contains(pos) {
                let canvas_x = (pos.x - canvas_rect.min.x) / effective_zoom;
                let canvas_y = (pos.y - canvas_rect.min.y) / effective_zoom;
                let label = format!("({:.0}, {:.0})", canvas_x, canvas_y);
                painter.text(
                    pos + egui::vec2(drag_size / 2.0 + 5.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(scaled_font(12.0, state.config.ui_scale)),
                    egui::Color32::YELLOW,
                );
            }
        }
    }
}

fn render_dialogs(ctx: &egui::Context, state: &mut AppState) {
    // Rename dialog
    if state.show_rename_dialog {
        let title = match &state.context_menu_target {
            Some(ContextMenuTarget::Character { .. }) => "Rename Character",
            Some(ContextMenuTarget::Part { .. }) => "Rename Part",
            Some(ContextMenuTarget::Animation { .. }) => "Rename Animation",
            Some(ContextMenuTarget::Frame { .. }) | Some(ContextMenuTarget::Layer { .. }) | None => "Rename",
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
                                        if let Some(character) = project.characters.iter_mut().find(|c| c.name == char_name) {
                                            let old_name = character.name.clone();
                                            character.name = new_name.clone();
                                            // Update active character if it was renamed
                                            if state.active_character.as_ref() == Some(&old_name) {
                                                state.active_character = Some(new_name.clone());
                                            }
                                            // Update active tab if it was the character editor for this character
                                            if let ActiveTab::CharacterEditor(ref tab_name) = state.active_tab {
                                                if tab_name == &old_name {
                                                    state.active_tab = ActiveTab::CharacterEditor(new_name.clone());
                                                }
                                            }
                                            state.set_status(format!("Renamed character to '{}'", new_name));
                                        }
                                    }
                                }
                                ContextMenuTarget::Part { char_name, part_name } => {
                                    if let Some(ref mut project) = state.project {
                                        if let Some(character) = project.get_character_mut(&char_name) {
                                            if let Some(part) = character.parts.iter_mut().find(|p| p.name == part_name) {
                                                part.name = new_name.clone();
                                                state.editor_selected_part = Some(new_name.clone());
                                                state.set_status(format!("Renamed part to '{}'", new_name));
                                            }
                                        }
                                    }
                                }
                                ContextMenuTarget::Animation { char_name, anim_index, .. } => {
                                    if let Some(ref mut project) = state.project {
                                        if let Some(character) = project.get_character_mut(&char_name) {
                                            if let Some(anim) = character.animations.get_mut(anim_index) {
                                                anim.name = new_name.clone();
                                                state.set_status(format!("Renamed animation to '{}'", new_name));
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
            Some(ContextMenuTarget::Character { char_name }) => ("Delete Character?", "character", char_name.clone()),
            Some(ContextMenuTarget::Part { part_name, .. }) => ("Delete Part?", "part", part_name.clone()),
            Some(ContextMenuTarget::Animation { anim_name, .. }) => ("Delete Animation?", "animation", anim_name.clone()),
            Some(ContextMenuTarget::Frame { frame_index, .. }) => ("Delete Frame?", "frame", format!("{}", frame_index + 1)),
            Some(ContextMenuTarget::Layer { layer_name, .. }) => ("Delete Layer?", "layer", layer_name.clone()),
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
                                        if let ActiveTab::CharacterEditor(ref tab_name) = state.active_tab {
                                            if tab_name == &char_name {
                                                state.active_tab = ActiveTab::Canvas;
                                            }
                                        }
                                        state.set_status(format!("Deleted character '{}'", char_name));
                                    }
                                }
                                ContextMenuTarget::Part { char_name, part_name } => {
                                    if let Some(ref mut project) = state.project {
                                        if let Some(character) = project.get_character_mut(&char_name) {
                                            character.parts.retain(|p| p.name != part_name);
                                            if state.editor_selected_part.as_ref() == Some(&part_name) {
                                                state.editor_selected_part = None;
                                            }
                                            state.set_status(format!("Deleted part '{}'", part_name));
                                        }
                                    }
                                }
                                ContextMenuTarget::Animation { char_name, anim_index, anim_name } => {
                                    if let Some(ref mut project) = state.project {
                                        if let Some(character) = project.get_character_mut(&char_name) {
                                            if anim_index < character.animations.len() {
                                                character.animations.remove(anim_index);
                                                // Adjust current animation index
                                                if state.current_animation >= character.animations.len() && !character.animations.is_empty() {
                                                    state.current_animation = character.animations.len() - 1;
                                                }
                                                state.current_frame = 0;
                                                state.set_status(format!("Deleted animation '{}'", anim_name));
                                            }
                                        }
                                    }
                                }
                                ContextMenuTarget::Frame { char_name, anim_index, frame_index } => {
                                    if let Some(ref mut project) = state.project {
                                        if let Some(character) = project.get_character_mut(&char_name) {
                                            if let Some(anim) = character.animations.get_mut(anim_index) {
                                                if frame_index < anim.frames.len() && anim.frames.len() > 1 {
                                                    anim.frames.remove(frame_index);
                                                    // Adjust current frame index
                                                    if state.current_frame >= anim.frames.len() {
                                                        state.current_frame = anim.frames.len() - 1;
                                                    }
                                                    state.selected_part_id = None;
                                                    state.set_status(format!("Deleted frame {}", frame_index + 1));
                                                } else if anim.frames.len() == 1 {
                                                    state.set_status("Cannot delete the only frame");
                                                }
                                            }
                                        }
                                    }
                                }
                                ContextMenuTarget::Layer { layer_id, layer_name } => {
                                    let current_anim_idx = state.current_animation;
                                    let current_frame_idx = state.current_frame;
                                    if let Some(ref char_name) = state.active_character.clone() {
                                        if let Some(ref mut project) = state.project {
                                            if let Some(character) = project.get_character_mut(char_name) {
                                                if let Some(anim) = character.animations.get_mut(current_anim_idx) {
                                                    if let Some(frame) = anim.frames.get_mut(current_frame_idx) {
                                                        frame.placed_parts.retain(|p| p.id != layer_id);
                                                        if state.selected_part_id == Some(layer_id) {
                                                            state.selected_part_id = None;
                                                        }
                                                        state.set_status(format!("Deleted layer '{}'", layer_name));
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
                if let Some(duration) = state.time_since_save() {
                    let minutes = duration.as_secs() / 60;
                    if minutes > 0 {
                        ui.label(format!("Last saved {} minute{} ago.", minutes, if minutes == 1 { "" } else { "s" }));
                    } else {
                        ui.label("Last saved less than a minute ago.");
                    }
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
                                    if let Some(path) = pick_open_file() {
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
                                if let Some(path) = pick_open_file() {
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

    // Load dialog
    if state.show_load_dialog {
        egui::Window::new("Open Project")
            .collapsible(false)
            .resizable(false)
            .min_width(400.0)
            .show(ctx, |ui| {
                ui.label("Enter file path:");
                ui.text_edit_singleline(&mut state.file_path_input);

                ui.horizontal(|ui| {
                    if ui.button("Open").clicked() && !state.file_path_input.is_empty() {
                        let path = state.file_path_input.clone();
                        match state.load_project(&path) {
                            Ok(()) => {
                                state.set_status(format!("Loaded {}", path));
                            }
                            Err(e) => {
                                state.set_status(format!("Load failed: {}", e));
                            }
                        }
                        state.show_load_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_load_dialog = false;
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
                if let (Some(ref char_name), Some(ref part_name), Some(ref state_name)) =
                    (&state.selected_character_for_part, &state.selected_part_for_state, &state.selected_state_for_import)
                {
                    ui.label(format!("Importing to: {} / {} / {} @ {}°",
                        char_name, part_name, state_name, state.selected_rotation_for_import));
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
                                if let (Some(ref char_name), Some(ref part_name), Some(ref state_name)) =
                                    (&state.selected_character_for_part, &state.selected_part_for_state, &state.selected_state_for_import)
                                {
                                    if let Some(ref mut project) = state.project {
                                        if let Some(character) = project.get_character_mut(char_name) {
                                            if let Some(part) = character.get_part_mut(part_name) {
                                                if let Some(state_obj) = part.states.iter_mut().find(|s| s.name == *state_name) {
                                                    if let Some(rotation) = state_obj.rotations.get_mut(&rotation_angle) {
                                                        rotation.image_data = Some(base64_data);
                                                        state.set_status(format!("Image imported for {}° rotation", rotation_angle));
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
                            let character = Character::new(&state.new_character_name);
                            project.add_character(character);
                            state.active_character = Some(state.new_character_name.clone());
                            state.current_animation = 0;
                            state.current_frame = 0;
                            // Open the character editor for the new character
                            state.active_tab = ActiveTab::CharacterEditor(state.new_character_name.clone());
                            state.set_status(format!("Created character: {}", state.new_character_name));
                        }
                        state.show_new_character_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_new_character_dialog = false;
                    }
                });
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
                                            let new_state = State::new(&state.new_state_name, RotationMode::Deg45);
                                            part.add_state(new_state);
                                            state.editor_selected_state = Some(state.new_state_name.clone());
                                            state.set_status(format!("Created state: {}", state.new_state_name));
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
                let state_name = state.editor_selected_state.clone()
                    .or_else(|| Some("default".to_string()));

                match import_image_as_base64(path.to_str().unwrap_or("")) {
                    Ok(base64_data) => {
                        if let (Some(ref pn), Some(ref sn)) = (part_name, state_name) {
                            if let Some(ref mut project) = state.project {
                                if let Some(character) = project.get_character_mut(&char_name) {
                                    if let Some(part) = character.get_part_mut(pn) {
                                        if let Some(state_obj) = part.states.iter_mut().find(|s| &s.name == sn) {
                                            if let Some(rotation) = state_obj.rotations.get_mut(&angle) {
                                                rotation.image_data = Some(base64_data);
                                                state.set_status(format!("Imported image for {}°", angle));
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

/// Check if a pixel in a base64-encoded image is opaque (alpha > 0)
/// Returns true if the pixel is non-transparent, false if transparent or out of bounds
fn is_pixel_opaque(base64_data: &str, x: u32, y: u32) -> bool {
    use base64::Engine;

    // Decode base64
    let png_bytes = match base64::engine::general_purpose::STANDARD.decode(base64_data) {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };

    // Load image
    let img = match image::load_from_memory(&png_bytes) {
        Ok(img) => img.to_rgba8(),
        Err(_) => return false,
    };

    // Check bounds
    if x >= img.width() || y >= img.height() {
        return false;
    }

    // Check alpha channel (4th component of RGBA)
    let pixel = img.get_pixel(x, y);
    pixel[3] > 0
}

fn import_image_as_base64(path: &str) -> Result<String, String> {
    const MAX_TEXTURE_SIZE: u32 = 2048;

    let bytes = fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;

    // Verify it's a valid image
    let img = image::load_from_memory(&bytes).map_err(|e| format!("Invalid image: {}", e))?;

    // Resize if too large
    let (width, height) = (img.width(), img.height());
    let img = if width > MAX_TEXTURE_SIZE || height > MAX_TEXTURE_SIZE {
        let scale = (MAX_TEXTURE_SIZE as f32 / width as f32)
            .min(MAX_TEXTURE_SIZE as f32 / height as f32);
        let new_width = (width as f32 * scale) as u32;
        let new_height = (height as f32 * scale) as u32;
        // Note: Image will be resized to fit within 2048x2048
        img.resize(new_width, new_height, image::imageops::FilterType::Nearest)
    } else {
        img
    };

    // Re-encode as PNG to ensure consistent format
    let mut png_bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_bytes);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &png_bytes))
}

fn decode_base64_to_texture(
    ctx: &egui::Context,
    name: &str,
    base64_data: &str,
) -> Result<egui::TextureHandle, String> {
    use base64::Engine;

    const MAX_TEXTURE_SIZE: u32 = 2048;

    // Decode base64
    let png_bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    // Load image
    let img = image::load_from_memory(&png_bytes)
        .map_err(|e| format!("Failed to load image: {}", e))?;

    // Check if image needs to be resized
    let (width, height) = (img.width(), img.height());
    let img = if width > MAX_TEXTURE_SIZE || height > MAX_TEXTURE_SIZE {
        // Calculate new size maintaining aspect ratio
        let scale = (MAX_TEXTURE_SIZE as f32 / width as f32)
            .min(MAX_TEXTURE_SIZE as f32 / height as f32);
        let new_width = (width as f32 * scale) as u32;
        let new_height = (height as f32 * scale) as u32;
        img.resize(new_width, new_height, image::imageops::FilterType::Nearest)
    } else {
        img
    };

    // Convert to RGBA
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let pixels = rgba.into_raw();

    // Create egui ColorImage
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);

    // Create texture
    Ok(ctx.load_texture(
        name,
        color_image,
        egui::TextureOptions::NEAREST, // Pixel art should use nearest neighbor
    ))
}

/// Render a single frame to an RGBA image buffer
fn render_frame_to_image(
    project: &Project,
    animation: &model::Animation,
    frame_idx: usize,
) -> Result<image::RgbaImage, String> {
    let frame = animation.frames.get(frame_idx)
        .ok_or_else(|| format!("Frame {} not found", frame_idx))?;

    let (canvas_w, canvas_h) = project.canvas_size;
    let mut canvas = image::RgbaImage::new(canvas_w, canvas_h);

    // Draw each placed part (in order - later parts on top)
    for placed in &frame.placed_parts {
        // Find the part's image data
        let image_data = project.get_character(&placed.character_name)
            .and_then(|c| c.get_part(&placed.part_name))
            .and_then(|p| p.states.iter().find(|s| s.name == placed.state_name))
            .and_then(|s| s.rotations.get(&placed.rotation))
            .and_then(|r| r.image_data.as_ref());

        if let Some(base64_data) = image_data {
            // Decode the image
            use base64::Engine;
            let png_bytes = base64::engine::general_purpose::STANDARD
                .decode(base64_data)
                .map_err(|e| format!("Base64 decode error: {}", e))?;

            let part_img = image::load_from_memory(&png_bytes)
                .map_err(|e| format!("Image load error: {}", e))?
                .to_rgba8();

            // Composite onto canvas at the specified position
            let x = placed.position.0.round() as i32;
            let y = placed.position.1.round() as i32;

            for (px, py, pixel) in part_img.enumerate_pixels() {
                let dest_x = x + px as i32;
                let dest_y = y + py as i32;

                // Bounds check
                if dest_x >= 0 && dest_x < canvas_w as i32 && dest_y >= 0 && dest_y < canvas_h as i32 {
                    let dest_x = dest_x as u32;
                    let dest_y = dest_y as u32;

                    // Alpha blending
                    let src = pixel;
                    let dst = canvas.get_pixel_mut(dest_x, dest_y);

                    let src_a = src[3] as f32 / 255.0;
                    let dst_a = dst[3] as f32 / 255.0;
                    let out_a = src_a + dst_a * (1.0 - src_a);

                    if out_a > 0.0 {
                        for i in 0..3 {
                            let src_c = src[i] as f32 / 255.0;
                            let dst_c = dst[i] as f32 / 255.0;
                            let out_c = (src_c * src_a + dst_c * dst_a * (1.0 - src_a)) / out_a;
                            dst[i] = (out_c * 255.0).round() as u8;
                        }
                        dst[3] = (out_a * 255.0).round() as u8;
                    }
                }
            }
        }
    }

    Ok(canvas)
}

/// Export the current animation as a spritesheet
fn export_current_animation(state: &AppState, output_path: &str) -> Result<(String, String), String> {
    let project = state.project.as_ref().ok_or("No project loaded")?;
    let char_name = state.active_character.as_ref().ok_or("No character selected")?;
    let character = project.get_character(char_name).ok_or("Character not found")?;
    let animation = character.animations.get(state.current_animation)
        .ok_or("Animation not found")?;

    if animation.frames.is_empty() {
        return Err("Animation has no frames".to_string());
    }

    let (canvas_w, canvas_h) = project.canvas_size;
    let frame_count = animation.frames.len();

    // Calculate spritesheet dimensions (horizontal strip for small counts, grid for larger)
    let (cols, rows) = if frame_count <= 8 {
        (frame_count, 1)
    } else {
        let cols = (frame_count as f32).sqrt().ceil() as usize;
        let rows = (frame_count + cols - 1) / cols;
        (cols, rows)
    };

    let sheet_w = cols as u32 * canvas_w;
    let sheet_h = rows as u32 * canvas_h;
    let mut spritesheet = image::RgbaImage::new(sheet_w, sheet_h);

    // Render each frame and place it in the spritesheet
    let mut frame_metadata = Vec::new();
    for (i, frame) in animation.frames.iter().enumerate() {
        let frame_img = render_frame_to_image(project, animation, i)?;

        let col = i % cols;
        let row = i / cols;
        let x = col as u32 * canvas_w;
        let y = row as u32 * canvas_h;

        // Copy frame to spritesheet
        for (px, py, pixel) in frame_img.enumerate_pixels() {
            spritesheet.put_pixel(x + px, y + py, *pixel);
        }

        frame_metadata.push(serde_json::json!({
            "x": x,
            "y": y,
            "width": canvas_w,
            "height": canvas_h,
            "duration_ms": frame.duration_ms
        }));
    }

    // Ensure output path ends with .png
    let png_path = if output_path.to_lowercase().ends_with(".png") {
        output_path.to_string()
    } else {
        format!("{}.png", output_path)
    };

    // Save spritesheet
    spritesheet.save(&png_path)
        .map_err(|e| format!("Failed to save spritesheet: {}", e))?;

    // Create metadata JSON
    let json_path = png_path.replace(".png", ".json");
    let metadata = serde_json::json!({
        "sprite_sheet": PathBuf::from(&png_path).file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| png_path.clone()),
        "animation": animation.name,
        "frame_width": canvas_w,
        "frame_height": canvas_h,
        "columns": cols,
        "rows": rows,
        "frames": frame_metadata
    });

    let json_str = serde_json::to_string_pretty(&metadata)
        .map_err(|e| format!("Failed to serialize metadata: {}", e))?;
    fs::write(&json_path, json_str)
        .map_err(|e| format!("Failed to save metadata: {}", e))?;

    Ok((png_path, json_path))
}

/// Export all animations for the current character
fn export_all_animations(state: &AppState, output_dir: &str) -> Result<usize, String> {
    let project = state.project.as_ref().ok_or("No project loaded")?;
    let char_name = state.active_character.as_ref().ok_or("No character selected")?;
    let character = project.get_character(char_name).ok_or("Character not found")?;

    // Create output directory if needed
    fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    let mut exported_count = 0;
    let (canvas_w, canvas_h) = project.canvas_size;

    for animation in &character.animations {
        if animation.frames.is_empty() {
            continue;
        }

        let frame_count = animation.frames.len();

        // Calculate spritesheet dimensions
        let (cols, rows) = if frame_count <= 8 {
            (frame_count, 1)
        } else {
            let cols = (frame_count as f32).sqrt().ceil() as usize;
            let rows = (frame_count + cols - 1) / cols;
            (cols, rows)
        };

        let sheet_w = cols as u32 * canvas_w;
        let sheet_h = rows as u32 * canvas_h;
        let mut spritesheet = image::RgbaImage::new(sheet_w, sheet_h);

        // Render each frame
        let mut frame_metadata = Vec::new();
        for (i, frame) in animation.frames.iter().enumerate() {
            let frame_img = render_frame_to_image(project, animation, i)?;

            let col = i % cols;
            let row = i / cols;
            let x = col as u32 * canvas_w;
            let y = row as u32 * canvas_h;

            for (px, py, pixel) in frame_img.enumerate_pixels() {
                spritesheet.put_pixel(x + px, y + py, *pixel);
            }

            frame_metadata.push(serde_json::json!({
                "x": x,
                "y": y,
                "width": canvas_w,
                "height": canvas_h,
                "duration_ms": frame.duration_ms
            }));
        }

        // Sanitize animation name for filename
        let safe_name: String = animation.name.chars()
            .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
            .collect();
        let png_path = format!("{}/{}_{}.png", output_dir, char_name, safe_name);

        // Save spritesheet
        spritesheet.save(&png_path)
            .map_err(|e| format!("Failed to save {}: {}", png_path, e))?;

        // Create metadata JSON
        let json_path = png_path.replace(".png", ".json");
        let metadata = serde_json::json!({
            "sprite_sheet": PathBuf::from(&png_path).file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| png_path.clone()),
            "character": char_name,
            "animation": animation.name,
            "frame_width": canvas_w,
            "frame_height": canvas_h,
            "columns": cols,
            "rows": rows,
            "frames": frame_metadata
        });

        let json_str = serde_json::to_string_pretty(&metadata)
            .map_err(|e| format!("Failed to serialize metadata: {}", e))?;
        fs::write(&json_path, json_str)
            .map_err(|e| format!("Failed to save {}: {}", json_path, e))?;

        exported_count += 1;
    }

    Ok(exported_count)
}
