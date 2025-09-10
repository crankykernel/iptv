# IPTV Terminal Player

A modern, terminal-based IPTV streaming client with Xtreme API support. Features both TUI (Terminal User Interface) and CLI modes for browsing and playing live TV, movies, and TV series.

**Note**: This project is almost completely coded by an AI agent, guided by an experienced software developer.

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
- **MPV Player**: Required for video playback

### System Requirements
- Linux with terminal support
- Internet connection for IPTV streaming

## Installation

### Using Cargo Install
```bash
cargo install --git https://github.com/your-username/iptv
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

## License

MIT

---

**Note**: This application is for use with legitimate IPTV services. Users are responsible for ensuring they have proper authorization to access the content they stream.
