use crate::entity::{EnemyState, FloorCarving};
use crate::position::Position;
use crate::tile::Tile;
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Prng {
    state: u64,
}

#[allow(dead_code)]
impl Prng {
    pub fn new(seed: u64, floor: u32) -> Self {
        let combined = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add((floor as u64).wrapping_mul(1442695040888963407))
            .wrapping_add(1);
        let mut rng = Self {
            state: if combined == 0 { 0xdeadbeefcafebabe } else { combined },
        };
        for _ in 0..8 {
            rng.next_u64();
        }
        rng
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    pub fn gen_range(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        let range = (max - min + 1) as u64;
        min + (self.next_u64() % range) as i32
    }

    pub fn gen_bool(&mut self, chance_pct: u32) -> bool {
        (self.next_u64() % 100) < chance_pct as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub const fn center(&self) -> Position {
        Position::new(self.x + self.w / 2, self.y + self.h / 2)
    }

    pub const fn intersects(&self, other: &Rect) -> bool {
        self.x <= other.x + other.w
            && self.x + self.w >= other.x
            && self.y <= other.y + other.h
            && self.y + self.h >= other.y
    }

    pub const fn contains(&self, p: Position) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloorMap {
    pub floor: u32,
    pub width: i32,
    pub height: i32,
    pub tiles: Vec<Tile>,
    pub spawn_pos: Position,
    pub stairs_pos: Position,
    pub shrine_pos: Option<Position>,
    pub initial_enemies: Vec<EnemyState>,
    pub gates: Vec<(u8, Position)>,
    pub plates: Vec<(u8, Position)>,
    #[serde(default)]
    pub initial_carvings: Vec<FloorCarving>,
}

impl FloorMap {
    pub const DEFAULT_WIDTH: i32 = 80;
    pub const DEFAULT_HEIGHT: i32 = 30;

    pub fn new_empty(floor: u32, width: i32, height: i32) -> Self {
        let size = (width * height) as usize;
        Self {
            floor,
            width,
            height,
            tiles: vec![Tile::Wall; size],
            spawn_pos: Position::new(2, 2),
            stairs_pos: Position::new(width - 3, height - 3),
            shrine_pos: None,
            initial_enemies: Vec::new(),
            gates: Vec::new(),
            plates: Vec::new(),
            initial_carvings: Vec::new(),
        }
    }

    fn index(&self, pos: Position) -> Option<usize> {
        if pos.in_bounds(self.width, self.height) {
            Some((pos.y * self.width + pos.x) as usize)
        } else {
            None
        }
    }

    pub fn get_tile(&self, pos: Position) -> Option<Tile> {
        self.index(pos).map(|idx| self.tiles[idx])
    }

    pub fn set_tile(&mut self, pos: Position, tile: Tile) {
        if let Some(idx) = self.index(pos) {
            self.tiles[idx] = tile;
        }
    }

    pub fn is_walkable(&self, pos: Position) -> bool {
        self.get_tile(pos).map_or(false, |t| t.is_walkable())
    }

    pub fn generate(seed: u64, floor: u32) -> Self {
        crate::map_gen::generate_floor_map(floor, seed)
    }

    pub fn generate_with_size(seed: u64, floor: u32, width: i32, height: i32) -> Self {
        crate::map_gen::generate_floor_map_with_size(floor, seed, width, height)
    }
}
