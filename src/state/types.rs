/// Zoom levels available in the application
pub const ZOOM_LEVELS: [f32; 14] = [
    0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0, 64.0, 128.0,
];

/// Active tab in the central panel
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ActiveTab {
    #[default]
    Canvas,
    CharacterEditor(String), // Character name being edited
}

/// Action pending confirmation due to unsaved changes
#[derive(Clone, Debug)]
pub enum PendingAction {
    CloseProject,
    NewProject,
    OpenProject,
    Exit,
}

#[derive(Clone)]
pub struct DraggedPart {
    pub character_id: u64,
    pub part_name: String,
    pub state_name: String,
}

#[derive(Clone)]
pub struct GalleryDrag {
    pub character_id: u64,
    pub character_name: String, // Kept for texture cache keys
    pub part_name: String,
    pub state_name: String,
}

#[derive(Clone, Debug)]
pub enum ContextMenuTarget {
    Character { char_name: String },
    Part { char_name: String, part_name: String },
    Animation { char_name: String, anim_index: usize, anim_name: String },
    Frame { char_name: String, anim_index: usize, frame_index: usize },
    Layer { layer_id: u64, layer_name: String },
}
