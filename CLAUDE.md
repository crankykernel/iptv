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

# Run with debug logging
RUST_LOG=debug cargo run -- --tui

# Run with debug logging to file
cargo run -- --debug-log --tui
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

**API Layer** (`src/xtream_api.rs`):
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

**Media Playback** (`src/player.rs`, `src/mpv_player.rs`):
- Abstract player interface with MPV implementation
- Handles stream URLs, authentication, and background playback

### State Management

The TUI uses a state machine pattern (`AppState` enum) to manage navigation:
- Provider selection → Main menu → Category → Stream/VOD selection
- Special states for VOD info display, series navigation, and favourites
- Each state maintains its own context (selected items, scroll position, filters)

### Data Flow

1. User configuration loaded from TOML files
2. API clients initialized for each provider
3. Data fetched and cached locally for performance
4. UI presents cached data with real-time filtering
5. Stream URLs generated on-demand when playback requested
6. MPV player launched with authenticated stream URLs

## Key Implementation Details

- **Async Operations**: All API calls use tokio for non-blocking I/O
- **Error Handling**: Comprehensive error propagation with anyhow
- **Progress Indication**: Visual feedback during long operations
- **Fuzzy Search**: Real-time filtering using fuzzy-matcher
- **State Preservation**: Navigation state saved when drilling into content
- **Cross-Provider Support**: Unified favourite system across multiple providers
- Always make sure project is "cargo clippy" clean before commit. Always run "cargo fmt" before commit, but after fixing clippy issues.