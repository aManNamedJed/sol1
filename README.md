# Sol 1

A cozy minimalist top-down Mars exploration game built with Rust and WebAssembly.

## Play Online

**🎮 [Play Sol 1 on GitHub Pages](https://aManNamedJed.github.io/sol1/)**

## Overview

Sol 1 is a calm, meditative browser game where you control a single robot exploring the Martian surface. There are no enemies, no combat, and no harsh failure states—just a small robot on a large planet, slowly making Mars a little more alive.

### Key Features

- **Minimal & Calm**: Clean visuals, no flashing, no harsh warnings
- **Energy Management**: Movement costs energy; return to base or charging stations to recharge
- **Fog of War**: Explore the unknown—only see what's within your vision radius
- **AI Autopilot**: Watch the robot explore autonomously (press `A` to toggle)
- **Day/Night Cycle**: Experience Mars's day and night with sun/moon dial indicator
- **Charging Stations**: Place strategic recharge points (1-day boot-up time)
- **Ice Terraforming**: Collect ice samples to increase Mars habitability
- **Resource Depletion**: Ice deposits are finite—explore to find more
- **Pure WebAssembly**: Runs entirely in the browser, no external assets

## Architecture

The game is built with a clean separation of concerns:

```
sol1/
├── Cargo.toml          # Dependencies and build config
├── src/
│   ├── lib.rs          # WASM entry point and game loop
│   └── game/
│       ├── mod.rs      # Module organization
│       ├── types.rs    # Core data types
│       ├── world.rs    # World grid and terraforming
│       ├── robot.rs    # Robot state and actions
│       ├── systems.rs  # Game logic systems
│       ├── renderer.rs # Canvas rendering
│       ├── input.rs    # Keyboard handling
│       └── game.rs     # Main game state
├── static/
│   └── index.html      # Game interface
└── README.md
```

## Game Mechanics

### Robot

- **Position**: Moves one tile at a time
- **Energy**: Starts at 100, depletes with actions
- **Integrity**: Health stat (100, currently unused)
- **Ice Samples**: Carries collected ice (auto-deposits at base)
- **Actions**:
  - Move: 1 energy per tile
  - Collect: 3 energy (depletes ice deposits)
  - Place Charging Station: 5 energy

### World

- **Grid**: 200x200 tiles
- **Tile Types**:
  - Regolith (Mars soil, default)
  - Rock (impassable)
  - Ice (finite resource, turns to regolith when collected)
  - Base (spawn point, recharge station)
  - Charging Station (placeable recharge points)
- **Procedural Generation**: Sparse clusters of rocks and ice
- **Fog of War**: 6-tile vision radius, explored areas remain visible but dimmed

### Day/Night Cycle

- Full cycle takes ~2 minutes
- **Day** (time 0.0–0.5): Recharge energy at base or operational charging stations (5 energy/sec)
- **Night** (time 0.5–1.0): No recharge, darker visuals
- **Sun/Moon Dial**: Visual indicator in top-right shows current time

### Energy System

- When energy reaches 0:
  - Robot powers down
  - Day advances
  - **Game Over** if not at base or operational charging station
  - Full energy restore if at valid charging point

### Charging Stations

- Cost: 5 energy to place
- Boot-up time: 1 full day
- Can only be placed on regolith or ice (not rocks or other stations)
- Strategic placement extends exploration range

### Ice Terraforming

- Collect ice samples and return to base (auto-deposits)
- Base processes 1 ice per 10 seconds
- Each ice sample increases Mars health by 2%
- 50 ice samples = 100% terraformed Mars
- Visible as green tint spreading from base

## Controls

| Key       | Action                 |
| --------- | ---------------------- |
| `↑ ↓ ← →` | Move robot             |
| `E`       | Collect ice resource   |
| `B`       | Build charging station |
| `A`       | Toggle AI autopilot    |

### AI Autopilot

Press `A` to enable/disable autonomous mode. The robot will:

- Explore unexplored areas of Mars
- Detect and collect ice deposits (when far enough from base)
- Return to base automatically when carrying 3+ ice samples or low on energy
- Place charging stations strategically (~30 tiles from base, 15 tiles apart)
- Manage energy to avoid getting stranded

## Build Instructions

### Prerequisites

1. **Install Rust**: https://rustup.rs/
2. **Install wasm-pack**:
   ```bash
   curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
   ```

### Build

```bash
# Build the WebAssembly module
wasm-pack build --target web

# The output will be in pkg/
```

### Run

Serve the static folder with any HTTP server:

```bash
# Option 1: Python
python3 -m http.server 8000

# Option 2: Rust
cargo install simple-http-server
simple-http-server -p 8000

# Option 3: Node.js
npx serve -p 8000
```

Then open: http://localhost:8000/static/

## Dependencies

Minimal and intentional:

- `wasm-bindgen` - Rust/JavaScript interop
- `web-sys` - Web API bindings

No game engines. No TypeScript. Just Rust and the browser.

## Design Philosophy

### Cozy Design Principles

- No flashing or harsh effects
- No loud warnings or fail states
- Soft color palette (Mars tones shifting to subtle greens)
- Smooth day/night transitions
- Robot "powers down" instead of dying

### Technical Principles

- Clean separation of simulation and rendering
- Fixed timestep updates for consistent behavior
- No unwrap() spam—proper error handling
- Small, readable modules
- Minimal dependencies

## Future Expansion Ideas

These are commented but not yet implemented:

- **Buildable Solar Panels**: Increase energy capacity
- **Resource Storage**: Collect and store materials
- **Helper Drones**: Automate simple tasks
- **Wear & Repair**: Integrity affects movement speed
- **Ambient Audio**: Subtle Mars wind sounds
- **Science Experiments**: Unlock new abilities
- **Radio Messages**: Narrative snippets from Earth

## Development

### Project Goals

- Calm, meditative gameplay
- No pressure or time limits
- Visual-only feedback (no UI clutter)
- Hopeful tone—making Mars bloom

### Performance

- Optimized for size with `opt-level = "z"`
- Link-time optimization enabled
- Runs at 60fps with low resource usage

## License

This is a personal project. Feel free to learn from the code.

## Acknowledgments

Inspired by games like:

- _A Short Hike_ (calm exploration)
- _Townscaper_ (minimal interface)
- _Lonely Mountains: Downhill_ (cozy minimalism)

---

_A small robot. A large planet. One sol at a time._
