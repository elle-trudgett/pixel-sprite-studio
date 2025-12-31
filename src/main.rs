mod model;

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use model::{Animation, Character, Part, Project, State, RotationMode, PlacedPart};
use std::path::PathBuf;
use std::fs;
use std::collections::HashMap;

#[cfg(target_os = "windows")]
use rfd::FileDialog;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sprite Animator".into(),
                resolution: (1920., 1080.).into(),
                resizable: true,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin)
        .init_resource::<AppState>()
        .add_systems(Startup, setup)
        .add_systems(Update, ui_system)
        .run();
}

#[derive(Resource, Default)]
struct AppState {
    project: Option<Project>,
    project_path: Option<PathBuf>,

    // UI state
    show_grid: bool,
    zoom_level: f32,
    current_animation: usize,
    current_frame: usize,
    is_playing: bool,
    selected_part_id: Option<u64>,
    pixel_aligned: bool,

    // Dragging state
    dragging_part: Option<DraggedPart>,
    drag_offset: (f32, f32),
    drag_accumulator: (f32, f32), // Accumulates true position during pixel-aligned drag

    // Dialogs
    show_new_character_dialog: bool,
    show_new_part_dialog: bool,
    show_new_state_dialog: bool,
    show_new_animation_dialog: bool,
    show_save_dialog: bool,
    show_load_dialog: bool,
    show_import_image_dialog: bool,

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

impl AppState {
    fn new() -> Self {
        Self {
            project: None,
            project_path: None,
            show_grid: true,
            zoom_level: 4.0,
            current_animation: 0,
            current_frame: 0,
            is_playing: false,
            selected_part_id: None,
            pixel_aligned: true,
            dragging_part: None,
            drag_offset: (0.0, 0.0),
            drag_accumulator: (0.0, 0.0),
            show_new_character_dialog: false,
            show_new_part_dialog: false,
            show_new_state_dialog: false,
            show_new_animation_dialog: false,
            show_save_dialog: false,
            show_load_dialog: false,
            show_import_image_dialog: false,
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
        if let Some(ref mut project) = self.project {
            let id = project.next_id();
            let placed = PlacedPart::new(id, character, part, state);
            let mut placed = placed;
            placed.position = (x, y);

            if let Some(anim) = project.animations.get_mut(self.current_animation) {
                if let Some(frame) = anim.frames.get_mut(self.current_frame) {
                    frame.placed_parts.push(placed);
                    self.selected_part_id = Some(id);
                }
            }
        }
    }

    fn get_selected_placed_part(&self) -> Option<&PlacedPart> {
        let id = self.selected_part_id?;
        let project = self.project.as_ref()?;
        let anim = project.animations.get(self.current_animation)?;
        let frame = anim.frames.get(self.current_frame)?;
        frame.placed_parts.iter().find(|p| p.id == id)
    }

    fn get_selected_placed_part_mut(&mut self) -> Option<&mut PlacedPart> {
        let id = self.selected_part_id?;
        let project = self.project.as_mut()?;
        let anim = project.animations.get_mut(self.current_animation)?;
        let frame = anim.frames.get_mut(self.current_frame)?;
        frame.placed_parts.iter_mut().find(|p| p.id == id)
    }

    fn delete_selected_part(&mut self) {
        if let Some(id) = self.selected_part_id {
            if let Some(ref mut project) = self.project {
                if let Some(anim) = project.animations.get_mut(self.current_animation) {
                    if let Some(frame) = anim.frames.get_mut(self.current_frame) {
                        frame.placed_parts.retain(|p| p.id != id);
                    }
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

        Ok(())
    }

    fn save_project_as(&mut self, path: &str) -> Result<(), String> {
        self.project_path = Some(PathBuf::from(path));
        self.save_project()
    }

    fn load_project(&mut self, path: &str) -> Result<(), String> {
        let json = fs::read_to_string(path).map_err(|e| format!("Read error: {}", e))?;
        let project = Project::from_json(&json).map_err(|e| format!("Parse error: {}", e))?;

        self.project = Some(project);
        self.project_path = Some(PathBuf::from(path));
        self.current_animation = 0;
        self.current_frame = 0;
        self.selected_part_id = None;

        Ok(())
    }

    fn new_project(&mut self) {
        self.project = Some(Project::new("Untitled"));
        self.project_path = None;
        self.current_animation = 0;
        self.current_frame = 0;
        self.selected_part_id = None;
    }

    fn current_animation(&self) -> Option<&Animation> {
        self.project.as_ref()?.animations.get(self.current_animation)
    }

    fn current_animation_mut(&mut self) -> Option<&mut Animation> {
        self.project.as_mut()?.animations.get_mut(self.current_animation)
    }

    fn total_frames(&self) -> usize {
        self.current_animation().map(|a| a.frames.len()).unwrap_or(1)
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

// Fallback for non-Windows (returns None, uses text input instead)
#[cfg(not(target_os = "windows"))]
fn pick_save_file() -> Option<PathBuf> { None }
#[cfg(not(target_os = "windows"))]
fn pick_open_file() -> Option<PathBuf> { None }
#[cfg(not(target_os = "windows"))]
fn pick_image_file() -> Option<PathBuf> { None }

fn setup(mut commands: Commands, mut state: ResMut<AppState>) {
    commands.spawn(Camera2d);
    *state = AppState::new();
}

fn ui_system(mut contexts: EguiContexts, mut state: ResMut<AppState>) {
    let ctx = contexts.ctx_mut();

    // Dialogs (rendered first so they appear on top)
    render_dialogs(ctx, &mut state);

    // Menu bar
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Project").clicked() {
                    state.new_project();
                    state.set_status("Created new project");
                    ui.close_menu();
                }
                if ui.button("Open...").clicked() {
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
                    // Try native file dialog first
                    if let Some(path) = pick_save_file() {
                        let path_str = path.to_string_lossy().to_string();
                        match state.save_project_as(&path_str) {
                            Ok(()) => state.set_status(format!("Saved to {}", path_str)),
                            Err(e) => state.set_status(format!("Save failed: {}", e)),
                        }
                    } else {
                        // Fallback to text input dialog
                        state.show_save_dialog = true;
                        state.file_path_input = state.project_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| "project.sprite-animator.json".to_string());
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    std::process::exit(0);
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
                ui.checkbox(&mut state.show_grid, "Show Grid");
                ui.separator();
                if ui.button("Zoom In").clicked() {
                    state.zoom_level = (state.zoom_level * 1.5).min(16.0);
                    ui.close_menu();
                }
                if ui.button("Zoom Out").clicked() {
                    state.zoom_level = (state.zoom_level / 1.5).max(0.5);
                    ui.close_menu();
                }
                if ui.button("Reset Zoom (4x)").clicked() {
                    state.zoom_level = 4.0;
                    ui.close_menu();
                }
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
                if ui.add_enabled(has_project, egui::Button::new("Export Current Animation...")).clicked() {
                    ui.close_menu();
                }
                if ui.add_enabled(has_project, egui::Button::new("Export All...")).clicked() {
                    ui.close_menu();
                }
            });
        });

        // Status message in menu bar
        if let Some((ref msg, _)) = state.status_message {
            ui.separator();
            ui.label(msg);
        }
    });

