# ⚡ TurboFind

**Blazing fast file indexer and search for Windows** — written in Rust 🦀

Indexes **783,000+ files in ~25 seconds**, then searches them in **sub-millisecond** time with fuzzy matching. Built as a lightweight alternative to Windows Search and Everything.

![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)
![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-blue)
![License](https://img.shields.io/badge/License-MIT-green)

---

## Demo

```
  ╔══════════════════════════════════════╗
  ║   ⚡ TurboFind v1.0                  ║
  ║   Blazing Fast File Search            ║
  ╚══════════════════════════════════════╝

  ⚡ Indexing filesystem...
  ✅ Indexed 783373 files in 24.90s (31463 files/sec)

  🔍 shader_wa
  ──────────────────────────────────────────
  ✨ shader_water.glsl          C:\Projects\threejs-terrain\shaders
  ✨ water_reflection.glsl      C:\Projects\threejs-terrain\shaders
  📄 shader_water_backup.txt    C:\Projects\old\shaders
```

## Features

- **Parallel filesystem crawling** — saturates all CPU cores with [rayon](https://github.com/rayon-rs/rayon)
- **Fuzzy matching** — finds files even with typos (`budgt` → `budget.xlsx`)
- **Binary index cache** — first run indexes in ~25s, subsequent launches reload in <100ms
- **Filter syntax** — `ext:rs config` finds only `.rs` files matching "config"
- **Interactive TUI** — terminal UI with keyboard navigation, powered by [crossterm](https://github.com/crossterm-rs/crossterm)
- **File type icons** — visual indicators for code, images, audio, video, archives, documents
- **Cross-platform** — Windows, Linux, macOS

## Performance

| Metric | TurboFind |
|--------|-----------|
| Index speed | ~31,000 files/sec |
| Search latency | <20ms (fuzzy) |
| Cache reload | <100ms |
| Memory footprint | ~50-80 MB |
| Binary size | ~2 MB (release, stripped) |

## Installation

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (`rustup`)
- On Windows: Visual Studio C++ Build Tools

### Build from source

```bash
git clone https://github.com/fulong97/turbofind.git
cd turbofind
cargo build --release
```

The binary will be at `target/release/turbofind.exe` (Windows) or `target/release/turbofind` (Linux/macOS).

### Install globally

```bash
cargo install --path .
```

## Usage

```bash
# Index default paths (C:\Users on Windows, /home on Linux)
turbofind

# Index specific directories
turbofind C:\Projects D:\Documents

# Index entire drive
turbofind C:\
```

### Search Syntax

| Query | Description |
|-------|-------------|
| `budget` | Fuzzy search all files matching "budget" |
| `ext:rs config` | Only `.rs` files matching "config" |
| `ext:pdf invoice` | Only PDFs matching "invoice" |
| `ext:glsl water` | Only GLSL shaders matching "water" |
| `dir: projects` | Only directories matching "projects" |

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Type` | Live search with instant results |
| `↑` / `↓` | Navigate results |
| `Enter` | Open file with default application |
| `Ctrl+O` | Open containing folder in Explorer |
| `Backspace` | Delete last character |
| `Esc` | Quit |

## Architecture

```
Filesystem          Indexer                    Search
───────────    ─────────────────         ─────────────────
                                        
C:\Users ──┐   ┌─────────────────┐      ┌───────────────┐
           ├──▶│ Parallel Crawler │─────▶│ FileIndex     │
C:\Docs ───┘   │ (rayon, walkdir) │      │ Vec<Entry>    │
               └─────────────────┘      │ ext_map       │
                       │                └───────┬───────┘
                       ▼                        │
               ┌─────────────────┐      ┌───────▼───────┐
               │ Binary Cache    │      │ Fuzzy Matcher │
               │ (bincode)       │      │ (skim)        │
               │ <100ms reload   │      │ <1ms search   │
               └─────────────────┘      └───────┬───────┘
                                                │
                                        ┌───────▼───────┐
                                        │ Crossterm TUI │
                                        │ Interactive   │
                                        └───────────────┘
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| [rayon](https://crates.io/crates/rayon) | Parallel filesystem crawling |
| [walkdir](https://crates.io/crates/walkdir) | Recursive directory traversal |
| [fuzzy-matcher](https://crates.io/crates/fuzzy-matcher) | Skim-based fuzzy matching |
| [crossterm](https://crates.io/crates/crossterm) | Cross-platform terminal UI |
| [bincode](https://crates.io/crates/bincode) | Fast binary serialization for index cache |
| [serde](https://crates.io/crates/serde) | Serialization framework |
| [dirs](https://crates.io/crates/dirs) | Platform-specific cache directory |

## How it works

1. **Indexing** — TurboFind walks the filesystem in parallel using all available CPU cores. Each file's name, path, size, extension, and modification time are stored in a `Vec<FileEntry>`.

2. **Caching** — The index is serialized to a binary file using bincode. On subsequent launches, if the cache is less than 1 hour old, it's loaded directly (~100ms) instead of re-crawling.

3. **Searching** — Queries are matched against filenames using the Skim fuzzy matching algorithm (same as fzf). Results are scored and sorted by relevance. Extension filters (`ext:`) use a pre-built HashMap for O(1) lookup.

4. **Display** — Results are rendered in an interactive terminal UI with crossterm, handling keyboard input for navigation and file opening.

## Contributing

Contributions welcome! Some ideas:

- [ ] Real-time filesystem watching (auto-update index on file changes)
- [ ] Regex search mode
- [ ] Content search (grep inside files)
- [ ] Custom ignore patterns (`.turbofindignore`)
- [ ] Config file for default roots and settings
- [ ] GUI frontend (egui or tauri)

## License

MIT

---

*Built with Rust 🦀 and caffeine ☕*
