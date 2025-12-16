# Emoru

A fast, cross-platform emoji picker for Linux and macOS.

![Emoru Demo](https://raw.githubusercontent.com/YOUR_USERNAME/emoru/main/demo.gif)

## Features

- **Fast search**: Type to instantly filter through 1700+ emojis
- **Keyboard-driven**: Navigate with arrow keys, select with Enter
- **Auto-paste**: Selected emoji is automatically pasted into your active application
- **Match highlighting**: Search terms are highlighted in bold within results
- **Cross-platform**: Works on Linux (X11) and macOS
- **Native performance**: Built with Rust and Slint UI framework

## Installation

### Pre-built Binaries

Download the latest release for your platform from the [Releases](https://github.com/YOUR_USERNAME/emoru/releases) page:

- **Linux (x86_64)**: `emoru-linux-x86_64.tar.gz`
- **macOS (Intel)**: `emoru-macos-x86_64.tar.gz`
- **macOS (Apple Silicon)**: `emoru-macos-aarch64.tar.gz`

Extract and run:

```bash
tar -xzf emoru-linux-x86_64.tar.gz
./emoru
```

For convenient access, move to a directory in your PATH:

```bash
sudo mv emoru /usr/local/bin/
sudo mv data /usr/local/share/emoru/
```

### Build from Source

Requirements:
- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- Linux: `libxcb`, `libxkbcommon` development packages

```bash
# Clone the repository
git clone https://github.com/YOUR_USERNAME/emoru.git
cd emoru

# Build release binary
cargo build --release

# Run
./target/release/emoru
```

#### Linux Dependencies

**Ubuntu/Debian:**
```bash
sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev
```

**Fedora:**
```bash
sudo dnf install libxcb-devel libxkbcommon-devel
```

**Arch Linux:**
```bash
sudo pacman -S libxcb libxkbcommon
```

## Usage

1. Launch `emoru` (bind it to a keyboard shortcut for quick access)
2. Start typing to search emojis
3. Use **Up/Down arrows** to navigate results
4. Press **Enter** or **Tab** to select and paste
5. Press **Escape** to cancel
6. Press **Ctrl+Backspace** to clear search

### Recommended: Keyboard Shortcut

For best experience, bind emoru to a global keyboard shortcut:

**i3wm** (`~/.config/i3/config`):
```
bindsym $mod+period exec emoru
```

**sway** (`~/.config/sway/config`):
```
bindsym $mod+period exec emoru
```

**GNOME**:
Settings → Keyboard → Custom Shortcuts → Add `emoru`

**macOS**:
Use Automator or tools like Hammerspoon to bind to a shortcut.

## Data Files

Emoru searches for emoji data in these locations (in order):

1. `./data/` (next to the executable)
2. `~/.emoru/data/`
3. `~/emoji_picker_images/` (legacy)

The data directory should contain:
- `emojis9.txt` - Emoji index file
- `emoji_picker_images/` - Directory with emoji images (base64 encoded PNGs)

## Technology

- **[Rust](https://www.rust-lang.org/)** - Systems programming language
- **[Slint](https://slint.dev/)** - Declarative UI framework for native applications
- **[arboard](https://crates.io/crates/arboard)** - Cross-platform clipboard support
- **[xdotool](https://github.com/jordansissel/xdotool)** (Linux) - For simulating paste keypress

## Project Structure

```
emoru/
├── Cargo.toml          # Rust dependencies
├── build.rs            # Slint build configuration
├── src/
│   └── main.rs         # Application logic
├── ui/
│   └── main.slint      # UI definition (Slint markup)
└── data/
    ├── emojis9.txt     # Emoji index
    └── emoji_picker_images/
        └── *.base64    # Emoji images
```

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

Contributions welcome! Please open an issue or submit a pull request.

## Acknowledgments

- Emoji images from [OpenMoji](https://openmoji.org/) / [Twemoji](https://twemoji.twitter.com/)
- Inspired by the need for a fast, keyboard-driven emoji picker on Linux
