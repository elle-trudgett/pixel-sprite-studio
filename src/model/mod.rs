use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Rotation mode determines the angle increments for pre-drawn rotations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RotationMode {
    #[default]
    Deg45,   // 8 rotations: 0, 45, 90, 135, 180, 225, 270, 315
    Deg22_5, // 16 rotations: 0, 22.5, 45, ... 337.5
}

impl RotationMode {
    pub fn angles(&self) -> Vec<u16> {
        match self {
            RotationMode::Deg45 => vec![0, 45, 90, 135, 180, 225, 270, 315],
            RotationMode::Deg22_5 => (0..16).map(|i| (i * 225) / 10).collect(), // 0, 22, 45, 67, 90...
        }
    }

    pub fn step(&self) -> u16 {
        match self {
            RotationMode::Deg45 => 45,
            RotationMode::Deg22_5 => 22, // Actually 22.5, but we use integers
        }
    }

    /// Get the mirror angle for automatic rotation generation
    /// e.g., 45° mirrors to 315°, 90° mirrors to 270°
    pub fn mirror_angle(&self, angle: u16) -> u16 {
        if angle == 0 || angle == 180 {
            angle // These don't need mirroring
        } else {
            360 - angle
        }
    }
}

/// A single rotation variant of a state (the actual image data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rotation {
    pub angle: u16,
    /// Base64-encoded PNG data, or None if this rotation should be auto-generated via mirroring
    pub image_data: Option<String>,
    #[serde(skip)]
    pub is_mirrored: bool, // Runtime flag: true if this was generated from mirroring
}

impl Rotation {
    pub fn new(angle: u16) -> Self {
        Self {
            angle,
            image_data: None,
            is_mirrored: false,
        }
    }

    pub fn with_image(angle: u16, image_data: String) -> Self {
        Self {
            angle,
            image_data: Some(image_data),
            is_mirrored: false,
        }
    }
}

/// A state represents a specific visual variant of a part (e.g., "straight", "turned", "flap1")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub name: String,
    pub rotation_mode: RotationMode,
    /// Map of angle -> Rotation data
    pub rotations: HashMap<u16, Rotation>,
}

impl State {
    pub fn new(name: impl Into<String>, rotation_mode: RotationMode) -> Self {
        let name = name.into();
        let mut rotations = HashMap::new();

        // Initialize empty rotations for all angles
        for angle in rotation_mode.angles() {
            rotations.insert(angle, Rotation::new(angle));
        }

        Self {
            name,
            rotation_mode,
            rotations,
        }
    }

    /// Get a rotation, auto-generating via mirror if needed
    pub fn get_rotation(&self, angle: u16) -> Option<&Rotation> {
        self.rotations.get(&angle)
    }

    /// Check if this state has any actual image data
    pub fn has_images(&self) -> bool {
        self.rotations.values().any(|r| r.image_data.is_some())
    }
}

/// A part of a character (e.g., "head", "torso", "cape")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Part {
    pub name: String,
    pub states: Vec<State>,
    pub default_z: i32,
}

impl Part {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        // Create with a default state
        let default_state = State::new("default", RotationMode::Deg45);
        Self {
            name,
            states: vec![default_state],
            default_z: 0,
        }
    }

    pub fn get_state(&self, name: &str) -> Option<&State> {
        self.states.iter().find(|s| s.name == name)
    }

    pub fn get_state_mut(&mut self, name: &str) -> Option<&mut State> {
        self.states.iter_mut().find(|s| s.name == name)
    }

    pub fn add_state(&mut self, state: State) {
        self.states.push(state);
    }
}

/// A character is a collection of parts that form a complete sprite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    pub name: String,
    pub parts: Vec<Part>,
    #[serde(default)]
    pub animations: Vec<Animation>,
    #[serde(default = "default_canvas_size")]
    pub canvas_size: (u32, u32),
}

fn default_canvas_size() -> (u32, u32) {
    (64, 64)
}

impl Character {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            parts: Vec::new(),
            animations: vec![Animation::new("Untitled Animation")],
            canvas_size: (64, 64),
        }
    }

    pub fn get_part(&self, name: &str) -> Option<&Part> {
        self.parts.iter().find(|p| p.name == name)
    }

    pub fn get_part_mut(&mut self, name: &str) -> Option<&mut Part> {
        self.parts.iter_mut().find(|p| p.name == name)
    }

    pub fn add_part(&mut self, part: Part) {
        self.parts.push(part);
    }

    pub fn get_animation(&self, name: &str) -> Option<&Animation> {
        self.animations.iter().find(|a| a.name == name)
    }

    pub fn get_animation_mut(&mut self, name: &str) -> Option<&mut Animation> {
        self.animations.iter_mut().find(|a| a.name == name)
    }

    pub fn add_animation(&mut self, animation: Animation) {
        self.animations.push(animation);
    }
}