    // Asset browser (left panel)
    egui::SidePanel::left("asset_browser")
        .default_width(250.0)
        .min_width(200.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("Asset Browser");
            ui.separator();

            if let Some(ref project) = state.project.clone() {
                // Characters tree
                egui::CollapsingHeader::new("Characters")
                    .default_open(true)
                    .show(ui, |ui| {
                        if project.characters.is_empty() {
                            ui.label("  (No characters)");
                        }
                        for character in &project.characters {
                            let char_name = character.name.clone();
                            egui::CollapsingHeader::new(&character.name)
                                .default_open(true)
                                .show(ui, |ui| {
                                    for part in &character.parts {
                                        let part_name = part.name.clone();
                                        ui.horizontal(|ui| {
                                            ui.label(format!("  {}", &part.name));
                                            if ui.small_button("Place").clicked() {
                                                // Place part at center of canvas
                                                let center_x = project.canvas_size.0 as f32 / 2.0 - 8.0;
                                                let center_y = project.canvas_size.1 as f32 / 2.0 - 8.0;
                                                let state_name = part.states.first()
                                                    .map(|s| s.name.clone())
                                                    .unwrap_or_else(|| "default".to_string());
                                                state.place_part_on_canvas(&char_name, &part_name, &state_name, center_x, center_y);
                                                state.set_status(format!("Placed {}", part_name));
                                            }
                                        });
                                        egui::CollapsingHeader::new(format!("    States"))
                                            .id_salt(format!("{}-{}-states", char_name, part_name))
                                            .default_open(false)
                                            .show(ui, |ui| {
                                                for state_item in &part.states {
                                                    let state_name = state_item.name.clone();
                                                    let has_images = state_item.has_images();
                                                    egui::CollapsingHeader::new(format!("      {} {}", &state_item.name, if has_images { "✓" } else { "" }))
                                                        .id_salt(format!("{}-{}-{}-rotations", char_name, part_name, state_name))
                                                        .default_open(false)
                                                        .show(ui, |ui| {
                                                            // Show all rotation angles
                                                            for angle in state_item.rotation_mode.angles() {
                                                                let has_image = state_item.rotations.get(&angle)
                                                                    .map(|r| r.image_data.is_some())
                                                                    .unwrap_or(false);
                                                                ui.horizontal(|ui| {
                                                                    ui.label(format!("        {}°", angle));
                                                                    if has_image {
                                                                        ui.label("✓");
                                                                    }
                                                                    if ui.small_button("Import").clicked() {
                                                                        // Try native file dialog first
                                                                        if let Some(path) = pick_image_file() {
                                                                            let path_str = path.to_string_lossy().to_string();
                                                                            match import_image_as_base64(&path_str) {
                                                                                Ok(base64_data) => {
                                                                                    if let Some(ref mut project) = state.project {
                                                                                        if let Some(character) = project.get_character_mut(&char_name) {
                                                                                            if let Some(part) = character.get_part_mut(&part_name) {
                                                                                                if let Some(state_obj) = part.states.iter_mut().find(|s| s.name == state_name) {
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
                                                                                Err(e) => state.set_status(format!("Import failed: {}", e)),
                                                                            }
                                                                        } else {
                                                                            // Fallback to text input dialog
                                                                            state.show_import_image_dialog = true;
                                                                            state.import_image_path.clear();
                                                                            state.selected_character_for_part = Some(char_name.clone());
                                                                            state.selected_part_for_state = Some(part_name.clone());
                                                                            state.selected_state_for_import = Some(state_name.clone());
                                                                            state.selected_rotation_for_import = angle;
                                                                        }
                                                                    }
                                                                });
                                                            }
                                                        });
                                                }
                                                if ui.small_button("+ State").clicked() {
                                                    state.show_new_state_dialog = true;
                                                    state.new_state_name.clear();
                                                    state.selected_character_for_part = Some(char_name.clone());
                                                    state.selected_part_for_state = Some(part_name.clone());
                                                }
                                            });
                                    }
                                    if ui.small_button("+ Part").clicked() {
                                        state.show_new_part_dialog = true;
                                        state.new_part_name.clear();
                                        state.selected_character_for_part = Some(char_name.clone());
                                    }
                                });
                        }
                        ui.separator();
                        if ui.button("+ New Character").clicked() {
                            state.show_new_character_dialog = true;
                            state.new_character_name.clear();
                        }
                    });

                ui.separator();

                // Animations list
                egui::CollapsingHeader::new("Animations")
                    .default_open(true)
                    .show(ui, |ui| {
                        for (i, anim) in project.animations.iter().enumerate() {
                            let selected = i == state.current_animation;
                            if ui.selectable_label(selected, &anim.name).clicked() {
                                state.current_animation = i;
                                state.current_frame = 0;
                            }
                        }
                        ui.separator();
                        if ui.button("+ New Animation").clicked() {
                            state.show_new_animation_dialog = true;
                            state.new_animation_name.clear();
                        }
                    });
            } else {
                ui.label("No project loaded");
                ui.label("");
                if ui.button("New Project").clicked() {
                    state.new_project();
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

            if let Some((character_name, part_name, current_state, position, rotation, z_override)) = selected_info {
                ui.label(format!("Selected: {}", part_name));
                ui.label(format!("Character: {}", character_name));
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
                    ui.checkbox(&mut state.pixel_aligned, "Pixel aligned");
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

                // Z-Index override
                let mut z = z_override.unwrap_or(0);
                ui.horizontal(|ui| {
                    ui.label("Z-Index:");
                    if ui.add(egui::DragValue::new(&mut z).speed(1)).changed() {
                        if let Some(part) = state.get_selected_placed_part_mut() {
                            part.z_override = Some(z);
                        }
                    }
                });

                ui.separator();
                if ui.button("Delete Part").clicked() {
                    state.delete_selected_part();
                    state.set_status("Part deleted");
                }
            } else {
                ui.label("No selection");
                ui.label("");
                ui.label("Click 'Place' next to a part");
                ui.label("in the asset browser, then");
                ui.label("click and drag on the canvas.");
            }

            ui.separator();

            // Canvas Settings - zoom is separate from project
            ui.collapsing("Canvas Settings", |ui| {
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
                    ui.add(egui::Slider::new(&mut state.zoom_level, 0.5..=16.0).logarithmic(true));
                });
            });

            if let Some(ref mut project) = state.project {
                ui.collapsing("Reference Layer", |ui| {
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
                });
            }
        });

    // Timeline (bottom panel)
    let total_frames = state.total_frames();
    egui::TopBottomPanel::bottom("timeline")
        .default_height(180.0)
        .min_height(100.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Timeline");
                ui.separator();

                // Animation selector - collect names first to avoid borrow issues
                let anim_names: Vec<String> = state.project
                    .as_ref()
                    .map(|p| p.animations.iter().map(|a| a.name.clone()).collect())
                    .unwrap_or_default();
                let current_name = anim_names.get(state.current_animation)
                    .map(|s| s.as_str())
                    .unwrap_or("None");

                egui::ComboBox::from_id_salt("anim_select")
                    .selected_text(current_name)
                    .show_ui(ui, |ui| {
                        for (i, name) in anim_names.iter().enumerate() {
                            if ui.selectable_value(&mut state.current_animation, i, name).clicked() {
                                state.current_frame = 0;
                            }
                        }
                    });

                ui.separator();

                // Playback controls
                let play_text = if state.is_playing { "⏸" } else { "▶" };
                if ui.button(play_text).clicked() {
                    state.is_playing = !state.is_playing;
                }
                if ui.button("⏹").clicked() {
                    state.is_playing = false;
                    state.current_frame = 0;
                }
                if ui.button("⏮").clicked() && state.current_frame > 0 {
                    state.current_frame -= 1;
                }
                if ui.button("⏭").clicked() && state.current_frame < total_frames - 1 {
                    state.current_frame += 1;
                }

                ui.separator();
                ui.label(format!("Frame: {} / {}", state.current_frame + 1, total_frames));

                ui.separator();
                if ui.button("+ Frame").clicked() {
                    if let Some(anim) = state.current_animation_mut() {
                        anim.add_frame();
                    }
                }
            });

            ui.separator();

            // Frame buttons
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    for frame in 0..total_frames {
                        let is_current = frame == state.current_frame;
                        let text = format!("{}", frame + 1);

                        let button = if is_current {
                            egui::Button::new(egui::RichText::new(text).strong())
                                .fill(egui::Color32::from_rgb(80, 120, 180))
                        } else {
                            egui::Button::new(text)
                        };

                        if ui.add_sized([40.0, 60.0], button).clicked() {
                            state.current_frame = frame;
                        }
                    }
                });
            });

