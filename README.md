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

Install the latest release with a single command:

```bash
curl -fsSL https://raw.githubusercontent.com/DeadZone-0/hyaku/main/install.sh | bash
```

Once installed, simply run:

```bash
hyaku
```

Or choose your climber name and hero class:

```bash
hyaku -u "Aria" -c 0
```

To auto-update Hyaku to the latest release at any time:

```bash
hyaku-update
```

---

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
| `[Space]` | **Directional Slash** | Melee sweep in your facing direction |
| `[Q]` | **Lightning Tether** | Link with the nearest ally or holo-decoy to shock enemies along the beam |
| `[R]` | **Holo-Decoy** | Spend 20 Mana to spawn a 15-second hologram that holds down pressure plates |
| `[B]` + Direction | **Build Bridge** | Spend 5 Shards to permanently bridge an adjacent Chasm (`═`) for all players |
| `[E]` | **Interact / Shrine** | Deposit shards into the Floor Shrine (`☼`), use stairs, or revive teammates |
| `[T]` / `[Enter]` | **Tower Comms** | Open chat deck to message climbers on your floor |
| `[K]` | **Carve Floor** | Carve a permanent message onto the floor tile |
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
