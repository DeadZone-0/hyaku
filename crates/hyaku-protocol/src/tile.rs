use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tile {
    Wall,
    Floor,
    PressurePlate { id: u8, active: bool },
    Gate { id: u8, open: bool },
    Shrine,
    StairsUp,
    Chasm,
    Bridge,
    Campfire,
}

impl Tile {
    pub const fn is_walkable(&self) -> bool {
        match self {
            Tile::Wall => false,
            Tile::Floor => true,
            Tile::PressurePlate { .. } => true,
            Tile::Gate { open, .. } => *open,
            Tile::Shrine => true,
            Tile::StairsUp => true,
            Tile::Chasm => false,
            Tile::Bridge => true,
            Tile::Campfire => true,
        }
    }

    pub const fn is_chasm(&self) -> bool {
        matches!(self, Tile::Chasm)
    }

    pub const fn glyph(&self) -> char {
        match self {
            Tile::Wall => '#',
            Tile::Floor => '.',
            Tile::PressurePlate { active: true, .. } => '_',
            Tile::PressurePlate { active: false, .. } => 'o',
            Tile::Gate { open: true, .. } => '/',
            Tile::Gate { open: false, .. } => '+',
            Tile::Shrine => 'S',
            Tile::StairsUp => '>',
            Tile::Chasm => ' ',
            Tile::Bridge => '=',
            Tile::Campfire => '*',
        }
    }

    pub const fn display_name(&self) -> &'static str {
        match self {
            Tile::Wall => "Wall",
            Tile::Floor => "Stone Floor",
            Tile::PressurePlate { active: true, .. } => "Depressed Pressure Plate",
            Tile::PressurePlate { active: false, .. } => "Pressure Plate",
            Tile::Gate { open: true, .. } => "Open Barrier Gate",
            Tile::Gate { open: false, .. } => "Locked Barrier Gate",
            Tile::Shrine => "Tower Shrine",
            Tile::StairsUp => "Ascending Stairs",
            Tile::Chasm => "Abyssal Chasm",
            Tile::Bridge => "Constructed Bridge",
            Tile::Campfire => "Resting Campfire",
        }
    }
}