            ui.separator();
            ui.label("  Tracks: (drag parts here to animate)");
        });

    // Central canvas area
    egui::CentralPanel::default().show(ctx, |ui| {
        if state.project.is_none() {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Welcome to Sprite Animator");
                    ui.label("");
                    ui.label("Create or open a project to begin.");
                    ui.label("");
                    if ui.button("New Project").clicked() {
                        state.new_project();
                    }
                });
            });
        } else {
            render_canvas(ui, &mut state);
        }
    });
}

// Info needed for rendering a placed part
struct PlacedPartRenderInfo {
    id: u64,
    part_name: String,
    character_name: String,
    state_name: String,
    rotation: u16,
    position: (f32, f32),
    image_data: Option<String>,
}

fn render_canvas(ui: &mut egui::Ui, state: &mut AppState) {
    // Capture values from project upfront to avoid borrow conflicts
    let (canvas_size, placed_parts) = {
        let Some(ref project) = state.project else { return };

        let parts: Vec<PlacedPartRenderInfo> =
            if let Some(anim) = project.animations.get(state.current_animation) {
                if let Some(frame) = anim.frames.get(state.current_frame) {
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
                                character_name: p.character_name.clone(),
                                state_name: p.state_name.clone(),
                                rotation: p.rotation,
                                position: p.position,
                                image_data,
                            }
                        })
                        .collect()
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

        (project.canvas_size, parts)
    };

    let available = ui.available_size();
    let canvas_w = canvas_size.0 as f32 * state.zoom_level;
    let canvas_h = canvas_size.1 as f32 * state.zoom_level;

    // Center the canvas
    let offset_x = (available.x - canvas_w) / 2.0;
    let offset_y = (available.y - canvas_h) / 2.0;

    let (response, painter) = ui.allocate_painter(available, egui::Sense::click_and_drag());
    let canvas_rect = egui::Rect::from_min_size(
        response.rect.min + egui::vec2(offset_x.max(0.0), offset_y.max(0.0)),
        egui::vec2(canvas_w, canvas_h),
    );

    // Draw canvas background
    painter.rect_filled(canvas_rect, 0.0, egui::Color32::from_rgb(40, 40, 50));

    // Draw grid if enabled
    if state.show_grid {
        let grid_color = egui::Color32::from_rgba_unmultiplied(100, 100, 100, 60);
        let cell_size = state.zoom_level;

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

    // Draw placed parts
    for part_info in &placed_parts {
        let screen_x = canvas_rect.min.x + part_info.position.0 * state.zoom_level;
        let screen_y = canvas_rect.min.y + part_info.position.1 * state.zoom_level;

        let is_selected = state.selected_part_id == Some(part_info.id);

        // Try to get or create texture for this part
        let texture_key = format!("{}/{}/{}/{}",
            part_info.character_name, part_info.part_name, part_info.state_name, part_info.rotation);

        let mut rendered_texture = false;
        let mut image_size = (16.0_f32, 16.0_f32);

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
                let scaled_size = egui::vec2(tex_size.x * state.zoom_level, tex_size.y * state.zoom_level);
                let part_rect = egui::Rect::from_min_size(
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

                // Draw selection border
                if is_selected {
                    painter.rect_stroke(part_rect, 0.0, egui::Stroke::new(2.0, egui::Color32::YELLOW));
                }

                rendered_texture = true;
            }
        }

        // Fallback: draw colored rectangle if no texture
        if !rendered_texture {
            let part_size_x = image_size.0 * state.zoom_level;
            let part_size_y = image_size.1 * state.zoom_level;
            let part_rect = egui::Rect::from_min_size(
                egui::pos2(screen_x, screen_y),
                egui::vec2(part_size_x, part_size_y),
            );

            // Color based on part name hash for variety
            let hash = part_info.part_name.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32));
            let r = ((hash * 17) % 200 + 55) as u8;
            let g = ((hash * 31) % 200 + 55) as u8;
            let b = ((hash * 47) % 200 + 55) as u8;

            let fill_color = egui::Color32::from_rgba_unmultiplied(r, g, b, 180);
            let stroke_color = if is_selected {
                egui::Color32::YELLOW
            } else {
                egui::Color32::WHITE
            };
            let stroke_width = if is_selected { 3.0 } else { 1.0 };

            painter.rect_filled(part_rect, 2.0, fill_color);
            painter.rect_stroke(part_rect, 2.0, egui::Stroke::new(stroke_width, stroke_color));

            // Draw part name
            painter.text(
                part_rect.center(),
                egui::Align2::CENTER_CENTER,
                &part_info.part_name,
                egui::FontId::proportional(10.0),
                egui::Color32::WHITE,
            );
        }
    }

    // Handle mouse interactions
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            // Check if we clicked on a part
            let mut clicked_part = None;
            for part_info in placed_parts.iter().rev() { // Reverse for top-to-bottom
                let screen_x = canvas_rect.min.x + part_info.position.0 * state.zoom_level;
                let screen_y = canvas_rect.min.y + part_info.position.1 * state.zoom_level;
                // Use cached texture size if available, otherwise default 16x16
                let part_size = if let Some(texture) = state.texture_cache.get(&format!(
                    "{}/{}/{}/{}", part_info.character_name, part_info.part_name,
                    part_info.state_name, part_info.rotation
                )) {
                    texture.size_vec2() * state.zoom_level
                } else {
                    egui::vec2(16.0, 16.0) * state.zoom_level
                };
                let part_rect = egui::Rect::from_min_size(
                    egui::pos2(screen_x, screen_y),
                    part_size,
                );
                if part_rect.contains(pos) {
                    clicked_part = Some(part_info.id);
                    break;
                }
            }
            state.selected_part_id = clicked_part;
        }
    }

    // Handle dragging selected part
    if response.drag_started() && state.selected_part_id.is_some() {
        // Initialize accumulator with current position when drag starts
        if let Some(part) = state.get_selected_placed_part() {
            state.drag_accumulator = part.position;
        }
    }

    if response.dragged() && state.selected_part_id.is_some() {
        let delta = response.drag_delta();
        let zoom = state.zoom_level;
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

    // Canvas border
    painter.rect_stroke(
        canvas_rect,
        0.0,
        egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 100, 120)),
    );

    // Canvas info overlay
    let parts_count = placed_parts.len();
    ui.put(
        egui::Rect::from_min_size(
            response.rect.min + egui::vec2(10.0, 10.0),
            egui::vec2(250.0, 20.0),
        ),
        egui::Label::new(format!(
            "Canvas: {}x{} @ {:.1}x | {} parts",
            canvas_size.0, canvas_size.1, state.zoom_level, parts_count
        )),
    );
}

