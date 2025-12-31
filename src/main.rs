use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

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
    project_name: Option<String>,
    show_grid: bool,
    show_reference: bool,
    canvas_size: (u32, u32),
    current_frame: usize,
    total_frames: usize,
    is_playing: bool,
    selected_part: Option<String>,
    zoom_level: f32,
}

impl AppState {
    fn new() -> Self {
        Self {
            project_name: None,
            show_grid: true,
            show_reference: true,
            canvas_size: (64, 64),
            current_frame: 0,
            total_frames: 1,
            is_playing: false,
            selected_part: None,
            zoom_level: 4.0,
        }
    }
}

fn setup(mut commands: Commands, mut state: ResMut<AppState>) {
    commands.spawn(Camera2d);
    *state = AppState::new();
}

fn ui_system(mut contexts: EguiContexts, mut state: ResMut<AppState>) {
    let ctx = contexts.ctx_mut();

    // Menu bar
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Project...").clicked() {
                    state.project_name = Some("Untitled".to_string());
                    ui.close_menu();
                }
                if ui.button("Open...").clicked() {
                    ui.close_menu();
                }
                if ui.button("Save").clicked() {
                    ui.close_menu();
                }
                if ui.button("Save As...").clicked() {
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
                ui.separator();
                if ui.button("Delete").clicked() {
                    ui.close_menu();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.checkbox(&mut state.show_grid, "Show Grid").clicked() {
                    ui.close_menu();
                }
                if ui.checkbox(&mut state.show_reference, "Show Reference").clicked() {
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Zoom In").clicked() {
                    state.zoom_level = (state.zoom_level * 1.5).min(16.0);
                    ui.close_menu();
                }
                if ui.button("Zoom Out").clicked() {
                    state.zoom_level = (state.zoom_level / 1.5).max(0.5);
                    ui.close_menu();
                }
                if ui.button("Reset Zoom").clicked() {
                    state.zoom_level = 4.0;
                    ui.close_menu();
                }
            });
            ui.menu_button("Character", |ui| {
                if ui.button("New Character...").clicked() {
                    ui.close_menu();
                }
                if ui.button("Add Part...").clicked() {
                    ui.close_menu();
                }
                if ui.button("Add State...").clicked() {
                    ui.close_menu();
                }
                if ui.button("Import Rotations...").clicked() {
                    ui.close_menu();
                }
            });
            ui.menu_button("Animation", |ui| {
                if ui.button("New Animation...").clicked() {
                    ui.close_menu();
                }
                if ui.button("Add Frame").clicked() {
                    state.total_frames += 1;
                    ui.close_menu();
                }
                if ui.button("Delete Frame").clicked() {
                    if state.total_frames > 1 {
                        state.total_frames -= 1;
                        if state.current_frame >= state.total_frames {
                            state.current_frame = state.total_frames - 1;
                        }
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Play / Pause").clicked() {
                    state.is_playing = !state.is_playing;
                    ui.close_menu();
                }
            });
            ui.menu_button("Export", |ui| {
                if ui.button("Export Current Animation...").clicked() {
                    ui.close_menu();
                }
                if ui.button("Export All Animations...").clicked() {
                    ui.close_menu();
                }
            });
        });
    });

    // Asset browser (left panel)
    egui::SidePanel::left("asset_browser")
        .default_width(250.0)
        .min_width(200.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("Asset Browser");
            ui.separator();

            if state.project_name.is_none() {
                ui.label("No project loaded");
                ui.label("");
                if ui.button("New Project").clicked() {
                    state.project_name = Some("Untitled".to_string());
                }
            } else {
                egui::CollapsingHeader::new("Characters")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.label("  (No characters yet)");
                        if ui.small_button("+ Add Character").clicked() {
                            // Add character
                        }
                    });
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

            if let Some(ref part) = state.selected_part {
                ui.label(format!("Selected: {}", part));
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Position:");
                });
                ui.horizontal(|ui| {
                    ui.label("  X:");
                    let mut x = 0.0f32;
                    ui.add(egui::DragValue::new(&mut x).speed(1.0));
                    ui.label("  Y:");
                    let mut y = 0.0f32;
                    ui.add(egui::DragValue::new(&mut y).speed(1.0));
                });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Rotation:");
                    let mut rot = 0;
                    egui::ComboBox::from_id_salt("rotation")
                        .selected_text(format!("{}°", rot))
                        .show_ui(ui, |ui| {
                            for angle in (0..360).step_by(45) {
                                ui.selectable_value(&mut rot, angle, format!("{}°", angle));
                            }
                        });
                });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("State:");
                    egui::ComboBox::from_id_salt("state")
                        .selected_text("default")
                        .show_ui(ui, |ui| {
                            let _ = ui.selectable_label(true, "default");
                        });
                });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Z-Index:");
                    let mut z = 0i32;
                    ui.add(egui::DragValue::new(&mut z).speed(1));
                });
            } else {
                ui.label("No selection");
                ui.label("");
                ui.label("Select a part on the canvas");
                ui.label("or drag one from the asset");
                ui.label("browser to place it.");
            }

            ui.separator();
            ui.collapsing("Canvas Settings", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Size:");
                    let mut w = state.canvas_size.0 as i32;
                    let mut h = state.canvas_size.1 as i32;
                    ui.add(egui::DragValue::new(&mut w).speed(1).range(8..=512));
                    ui.label("x");
                    ui.add(egui::DragValue::new(&mut h).speed(1).range(8..=512));
                    state.canvas_size = (w as u32, h as u32);
                });
                ui.horizontal(|ui| {
                    ui.label("Zoom:");
                    ui.add(egui::Slider::new(&mut state.zoom_level, 0.5..=16.0).logarithmic(true));
                });
            });

            ui.collapsing("Reference Layer", |ui| {
                ui.checkbox(&mut state.show_reference, "Visible");
                if ui.button("Load Reference Image...").clicked() {
                    // Load reference
                }
                ui.horizontal(|ui| {
                    ui.label("Opacity:");
                    let mut opacity = 0.5f32;
                    ui.add(egui::Slider::new(&mut opacity, 0.0..=1.0));
                });
            });
        });

    // Timeline (bottom panel)
    egui::TopBottomPanel::bottom("timeline")
        .default_height(180.0)
        .min_height(100.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Timeline");
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
                if ui.button("⏮").clicked() {
                    if state.current_frame > 0 {
                        state.current_frame -= 1;
                    }
                }
                if ui.button("⏭").clicked() {
                    if state.current_frame < state.total_frames - 1 {
                        state.current_frame += 1;
                    }
                }

                ui.separator();
                ui.label(format!(
                    "Frame: {} / {}",
                    state.current_frame + 1,
                    state.total_frames
                ));

                ui.separator();
                if ui.button("+ Frame").clicked() {
                    state.total_frames += 1;
                }
            });

            ui.separator();

            // Frame timeline visualization
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    for frame in 0..state.total_frames {
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
                        }
                    }
                });
            });

            // Tracks area (placeholder)
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(60.0)
                .show(ui, |ui| {
                    ui.label("  Tracks: (drag parts here to animate)");
                });
        });

    // Central canvas area
    egui::CentralPanel::default().show(ctx, |ui| {
        if state.project_name.is_none() {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Welcome to Sprite Animator");
                    ui.label("");
                    ui.label("Create or open a project to begin.");
                    ui.label("");
                    if ui.button("New Project").clicked() {
                        state.project_name = Some("Untitled".to_string());
                    }
                });
            });
        } else {
            // Canvas area with grid
            let available = ui.available_size();
            let canvas_w = state.canvas_size.0 as f32 * state.zoom_level;
            let canvas_h = state.canvas_size.1 as f32 * state.zoom_level;

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

                // Vertical lines
                let mut x = canvas_rect.min.x;
                while x <= canvas_rect.max.x {
                    painter.line_segment(
                        [egui::pos2(x, canvas_rect.min.y), egui::pos2(x, canvas_rect.max.y)],
                        egui::Stroke::new(1.0, grid_color),
                    );
                    x += cell_size;
                }

                // Horizontal lines
                let mut y = canvas_rect.min.y;
                while y <= canvas_rect.max.y {
                    painter.line_segment(
                        [egui::pos2(canvas_rect.min.x, y), egui::pos2(canvas_rect.max.x, y)],
                        egui::Stroke::new(1.0, grid_color),
                    );
                    y += cell_size;
                }
            }

            // Canvas border
            painter.rect_stroke(
                canvas_rect,
                0.0,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 100, 120)),
            );

            // Show canvas info
            ui.put(
                egui::Rect::from_min_size(
                    response.rect.min + egui::vec2(10.0, 10.0),
                    egui::vec2(200.0, 20.0),
                ),
                egui::Label::new(format!(
                    "Canvas: {}x{} @ {:.1}x",
                    state.canvas_size.0, state.canvas_size.1, state.zoom_level
                )),
            );
        }
    });
}
