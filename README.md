# Pixel Sprite Studio

Pixel Sprite Studio is an animation tool for pixel art characters. It lets you organize all your character art in a single project and composite animations from individual pieces—useful for quick drafting or creating final, ready-to-use animations.

This is not a graphics editor. You import your own art and use Pixel Sprite Studio to assemble and animate it, then export to spritesheets.

## Features

- **Part-based characters** - Build characters from reusable parts (head, torso, limbs, etc.) each with multiple states and pre-drawn rotations
- **Drag-and-drop animation** - Drag parts onto the canvas to compose frames, reposition and layer them visually
- **Multi-angle rotation system** - Import 8 or 16 rotation angles per part state; missing angles auto-generate via mirroring
- **Animation timeline** - Frame-by-frame editing with playback preview and per-animation FPS control
- **Spritesheet export** - Export animations as spritesheets with JSON metadata for game engines
- **Self-contained projects** - All art is embedded in `.pss` project files, no external dependencies

## Building

```bash
cargo build --release
```

## License

MIT
