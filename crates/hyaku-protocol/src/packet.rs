use crate::direction::Direction;
use crate::entity::{EnemyState, FloorCarving, PlayerState, ProjectileState};
use crate::position::Position;
use crate::tile::Tile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientPacket {
    Join {
        username: String,
        class_idx: usize,
    },
    Move(Direction),
    Attack(Direction),
    ClassSkill(Direction),
    ToggleTether {
        target_id: Option<String>,
    },
    DeployDecoy,
    DepositShards {
        amount: u32,
    },
    BuildBridge(Direction),
    LightCampfire,
    CarveFloor {
        message: String,
    },
    UpvoteCarving {
        pos: Position,
    },
    AscendStairs,
    Respawn,
    Chat {
        text: String,
    },
    Ping {
        seq: u64,
    },
}

impl ClientPacket {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickState {
    pub tick: u64,
    pub floor: u32,
    pub players: Vec<PlayerState>,
    pub enemies: Vec<EnemyState>,
    pub projectiles: Vec<ProjectileState>,
    pub tiles_override: Vec<(Position, Tile)>,
    pub carvings: Vec<FloorCarving>,
    pub tower_shards_bank: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerPacket {
    Welcome {
        player_id: String,
        floor: u32,
        map_seed: u64,
    },
    Tick(TickState),
    FloorTransition {
        floor: u32,
        map_seed: u64,
        message: String,
    },
    ChatMessage {
        sender: String,
        text: String,
        timestamp: u64,
    },
    Announcement {
        text: String,
    },
    Error {
        message: String,
    },
    Pong {
        seq: u64,
    },
}

impl ServerPacket {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}
