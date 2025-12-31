# Sprite Animator

A Rust/Bevy application for creating sprite animations with a hierarchical part-based system, pre-drawn rotations, and spritesheet export.

## Features

- **Part-Based Character System** - Define characters with reusable parts (head, torso, cape, etc.)
- **State Variants** - Create multiple states for each part (straight, turned, flap1, flap2)
- **Pre-Drawn Rotations** - Import rotations at 45° or 22.5° intervals; missing angles are auto-generated via mirroring
- **Animation Timeline** - Multi-track timeline with drag-and-drop part placement
- **3-Tier Z-Index System** - Layer ordering at character, animation, and frame levels
- **Reference Layer** - Overlay reference images with adjustable position, scale, rotation, and opacity
- **Spritesheet Export** - Export single animations or batch export all to spritesheets
- **Self-Contained Projects** - JSON format with embedded base64 PNG data

## Building

Requires Rust 1.75+ and Cargo.

```bash
cargo build --release
```

## Running

```bash
cargo run --release
```

## Project Format

Projects are saved as `.sprite-animator.json` files containing:
- Character definitions with parts, states, and rotation images (base64 PNG)
- Animation definitions with frames and part placements
- Canvas size and project metadata

See [DESIGN.md](DESIGN.md) for the complete technical specification.

## License

MIT
