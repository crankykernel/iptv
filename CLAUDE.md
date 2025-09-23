# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Development Commands

### Build and Run
```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run in TUI mode (recommended for development)
cargo run -- --tui

# Run in CLI interactive mode
cargo run

# Run with debug logging
RUST_LOG=debug cargo run -- --tui

# Run with debug logging to file
cargo run -- --debug-log --tui
```

### Player Modes
```bash
# Play stream in detached mode (exits after starting MPV)
cargo run -- play --detached <stream_url>

# Play stream in terminal mode (shows MPV output with RPC support)
cargo run -- play --terminal <stream_url>

# Play stream in disassociated mode (completely independent MPV instance)
cargo run -- play --disassociated <stream_url>
```

### Testing and Quality
```bash
# Run tests
cargo test

# Run clippy for linting
cargo clippy -- -W clippy::all

# Format code
cargo fmt

# Check formatting without modifying
cargo fmt -- --check
```

## Architecture Overview

### Core Components

**API Layer** (`src/xtream.rs`):
- Handles all Xtreme Codes API communication
- Manages authentication, data fetching, and stream URL generation
- Implements custom deserializers for flexible API response handling
- Provides methods for categories, streams, VOD info, and series episodes

**User Interfaces**:
- **TUI Mode** (`src/tui/`): Full terminal UI with keyboard navigation, built on ratatui
  - `app.rs`: Application state machine and business logic
  - `ui.rs`: Rendering logic and layout management
  - `event.rs`: Keyboard input handling
  - `widgets.rs`: Custom UI components (scrollable text, lists)
- **CLI Mode** (`src/cli/`): Interactive menu system using inquire for quick access

**Data Management**:
- **Cache System** (`src/cache.rs`): SHA-256 based caching with automatic invalidation
- **Favourites** (`src/favourites.rs`): Persistent cross-provider favourite management
- **Configuration** (`src/config.rs`): TOML-based multi-provider configuration

**Media Playback** (`src/player/mod.rs`, `src/player/mpv.rs`, `src/player/ffplay.rs`):
- Abstract player interface with MPV as primary implementation
- Non-blocking MPV launch with background socket monitoring for responsive TUI
- Multiple playback modes: TUI (background with RPC), terminal (visible output), detached (exits after launch), disassociated (independent)
- RPC support via Unix socket for controlling existing MPV instances
- Automatic detection and reuse of running MPV instances
- Experimental ffplay support as fallback player option

### State Management

The TUI uses a state machine pattern (`AppState` enum) to manage navigation:
- Provider selection → Main menu → Category → Stream/VOD selection
- Special states for VOD info display, series navigation, and favourites
- Each state maintains its own context (selected items, scroll position, filters)

### Data Flow

1. User configuration loaded from TOML files (`~/.config/iptv/`)
2. API clients initialized for each provider
3. Data fetched and cached locally for performance (`~/.cache/iptv/`)
4. UI presents cached data with real-time filtering
5. Stream URLs generated on-demand when playback requested
6. MPV player launched with authenticated stream URLs
7. Subsequent streams sent via RPC to existing MPV instance when available

## Key Implementation Details

- **Async Operations**: All API calls and MPV operations use tokio for non-blocking I/O
- **Error Handling**: Comprehensive error propagation with anyhow
- **Progress Indication**: Visual feedback during long operations
- **Fuzzy Search**: Real-time filtering using fuzzy-matcher (case-insensitive)
- **State Preservation**: Navigation state saved when drilling into content
- **Cross-Provider Support**: Unified favourite system across multiple providers
- **MPV Integration**:
  - Smart RPC support with socket at `~/.local/state/iptv/mpv.sock`
  - Non-blocking socket monitoring prevents TUI freezing during startup
  - Async state management with Arc<RwLock<>> for thread-safe operations
- **Ignore System**: Pattern-based filtering of unwanted streams/categories

## Code Quality Standards

- Always ensure project is `cargo clippy` clean before committing
- Always run `cargo fmt` before committing (after fixing clippy issues)
- Use `RUST_LOG=debug` environment variable for debugging
- Follow existing code patterns and conventions in the codebase

## File Structure

```
src/
├── main.rs           # Entry point, CLI argument parsing
├── lib.rs            # Library root, public API
├── config.rs         # Configuration management
├── xtream.rs         # Xtream Codes API client
├── cache.rs          # Caching system
├── favourites.rs     # Favorites management
├── ignore.rs         # Ignore patterns system
├── player/
│   ├── mod.rs        # Player abstraction layer
│   ├── mpv.rs        # MPV implementation with non-blocking RPC
│   └── ffplay.rs     # FFplay fallback implementation
├── tui/
│   ├── mod.rs        # TUI module root
│   ├── app.rs        # Application state and logic
│   ├── ui.rs         # UI rendering
│   ├── event.rs      # Keyboard event handling
│   └── widgets.rs    # Custom UI widgets
└── cli/
    ├── mod.rs        # CLI module root
    ├── play.rs       # Play command implementation
    ├── list.rs       # List command implementation
    ├── search.rs     # Search functionality
    ├── info.rs       # Info command
    ├── cache.rs      # Cache management commands
    ├── favorites.rs  # Favorites CLI commands
    └── providers.rs  # Provider management
```