/// A placed part instance on the canvas within a frame
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacedPart {
    pub id: u64, // Unique ID for this placement
    pub character_name: String,
    pub part_name: String,
    #[serde(default)]
    pub layer_name: String, // Display name for the layer (may differ from part_name)
    pub state_name: String,
    pub rotation: u16,       // Current rotation angle
    pub position: (f32, f32), // (x, y) position on canvas
    pub z_override: Option<i32>, // Frame-level z-index override
    #[serde(default = "default_visible")]
    pub visible: bool, // Whether this layer is visible
}

fn default_visible() -> bool {
    true
}

impl PlacedPart {
    pub fn new(
        id: u64,
        character_name: impl Into<String>,
        part_name: impl Into<String>,
        state_name: impl Into<String>,
    ) -> Self {
        let part_name = part_name.into();
        Self {
            id,
            character_name: character_name.into(),
            layer_name: part_name.clone(), // Default layer name is the part name
            part_name,
            state_name: state_name.into(),
            rotation: 0,
            position: (0.0, 0.0),
            z_override: None,
            visible: true,
        }
    }

    pub fn with_layer_name(mut self, layer_name: impl Into<String>) -> Self {
        self.layer_name = layer_name.into();
        self
    }
}

/// Reference image for a single frame
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameReference {
    pub file_path: String,           // Original file path
    pub position: (f32, f32),        // Canvas-relative position
    pub scale: f32,                  // Scale factor (1.0 = fit canvas)
    // Note: opacity and show_on_top are view settings in AppState, not per-frame
}

impl FrameReference {
    pub fn new(file_path: String, scale: f32) -> Self {
        Self {
            file_path,
            position: (0.0, 0.0),
            scale,
        }
    }
}

/// A single frame in an animation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub duration_ms: u32,
    pub placed_parts: Vec<PlacedPart>,
    /// Z-index overrides at the frame level (part_name -> z_index)
    pub z_overrides: HashMap<String, i32>,
    /// Optional reference image for this frame
    #[serde(default)]
    pub reference: Option<FrameReference>,
}

impl Frame {
    pub fn new(duration_ms: u32) -> Self {
        Self {
            duration_ms,
            placed_parts: Vec::new(),
            z_overrides: HashMap::new(),
            reference: None,
        }
    }
}

/// An animation is a sequence of frames
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Animation {
    pub name: String,
    pub frames: Vec<Frame>,
    /// Z-index overrides at the animation level (part_name -> z_index)
    pub z_overrides: HashMap<String, i32>,
    /// Playback speed in frames per second
    #[serde(default = "default_fps")]
    pub fps: u32,
}

fn default_fps() -> u32 {
    12
}

impl Animation {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            frames: vec![Frame::new(100)], // Start with one frame
            z_overrides: HashMap::new(),
            fps: 12,
        }
    }

    pub fn add_frame(&mut self) {
        self.frames.push(Frame::new(100));
    }

    pub fn get_frame(&self, index: usize) -> Option<&Frame> {
        self.frames.get(index)
    }

    pub fn get_frame_mut(&mut self, index: usize) -> Option<&mut Frame> {
        self.frames.get_mut(index)
    }
}

/// Reference layer for tracing/alignment
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReferenceLayer {
    pub visible: bool,
    pub image_data: Option<String>, // Base64 PNG
    pub position: (f32, f32),
    pub scale: f32,
    pub rotation: f32, // Degrees
    pub opacity: f32,
}

impl ReferenceLayer {
    pub fn new() -> Self {
        Self {
            visible: true,
            image_data: None,
            position: (0.0, 0.0),
            scale: 1.0,
            rotation: 0.0,
            opacity: 0.5,
        }
    }
}

/// Saved editor state for restoring on load
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorState {
    #[serde(default)]
    pub active_character: Option<String>,
    #[serde(default)]
    pub current_animation: usize,
    #[serde(default)]
    pub current_frame: usize,
    #[serde(default)]
    pub active_tab: String, // "canvas" or "editor"
    #[serde(default = "default_zoom")]
    pub zoom_level: f32,
    #[serde(default = "default_true")]
    pub show_grid: bool,
    #[serde(default = "default_true")]
    pub show_labels: bool,
    #[serde(default = "default_opacity")]
    pub reference_opacity: f32,
    #[serde(default)]
    pub reference_show_on_top: bool,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            active_character: None,
            current_animation: 0,
            current_frame: 0,
            active_tab: "canvas".to_string(),
            zoom_level: 16.0,
            show_grid: true,
            show_labels: true,
            reference_opacity: 0.5,
            reference_show_on_top: false,
        }
    }
}

fn default_zoom() -> f32 {
    16.0
}

fn default_true() -> bool {
    true
}

fn default_opacity() -> f32 {
    0.5
}

/// The complete project containing all data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub version: String,
    pub name: String,
    /// Legacy field - canvas size is now per-character
    #[serde(default = "default_canvas_size", skip_serializing)]
    pub canvas_size: (u32, u32),
    pub characters: Vec<Character>,
    /// Legacy field for v1 compatibility - animations are now per-character
    #[serde(default, skip_serializing)]
    pub animations: Vec<Animation>,
    /// Legacy field - reference images are now per-frame
    #[serde(default, skip_serializing)]
    pub reference_layer: ReferenceLayer,
    /// Deduplicated reference image thumbnails (file_path -> base64 JPG)
    #[serde(default)]
    pub reference_thumbnails: HashMap<String, String>,
    /// Saved editor state
    #[serde(default)]
    pub editor_state: EditorState,
    #[serde(skip)]
    pub next_part_id: u64, // Runtime counter for unique part placement IDs
}

