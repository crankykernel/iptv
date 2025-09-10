<!--
SPDX-License-Identifier: MIT
SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>
-->

# IPTV Terminal Player

A modern, terminal-based IPTV streaming client with Xtreme API support. Features both TUI (Terminal User Interface) and CLI modes for browsing and playing live TV, movies, and TV series.

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)
![Terminal](https://img.shields.io/badge/Terminal-%23054020?style=for-the-badge&logo=gnu-bash&logoColor=white)

## Features

### 🎥 Media Playback
- **Live TV**: Stream live television channels
- **Movies**: Browse and watch movies with detailed information
- **TV Series**: Watch TV shows with season/episode navigation
- **MPV Integration**: Uses MPV player for optimal playback experience

### 🎛️ Interface Options
- **TUI Mode**: Full-featured terminal user interface with keyboard navigation
- **CLI Mode**: Interactive menu system for quick access
- **Rofi Integration**: External launcher support for desktop environments

### ⭐ Smart Features
- **Favourites Management**: Save and organize your favorite content
- **Multi-Provider Support**: Connect to multiple IPTV providers simultaneously
- **Intelligent Caching**: Fast loading with automatic cache management
- **Fuzzy Search**: Quick content discovery with search/filter functionality
- **Cross-Provider Favourites**: Access favorites from all configured providers

### 🔧 Technical Features
- **Xtreme API**: Full support for Xtreme Codes API
- **Background Playback**: Start streams and continue using the terminal
- **Async Architecture**: Non-blocking, responsive interface
- **Configurable UI**: Customizable page sizes and display options

## Prerequisites

### Required Dependencies
- **Rust**: Version 1.70 or later
- **MPV Player**: For video playback (highly recommended)

### System Requirements
- Linux, macOS, or Windows with terminal support
- Internet connection for IPTV streaming

### Installing MPV
MPV is the recommended video player for the best experience:

**Ubuntu/Debian:**
```bash
sudo apt update && sudo apt install mpv
```

**Fedora/RHEL:**
```bash
sudo dnf install mpv
```

**macOS (Homebrew):**
```bash
brew install mpv
```

**Arch Linux:**
```bash
sudo pacman -S mpv
```

## Installation

### From Source (Recommended)
```bash
# Clone the repository
git clone <repository-url>
cd iptv

# Build the application
cargo build --release

# The binary will be available at target/release/iptv
```

### Development Build
```bash
cargo build
# Binary available at target/debug/iptv
```

## Configuration

### Creating Configuration File

The application looks for configuration in these locations (in order):
1. `./config.toml` (current directory)
2. `~/.config/iptv/config.toml` (recommended)

### Initial Setup

1. **Generate example configuration:**
   ```bash
   ./iptv
   ```
   This creates `config.example.toml` if no config is found.

2. **Copy and edit the configuration:**
   ```bash
   # Option 1: Local config
   cp config.example.toml config.toml
   
   # Option 2: User config directory (recommended)
   mkdir -p ~/.config/iptv
   cp config.example.toml ~/.config/iptv/config.toml
   ```

3. **Edit configuration with your provider details:**
   ```bash
   nano ~/.config/iptv/config.toml
   ```

### Configuration Format

```toml
# Multiple providers supported
[[providers]]
name = "My IPTV Service"
url = "https://your-server.com:port/player_api.php"
username = "your-username"
password = "your-password"

[[providers]]
name = "Secondary Provider"
url = "https://another-server.com:port/player_api.php"
username = "username2"
password = "password2"

[ui]
page_size = 20  # Items per page in menus
```

### Interactive Provider Setup

You can also add providers interactively:
```bash
./iptv add-provider
```

This will prompt you for provider details and automatically update your configuration.

## Usage

### TUI Mode (Recommended)

Launch the full terminal user interface:
```bash
./iptv --tui
```

**TUI Keyboard Shortcuts:**

| Key | Action |
|-----|--------|
| `↑`/`k` | Move up |
| `↓`/`j` | Move down |
| `PgUp` | Page up (10 items) |
| `PgDn` | Page down (10 items) |
| `Home` | Jump to first item |
| `End` | Jump to last item |
| `Enter` | Select item |
| `Esc`/`b` | Go back |
| `q` | Quit application |
| `/` | Fuzzy search/filter |
| `f` | Toggle favourite |
| `s` | Stop playback |
| `?`/`F1` | Toggle help |
| `Space` | Scroll down (in movie info) |
| `Shift+Space` | Scroll up (in movie info) |
| `Ctrl+C` | Force quit |

### CLI Mode

Launch the interactive command-line interface:
```bash
./iptv
```

Navigate through menus using number selections and follow the prompts.

### Rofi Integration

For desktop environments, launch favourites in Rofi:
```bash
./iptv rofi
```

This creates a searchable list of all your favourites across all providers.

## Command Line Options

```
USAGE:
    iptv [OPTIONS] [COMMAND]

OPTIONS:
    -c, --config <FILE>    Configuration file path
    -v, --verbose          Enable verbose (debug) logging
        --tui             Use TUI (Terminal User Interface) mode
        --debug-log       Enable debug logging to file (iptv_debug.log)
    -h, --help            Print help information
    -V, --version         Print version information

COMMANDS:
    rofi            Launch rofi menu with favourites
    add-provider    Interactively add a new Xtreme API provider
    help            Print this message or the help of the given subcommand(s)
```

## Features in Detail

### Favourites System
- Add/remove favourites with the `f` key in TUI mode
- Favourites are stored locally and persist between sessions
- Cross-provider favourites allow accessing content from all configured providers
- Quick access through dedicated favourites menu

### Caching System
- Automatic caching of API responses for faster navigation
- Cache is stored in the user's cache directory
- Intelligent cache invalidation ensures fresh content
- Manual refresh available with `r` key

### Search and Filtering
- Press `/` to activate fuzzy search in any list
- Search works across all content types (channels, movies, series)
- Real-time filtering as you type
- Supports partial matches and typos

### Multi-Provider Support
- Configure multiple IPTV providers in a single config file
- Switch between providers or view combined favourites
- Each provider's content is cached separately
- Provider selection menu when multiple providers are configured

## Troubleshooting

### MPV Not Found
If you see "MPV not found" warnings:
1. Install MPV using your system's package manager (see Prerequisites)
2. Ensure MPV is in your system PATH
3. The application will fall back to basic playback mode without MPV

### Configuration Issues
- Check configuration file location and format
- Verify provider URLs and credentials
- Test provider connectivity with verbose logging: `./iptv -v`

### Playback Issues
- Ensure stable internet connection
- Check if streams work in MPV directly
- Try different stream quality if available
- Review debug logs: `./iptv --debug-log --tui`

### Performance Issues
- Clear cache files in `~/.cache/iptv/`
- Reduce page size in configuration
- Enable debug logging to identify bottlenecks

## Development

### Building from Source
```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run -- --tui
```

### Project Structure
```
src/
├── main.rs           # CLI argument parsing and main entry point
├── lib.rs            # Library exports
├── config.rs         # Configuration management
├── xtream_api.rs     # Xtreme Codes API client
├── player.rs         # Media player abstraction
├── mpv_player.rs     # MPV player implementation
├── cache.rs          # Caching system
├── favourites.rs     # Favourites management
├── cli/              # CLI menu system
│   ├── mod.rs
│   └── menu.rs
└── tui/              # Terminal User Interface
    ├── mod.rs
    ├── app.rs        # Application state and logic
    ├── ui.rs         # UI rendering
    ├── event.rs      # Event handling
    └── widgets.rs    # Custom UI widgets
```

### Contributing
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

This project is provided as-is. Please ensure you comply with all applicable laws and terms of service for your IPTV providers.

## Support

For issues, questions, or contributions:
1. Check existing issues in the repository
2. Create a new issue with detailed information
3. Include debug logs when reporting problems
4. Specify your operating system and terminal type

---

**Note**: This application is for use with legitimate IPTV services. Users are responsible for ensuring they have proper authorization to access the content they stream.