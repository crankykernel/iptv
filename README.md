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
- **CLI Mode**: Scriptable command-line interface for automation
- **API Mode**: Direct access to Xtream API calls for advanced users

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
- **Rust**: Version 1.80.0 or later
- **MPV Player**: Required for video playback
  ```bash
  # Ubuntu/Debian
  sudo apt install mpv
  
  # Fedora
  sudo dnf install mpv
  
  # Arch
  sudo pacman -S mpv
  ```

### System Requirements
- Linux/macOS/Windows (Linux recommended)
- Terminal with UTF-8 support
- Internet connection for IPTV streaming

## Installation

### From Source
```bash
# Clone the repository
git clone https://github.com/your-username/iptv
cd iptv

# Build and install
cargo build --release

# Optional: Install to PATH
cargo install --path .
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

### Environment Variables

```bash
# Set default provider for CLI commands
export IPTV_PROVIDER="My IPTV Service"

# Enable debug logging
export RUST_LOG=debug
```



## Usage

### TUI Mode (Default)

Launch the full terminal user interface:
```bash
# Default mode - launches TUI
./iptv

# Or explicitly
./iptv tui
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
| `Enter` | Select item / Play stream |
| `d` | Play in detached window (streams/episodes) |
| `Esc`/`b` | Go back |
| `q` | Quit application |
| `/` | Fuzzy search/filter |
| `f` | Toggle favourite |
| `s` | Stop playback |
| `?`/`F1` | Toggle help |
| `Space` | Scroll down (in VOD info) |
| `Shift+Space` | Scroll up (in VOD info) |
| `Ctrl+C` | Force quit |

### CLI Mode

Scriptable command-line interface for automation:
```bash
# Search for content
./iptv cli search "movie name" --type movie
./iptv cli search "news" --type live



# Output in different formats
./iptv cli search "sports" --format json
./iptv cli search "sports" --format m3u
```

### API Mode

Direct access to Xtream API for advanced usage:
```bash
# Get user information
./iptv api user-info

# List live categories
./iptv api live-categories

# Get VOD information
./iptv api vod-info --id <vod_id>
```

## Command Line Options

```
USAGE:
    iptv [OPTIONS] [COMMAND]

OPTIONS:
    -v, --verbose     Enable verbose (debug) logging
        --debug-log   Enable debug logging to file (iptv_debug.log)
    -h, --help        Print help information
    -V, --version     Print version information

COMMANDS:
    tui    Launch interactive TUI (default if no command given)
    cli    Command-line interface for scriptable operations
    api    Execute raw API calls
    help   Print this message or the help of the given subcommand(s)
```

### CLI Subcommands

```
USAGE:
    iptv cli [OPTIONS] <COMMAND>

COMMANDS:
    play          Play stream/movie/episode by ID
    search        Search content across providers
    list          List streams/movies/series
    info          Get detailed information about content
    url           Get stream URL
    fav           Manage favorites
    cache         Manage cache
    providers     Manage providers
    add-provider  Interactively add a new provider

OPTIONS:
    -p, --provider <PROVIDER>  Provider name to use (or set IPTV_PROVIDER env var)
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
- Manual cache management via CLI:
  ```bash
  # Clear cache for a provider
  ./iptv cli cache clear <provider_name>
  
  # Refresh cache
  ./iptv cli cache refresh <provider_name>
  ```

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
