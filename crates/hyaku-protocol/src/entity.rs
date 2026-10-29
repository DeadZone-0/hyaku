use crate::position::Position;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerClassPreset {
    pub name: &'static str,
    pub description: &'static str,
    pub glyph: char,
    pub color_rgb: (u8, u8, u8),
    pub max_hp: i32,
    pub max_mana: i32,
    pub attack_power: i32,
}

// TODO(ZE): Customize class glyphs, names, and RGB color palettes
pub static PLAYER_CLASSES: &[PlayerClassPreset] = &[
    PlayerClassPreset {
        name: "Blade",
        description: "Frontline vanguard specializing in high survivability and sweeping strikes",
        glyph: '@',
        color_rgb: (235, 75, 75),
        max_hp: 120,
        max_mana: 40,
        attack_power: 25,
    },
    PlayerClassPreset {
        name: "Arcanist",
        description: "Channeler of tower ether, wielding high mana for tethering and holo-decoys",
        glyph: '@',
        color_rgb: (80, 160, 255),
        max_hp: 80,
        max_mana: 120,
        attack_power: 35,
    },
    PlayerClassPreset {
        name: "Ranger",
        description: "Agile scout equipped with long-range strikes and swift movement",
        glyph: '@',
        color_rgb: (85, 215, 120),
        max_hp: 95,
        max_mana: 70,
        attack_power: 20,
    },
    PlayerClassPreset {
        name: "Architect",
        description: "Master builder adept at bridging chasms, lighting campfires, and tower defenses",
        glyph: '@',
        color_rgb: (235, 195, 65),
        max_hp: 105,
        max_mana: 85,
        attack_power: 18,
    },
];

pub fn get_class_preset(idx: usize) -> &'static PlayerClassPreset {
    if idx < PLAYER_CLASSES.len() {
        &PLAYER_CLASSES[idx]
    } else {
        &PLAYER_CLASSES[0]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerState {
    pub id: String,
    pub username: String,
    pub pos: Position,
    pub hp: i32,
    pub max_hp: i32,
    pub mana: i32,
    pub max_mana: i32,
    pub shards: u32,
    pub floor: u32,
    pub class_idx: usize,
    pub glyph: char,
    pub color_rgb: (u8, u8, u8),
    pub tether_target: Option<String>,
    pub last_chat: Option<(String, u64)>,
}

impl PlayerState {
    pub fn new(id: String, username: String, class_idx: usize, pos: Position, floor: u32) -> Self {
        let preset = get_class_preset(class_idx);
        Self {
            id,
            username,
            pos,
            hp: preset.max_hp,
            max_hp: preset.max_hp,
            mana: preset.max_mana,
            max_mana: preset.max_mana,
            shards: 0,
            floor,
            class_idx,
            glyph: preset.glyph,
            color_rgb: preset.color_rgb,
            tether_target: None,
            last_chat: None,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.hp > 0
    }

    pub fn take_damage(&mut self, amount: i32) {
        self.hp = (self.hp - amount).max(0);
    }

    pub fn heal(&mut self, amount: i32) {
        self.hp = (self.hp + amount).min(self.max_hp);
    }

    pub fn restore_mana(&mut self, amount: i32) {
        self.mana = (self.mana + amount).min(self.max_mana);
    }

    pub fn spend_mana(&mut self, amount: i32) -> bool {
        if self.mana >= amount {
            self.mana -= amount;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EnemyKind {
    Crawler,
    Archer,
    Brute,
    Phantom,
    FloorBoss,
}

impl EnemyKind {
    pub const fn name(self) -> &'static str {
        match self {
            EnemyKind::Crawler => "Shadow Crawler",
            EnemyKind::Archer => "Abyssal Archer",
            EnemyKind::Brute => "Tower Brute",
            EnemyKind::Phantom => "Wandering Phantom",
            EnemyKind::FloorBoss => "Floor Sentinel Boss",
        }
    }

    pub const fn default_glyph(self) -> char {
        match self {
            EnemyKind::Crawler => 'c',
            EnemyKind::Archer => 'a',
            EnemyKind::Brute => 'B',
            EnemyKind::Phantom => 'p',
            EnemyKind::FloorBoss => 'X',
        }
    }

    pub const fn default_hp(self) -> i32 {
        match self {
            EnemyKind::Crawler => 30,
            EnemyKind::Archer => 45,
            EnemyKind::Brute => 90,
            EnemyKind::Phantom => 60,
            EnemyKind::FloorBoss => 350,
        }
    }

    pub const fn color_rgb(self) -> (u8, u8, u8) {
        match self {
            EnemyKind::Crawler => (180, 70, 70),
            EnemyKind::Archer => (140, 200, 80),
            EnemyKind::Brute => (210, 120, 50),
            EnemyKind::Phantom => (160, 110, 220),
            EnemyKind::FloorBoss => (240, 40, 40),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyState {
    pub id: u32,
    pub kind: EnemyKind,
    pub pos: Position,
    pub hp: i32,
    pub max_hp: i32,
    pub glyph: char,
    pub is_boss: bool,
}

impl EnemyState {
    pub fn new(id: u32, kind: EnemyKind, pos: Position, hp_multiplier: f32) -> Self {
        let base_hp = (kind.default_hp() as f32 * hp_multiplier).round() as i32;
        let is_boss = matches!(kind, EnemyKind::FloorBoss);
        Self {
            id,
            kind,
            pos,
            hp: base_hp,
            max_hp: base_hp,
            glyph: kind.default_glyph(),
            is_boss,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.hp > 0
    }

    pub fn take_damage(&mut self, amount: i32) {
        self.hp = (self.hp - amount).max(0);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectileState {
    pub pos: Position,
    pub glyph: char,
    pub color_rgb: (u8, u8, u8),
}

impl ProjectileState {
    pub const fn new(pos: Position, glyph: char, color_rgb: (u8, u8, u8)) -> Self {
        Self {
            pos,
            glyph,
            color_rgb,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloorCarving {
    pub pos: Position,
    pub author: String,
    pub message: String,
    pub upvotes: u32,
}

impl FloorCarving {
    pub fn new(pos: Position, author: String, message: String) -> Self {
        Self {
            pos,
            author,
            message,
            upvotes: 0,
        }
    }
}