impl Default for Project {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: "2.0".to_string(),
            name: name.into(),
            canvas_size: (64, 64),
            characters: Vec::new(),
            animations: Vec::new(), // Empty - animations are per-character now
            reference_layer: ReferenceLayer::new(),
            reference_thumbnails: HashMap::new(),
            editor_state: EditorState::default(),
            next_part_id: 1,
        }
    }

    pub fn get_character(&self, name: &str) -> Option<&Character> {
        self.characters.iter().find(|c| c.name == name)
    }

    pub fn get_character_mut(&mut self, name: &str) -> Option<&mut Character> {
        self.characters.iter_mut().find(|c| c.name == name)
    }

    pub fn add_character(&mut self, character: Character) {
        self.characters.push(character);
    }

    /// Generate a unique ID for placed parts
    pub fn next_id(&mut self) -> u64 {
        let id = self.next_part_id;
        self.next_part_id += 1;
        id
    }

    /// Save project to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Load project from JSON string, with automatic migration from v1 format
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let mut project: Self = serde_json::from_str(json)?;

        // Initialize next_part_id to be higher than any existing part ID
        let max_id = project.characters.iter()
            .flat_map(|c| c.animations.iter())
            .flat_map(|a| a.frames.iter())
            .flat_map(|f| f.placed_parts.iter())
            .map(|p| p.id)
            .max()
            .unwrap_or(0);
        project.next_part_id = max_id + 1;

        // Migrate v1 projects: move project-level animations to characters
        if !project.animations.is_empty() {
            // Build a set of which characters are used by each animation
            for anim in std::mem::take(&mut project.animations) {
                // Find which characters are used in this animation
                let used_chars: std::collections::HashSet<String> = anim.frames.iter()
                    .flat_map(|f| f.placed_parts.iter())
                    .map(|p| p.character_name.clone())
                    .collect();

                if used_chars.len() == 1 {
                    // Animation uses only one character - assign to that character
                    let char_name = used_chars.into_iter().next().unwrap();
                    if let Some(character) = project.characters.iter_mut()
                        .find(|c| c.name == char_name)
                    {
                        character.animations.push(anim);
                    }
                } else if !used_chars.is_empty() {
                    // Animation uses multiple characters - assign to first character
                    let char_name = used_chars.into_iter().next().unwrap();
                    if let Some(character) = project.characters.iter_mut()
                        .find(|c| c.name == char_name)
                    {
                        let mut anim = anim;
                        anim.name = format!("{} (multi-char)", anim.name);
                        character.animations.push(anim);
                    }
                }
                // If no characters used, animation is orphaned and dropped
            }

            // Ensure all characters have at least one animation
            for character in &mut project.characters {
                if character.animations.is_empty() {
                    character.animations.push(Animation::new("Untitled Animation"));
                }
            }

            // Update version to indicate migration happened
            project.version = "2.0".to_string();
        }

        // Migrate canvas_size from project level to character level
        // Characters with default canvas size inherit the project's canvas_size
        let project_canvas = project.canvas_size;
        for character in &mut project.characters {
            if character.canvas_size == (64, 64) && project_canvas != (64, 64) {
                character.canvas_size = project_canvas;
            }
        }

        Ok(project)
    }

    /// Check if this project was migrated from v1 (had project-level animations)
    pub fn was_migrated(&self) -> bool {
        self.version == "2.0" && self.animations.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_creation() {
        let project = Project::new("Test Project");
        assert_eq!(project.name, "Test Project");
        assert_eq!(project.canvas_size, (64, 64));
        assert_eq!(project.version, "2.0");
        // Animations are now per-character, not project-level
        assert!(project.animations.is_empty());
    }

    #[test]
    fn test_character_parts() {
        let mut char = Character::new("Hero");
        char.add_part(Part::new("head"));
        char.add_part(Part::new("torso"));

        assert_eq!(char.parts.len(), 2);
        assert!(char.get_part("head").is_some());
        assert!(char.get_part("torso").is_some());
    }

    #[test]
    fn test_rotation_mirroring() {
        let mode = RotationMode::Deg45;
        assert_eq!(mode.mirror_angle(0), 0);
        assert_eq!(mode.mirror_angle(45), 315);
        assert_eq!(mode.mirror_angle(90), 270);
        assert_eq!(mode.mirror_angle(135), 225);
        assert_eq!(mode.mirror_angle(180), 180);
    }

    #[test]
    fn test_serialization() {
        let project = Project::new("Test");
        let json = project.to_json().unwrap();
        let loaded = Project::from_json(&json).unwrap();
        assert_eq!(loaded.name, project.name);
    }
}