fn render_dialogs(ctx: &egui::Context, state: &mut AppState) {
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
                        }
                        state.show_new_character_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_new_character_dialog = false;
                    }
                });
            });
    }

    // New Part dialog
    if state.show_new_part_dialog {
        egui::Window::new("New Part")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                // Character selector
                let characters: Vec<String> = state.project
                    .as_ref()
                    .map(|p| p.characters.iter().map(|c| c.name.clone()).collect())
                    .unwrap_or_default();

                if let Some(ref mut selected) = state.selected_character_for_part {
                    ui.horizontal(|ui| {
                        ui.label("Character:");
                        egui::ComboBox::from_id_salt("part_char_select")
                            .selected_text(selected.as_str())
                            .show_ui(ui, |ui| {
                                for name in &characters {
                                    ui.selectable_value(selected, name.clone(), name);
                                }
                            });
                    });
                } else if let Some(first) = characters.first() {
                    state.selected_character_for_part = Some(first.clone());
                }

                ui.horizontal(|ui| {
                    ui.label("Part Name:");
                    ui.text_edit_singleline(&mut state.new_part_name);
                });

                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() && !state.new_part_name.is_empty() {
                        if let Some(ref char_name) = state.selected_character_for_part {
                            if let Some(ref mut project) = state.project {
                                if let Some(character) = project.get_character_mut(char_name) {
                                    character.add_part(Part::new(&state.new_part_name));
                                }
                            }
                        }
                        state.show_new_part_dialog = false;
                        state.selected_character_for_part = None;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_new_part_dialog = false;
                        state.selected_character_for_part = None;
                    }
                });
            });
    }

    // New State dialog
    if state.show_new_state_dialog {
        egui::Window::new("New State")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("State Name:");
                    ui.text_edit_singleline(&mut state.new_state_name);
                });

                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() && !state.new_state_name.is_empty() {
                        if let (Some(ref char_name), Some(ref part_name)) =
                            (&state.selected_character_for_part, &state.selected_part_for_state)
                        {
                            if let Some(ref mut project) = state.project {
                                if let Some(character) = project.get_character_mut(char_name) {
                                    if let Some(part) = character.get_part_mut(part_name) {
                                        part.add_state(State::new(&state.new_state_name, RotationMode::Deg45));
                                    }
                                }
                            }
                        }
                        state.show_new_state_dialog = false;
                        state.selected_character_for_part = None;
                        state.selected_part_for_state = None;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_new_state_dialog = false;
                        state.selected_character_for_part = None;
                        state.selected_part_for_state = None;
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
                        if let Some(ref mut project) = state.project {
                            let animation = Animation::new(&state.new_animation_name);
                            project.add_animation(animation);
                            state.current_animation = project.animations.len() - 1;
                            state.current_frame = 0;
                        }
                        state.show_new_animation_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_new_animation_dialog = false;
                    }
                });
            });
    }

    // Save dialog
    if state.show_save_dialog {
        egui::Window::new("Save Project")
            .collapsible(false)
            .resizable(false)
            .min_width(400.0)
            .show(ctx, |ui| {
                ui.label("Enter file path:");
                ui.text_edit_singleline(&mut state.file_path_input);
                ui.label("(Use .sprite-animator.json extension)");

                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() && !state.file_path_input.is_empty() {
                        let path = state.file_path_input.clone();
                        match state.save_project_as(&path) {
                            Ok(()) => {
                                state.set_status(format!("Saved to {}", path));
                            }
                            Err(e) => {
                                state.set_status(format!("Save failed: {}", e));
                            }
                        }
                        state.show_save_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        state.show_save_dialog = false;
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
}

fn import_image_as_base64(path: &str) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;

    // Verify it's a valid PNG
    let img = image::load_from_memory(&bytes).map_err(|e| format!("Invalid image: {}", e))?;

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

    // Decode base64
    let png_bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e| format!("Failed to decode base64: {}", e))?;

    // Load image
    let img = image::load_from_memory(&png_bytes)
        .map_err(|e| format!("Failed to load image: {}", e))?;

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
