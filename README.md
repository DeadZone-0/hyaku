# HYAKU (百) - 100-Floor Co-op Tower MMO TUI

```text
  ██╗  ██╗██╗   ██╗ █████╗ ██╗  ██╗██╗   ██╗    (百)
  ██║  ██║╚██╗ ██╔╝██╔══██╗██║ ██╔╝██║   ██║
  ███████║ ╚████╔╝ ███████║█████═╝ ██║   ██║    100-FLOOR CO-OP
  ██╔══██║  ╚██╔╝  ██╔══██║██╔═██╗ ██║   ██║    TOWER ASCENSION
  ██║  ██║   ██║   ██║  ██║██║ ╚██╗╚██████╔╝    
  ╚═╝  ╚═╝   ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝
```

**Hyaku** (百) is a cooperative terminal MMO where players climb a 100-floor tower together in real time. Work with other climbers to solve multi-plate puzzles, bridge chasms with gathered shards, fight monsters, and unlock floor shrines to reach the summit.

---

## Quick Install (Linux & macOS)

Install the latest release.

```bash
curl -fsSL https://raw.githubusercontent.com/DeadZone-0/hyaku/main/install.sh | bash
```

Start the game.

```bash
hyaku
```

You can also set your climber name and class at launch.

```bash
hyaku -u "Aria" -c 0
```

Run the updater to get the latest release.

```bash
hyaku-update
```

---

## Audio

Hyaku includes a dungeon loop that fades between screens, menu clicks, sword swings and hits, chimes for shrine completion and floor ascent, and gong cues for the Gatekeeper and death. Run `hyaku --mute` to turn it all off. On macOS, ambience uses Core Audio and sound effects use `afplay`. On Linux, playback uses `pw-play`, `paplay`, `ffplay`, or `aplay` when available.

The bundled clips are CC0. The ambience is [Whispers in the Fog by Ruhinre](https://opengameart.org/content/whispers-in-the-fog). The sword clips come from [StarNinjas](https://opengameart.org/node/122900), and the menu clicks, chimes, gongs, and hurt sound come from [rubberduck](https://opengameart.org/content/100-cc0-sfx).

## Live Multiplayer Server

An authoritative 24/7 game server is hosted on Hack Club Nest:
- **WebSocket Endpoint**: `ws://[2a01:4f9:3080:3192::2124]:3000/ws`
- **Container**: `ze68@hackclub.app` (VMID: 2124)
- **Auto-Upgrade**: The server runs as a systemd daemon (`hyaku-server.service`) and automatically updates itself from GitHub releases every 3 minutes.

---

## Controls & Co-op Mechanics

| Key | Action | Description |
| :--- | :--- | :--- |
| `[W] [A] [S] [D]` / Arrows | **Move** | Move your climber across the grid |
| `[Space]` | **Directional Slash** | Sweep up to two tiles forward. If the swing would miss, it turns toward an adjacent enemy. |
| `[Q]` | **Lightning Tether** | Link with the nearest ally or holo-decoy to shock enemies along the beam |
| `[R]` | **Holo-Decoy** | Spend 20 Mana to spawn a 15-second hologram that holds down pressure plates |
| `[B]` + Direction | **Build Bridge** | Spend 5 Shards to permanently bridge an adjacent Chasm (`═`) for all players |
| `[E]` | **Interact / Shrine** | Deposit shards into the Floor Shrine (`☼`), use stairs, or revive teammates |
| `[T]` / `[Enter]` | **Tower Comms** | Open chat deck to message climbers on your floor |
| `[C]` | **Kindle Campfire** | Spend 15 Shards to build a permanent campfire that heals nearby climbers |
| `[M]` | **Carve Floor** | Leave a permanent message on bare stone |
| `[L]` | **Praise Carving** | Upvote a nearby carving once and heal its living author |
| `[Tab]` | **Hall of Fame & Lore** | Browse the first climber to reach each unlocked floor |
| `[Esc]` | **Cancel / Back** | Close active prompt or cancel target selection |

---

## Hero Classes

1. **Blade (0)**: Frontline vanguard with high HP and sweeping strikes.
2. **Arcanist (1)**: Tower ether specialist with high mana for tethers and holo-decoys.
3. **Ranger (2)**: Agile scout equipped with long-range piercing attacks.
4. **Architect (3)**: Support builder adept at bridging chasms and tower defenses.

---

## Local Development & Building from Source

### Prerequisites
- [Rust](https://rustup.rs/) (1.80+ recommended)

### Build & Run Locally

```bash
# Clone the repository
git clone https://github.com/DeadZone-0/hyaku.git
cd hyaku

# Start local server
cargo run -p hyaku-server

# In another terminal, start client connecting to local server
cargo run -p hyaku-client -- --local
```

### World save

The server writes `hyaku_save.json` in its working directory. Set `HYAKU_SAVE_PATH` to use another file. The save keeps floor unlocks, first climbers, shrine offerings, gates, bridges, campfires, carvings, and votes. Keep the file when moving the server.

Reconnect with the same climber name and server address from the same client profile to resume your climb. The client creates a private resume key in its user data directory. The server saves your location on disconnect and checkpoints active climbers every five seconds.

A climber using the same name from a different profile gets a separate location. Earlier name-only locations can't be carried over to a keyed profile.

### Run Tests
```bash
cargo test --workspace
```

---

## Architecture

- **`crates/hyaku-protocol`**: Deterministic 100-floor world generator, network packet serialization, entity definitions, and coordinate math.
- **`crates/hyaku-server`**: Authoritative 20-TPS WebSocket server with real-time collision detection, co-op puzzle state, and floor broadcasting.
- **`crates/hyaku-client`**: Terminal UI rendered via `ratatui` and `crossterm`, featuring a 4-pane command deck, radar minimap, chat log, and speech bubbles.
- **`.github/workflows/release.yml`**: Automated cross-platform release pipeline for Linux (musl static) and macOS (Intel & Apple Silicon).
