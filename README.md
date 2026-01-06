# Pixel Sprite Studio

Pixel Sprite Studio is an animation tool for pixel art characters. You can organize character art for a single project together and composite animations from individual pieces (e.g. head, body, legs, sword, etc.)
It might be useful for quick drafting (and then touch up by hand) or creating final, ready-to-use animations. You can then swap out pieces if you improve the art and just re-export all the animations at once!

<img width="1442" height="833" alt="image" src="https://github.com/user-attachments/assets/af8b26d0-4ccc-41a8-90d5-10367aab46ea" />

## Features

- **Part-based characters** - Build characters from reusable parts (head, torso, limbs, etc.) each with multiple states and pre-drawn rotations
- **Drag-and-drop animation** - Drag parts onto the canvas to compose frames, reposition and layer them visually
- **Multi-angle rotation system** - Import 8 or 16 rotation angles per part state; missing angles auto-generate via mirroring
- **Animation timeline** - Frame-by-frame editing with playback preview and per-animation FPS control
- **Spritesheet export** - Export animations as spritesheets with JSON metadata for game engines
- **Self-contained projects** - All art is embedded in `.pss` project files, no external dependencies

## Download
Only released for Windows right not but you can build it for Mac or Linux

Releases page: https://github.com/elle-trudgett/pixel-sprite-studio/releases

## Building

```bash
cargo build --release
```

## License

MIT
