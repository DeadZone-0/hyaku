use crate::entity::{EnemyKind, EnemyState};
use crate::position::Position;
use crate::tile::Tile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Prng {
    state: u64,
}

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
        Self::generate_with_size(seed, floor, Self::DEFAULT_WIDTH, Self::DEFAULT_HEIGHT)
    }

    pub fn generate_with_size(seed: u64, floor: u32, width: i32, height: i32) -> Self {
        let mut prng = Prng::new(seed, floor);
        let mut map = Self::new_empty(floor, width, height);
        let is_boss_floor = floor % 10 == 0;
        let is_safe_floor = !is_boss_floor && (floor % 5 == 0);

        if is_boss_floor {
            map.generate_boss_floor(&mut prng);
        } else if is_safe_floor {
            map.generate_safe_floor(&mut prng);
        } else {
            map.generate_standard_floor(&mut prng);
        }

        map
    }

    fn generate_boss_floor(&mut self, _prng: &mut Prng) {
        let arena_w = (self.width - 16).max(24);
        let arena_h = (self.height - 8).max(16);
        let start_x = (self.width - arena_w) / 2;
        let start_y = (self.height - arena_h) / 2;

        for y in start_y..(start_y + arena_h) {
            for x in start_x..(start_x + arena_w) {
                let pos = Position::new(x, y);
                self.set_tile(pos, Tile::Floor);
            }
        }

        // Inner pillars
        let p_offset_x = arena_w / 4;
        let p_offset_y = arena_h / 4;
        let pillars = [
            Position::new(start_x + p_offset_x, start_y + p_offset_y),
            Position::new(start_x + arena_w - p_offset_x, start_y + p_offset_y),
            Position::new(start_x + p_offset_x, start_y + arena_h - p_offset_y),
            Position::new(start_x + arena_w - p_offset_x, start_y + arena_h - p_offset_y),
        ];
        for pillar in pillars {
            self.set_tile(pillar, Tile::Wall);
        }

        self.spawn_pos = Position::new(start_x + 3, start_y + arena_h / 2);
        self.stairs_pos = Position::new(start_x + arena_w - 4, start_y + arena_h / 2);
        self.set_tile(self.stairs_pos, Tile::StairsUp);

        let shrine_pos = Position::new(start_x + arena_w / 2, start_y + 2);
        self.set_tile(shrine_pos, Tile::Shrine);
        self.shrine_pos = Some(shrine_pos);

        // Gate blocking stairs until boss is dealt with
        let gate_pos = Position::new(self.stairs_pos.x - 2, self.stairs_pos.y);
        self.set_tile(gate_pos, Tile::Gate { id: 1, open: false });
        self.gates.push((1, gate_pos));

        // Pressure plate in the arena
        let plate_pos = Position::new(start_x + arena_w / 2, start_y + arena_h - 3);
        self.set_tile(plate_pos, Tile::PressurePlate { id: 1, active: false });
        self.plates.push((1, plate_pos));

        // Boss enemy in arena center
        let boss_pos = Position::new(start_x + arena_w / 2, start_y + arena_h / 2);
        let hp_mult = 1.0 + (self.floor as f32) * 0.15;
        let boss = EnemyState::new(1, EnemyKind::FloorBoss, boss_pos, hp_mult);
        self.initial_enemies.push(boss);

        // Minion guards
        let mut enemy_id = 2;
        for i in 0..4 {
            let offset_x = if i % 2 == 0 { -4 } else { 4 };
            let offset_y = if i < 2 { -3 } else { 3 };
            let guard_pos = Position::new(boss_pos.x + offset_x, boss_pos.y + offset_y);
            let kind = if i % 2 == 0 { EnemyKind::Brute } else { EnemyKind::Archer };
            self.initial_enemies.push(EnemyState::new(enemy_id, kind, guard_pos, hp_mult));
            enemy_id += 1;
        }
    }

    fn generate_safe_floor(&mut self, _prng: &mut Prng) {
        let room_w = (self.width - 20).max(20);
        let room_h = (self.height - 10).max(12);
        let start_x = (self.width - room_w) / 2;
        let start_y = (self.height - room_h) / 2;

        for y in start_y..(start_y + room_h) {
            for x in start_x..(start_x + room_w) {
                self.set_tile(Position::new(x, y), Tile::Floor);
            }
        }

        self.spawn_pos = Position::new(start_x + 3, start_y + room_h / 2);
        self.stairs_pos = Position::new(start_x + room_w - 4, start_y + room_h / 2);
        self.set_tile(self.stairs_pos, Tile::StairsUp);

        let campfire_pos = Position::new(start_x + room_w / 2, start_y + room_h / 2);
        self.set_tile(campfire_pos, Tile::Campfire);

        let shrine_pos = Position::new(start_x + room_w / 2, start_y + 3);
        self.set_tile(shrine_pos, Tile::Shrine);
        self.shrine_pos = Some(shrine_pos);
    }

    fn generate_standard_floor(&mut self, prng: &mut Prng) {
        let num_rooms = prng.gen_range(6, 10);
        let mut rooms: Vec<Rect> = Vec::new();

        for _ in 0..(num_rooms * 5) {
            if rooms.len() >= num_rooms as usize {
                break;
            }
            let w = prng.gen_range(8, 16);
            let h = prng.gen_range(6, 10);
            let x = prng.gen_range(2, self.width - w - 2);
            let y = prng.gen_range(2, self.height - h - 2);
            let new_room = Rect::new(x, y, w, h);

            let mut overlaps = false;
            for r in &rooms {
                if new_room.intersects(r) {
                    overlaps = true;
                    break;
                }
            }

            if !overlaps {
                for ry in y..(y + h) {
                    for rx in x..(x + w) {
                        self.set_tile(Position::new(rx, ry), Tile::Floor);
                    }
                }
                rooms.push(new_room);
            }
        }

        if rooms.is_empty() {
            let fallback = Rect::new(4, 4, self.width - 8, self.height - 8);
            for y in fallback.y..(fallback.y + fallback.h) {
                for x in fallback.x..(fallback.x + fallback.w) {
                    self.set_tile(Position::new(x, y), Tile::Floor);
                }
            }
            rooms.push(fallback);
        }

        // Connect adjacent rooms with corridors
        for i in 0..(rooms.len().saturating_sub(1)) {
            let prev = rooms[i].center();
            let next = rooms[i + 1].center();
            self.carve_corridor(prev, next);
        }

        self.spawn_pos = rooms[0].center();
        let last_idx = rooms.len() - 1;
        self.stairs_pos = rooms[last_idx].center();
        self.set_tile(self.stairs_pos, Tile::StairsUp);

        // Shrine in a middle room
        if rooms.len() > 2 {
            let shrine_idx = rooms.len() / 2;
            let shrine_pos = rooms[shrine_idx].center();
            if shrine_pos != self.spawn_pos && shrine_pos != self.stairs_pos {
                self.set_tile(shrine_pos, Tile::Shrine);
                self.shrine_pos = Some(shrine_pos);
            }
        }

        // Chasm hazard on select floors
        if self.floor % 3 == 0 && rooms.len() >= 3 {
            let chasm_room = &rooms[1];
            let chasm_x = chasm_room.center().x;
            for y in chasm_room.y..(chasm_room.y + chasm_room.h) {
                let pos = Position::new(chasm_x, y);
                if self.get_tile(pos) == Some(Tile::Floor) {
                    self.set_tile(pos, Tile::Chasm);
                }
            }
            // Pre-place a bridge over one slot so players can pass without crafting
            let bridge_pos = Position::new(chasm_x, chasm_room.center().y);
            self.set_tile(bridge_pos, Tile::Bridge);
        }

        // Puzzle gates on select floors
        if self.floor % 2 == 1 && rooms.len() >= 4 {
            let gate_room = &rooms[rooms.len() - 2];
            let plate_room = &rooms[1];

            let gate_pos = gate_room.center();
            let plate_pos = plate_room.center();

            if gate_pos != self.stairs_pos && plate_pos != self.spawn_pos {
                self.set_tile(gate_pos, Tile::Gate { id: 1, open: false });
                self.gates.push((1, gate_pos));

                self.set_tile(plate_pos, Tile::PressurePlate { id: 1, active: false });
                self.plates.push((1, plate_pos));
            }
        }

        // Populate enemies in rooms (excluding spawn room)
        let mut enemy_id = 1;
        let hp_mult = 1.0 + (self.floor as f32) * 0.08;

        for room in rooms.iter().skip(1) {
            let enemy_count = prng.gen_range(1, 3);
            for _ in 0..enemy_count {
                let ex = prng.gen_range(room.x + 1, room.x + room.w - 2);
                let ey = prng.gen_range(room.y + 1, room.y + room.h - 2);
                let pos = Position::new(ex, ey);

                if pos != self.stairs_pos && self.is_walkable(pos) {
                    let roll = prng.gen_range(0, 100);
                    let kind = if roll < 40 {
                        EnemyKind::Crawler
                    } else if roll < 70 {
                        EnemyKind::Archer
                    } else if roll < 90 {
                        EnemyKind::Brute
                    } else {
                        EnemyKind::Phantom
                    };
                    self.initial_enemies.push(EnemyState::new(enemy_id, kind, pos, hp_mult));
                    enemy_id += 1;
                }
            }
        }
    }

    fn carve_corridor(&mut self, start: Position, end: Position) {
        let mut cur = start;
        while cur.x != end.x {
            self.set_tile(cur, Tile::Floor);
            cur.x += if end.x > cur.x { 1 } else { -1 };
        }
        while cur.y != end.y {
            self.set_tile(cur, Tile::Floor);
            cur.y += if end.y > cur.y { 1 } else { -1 };
        }
        self.set_tile(end, Tile::Floor);
    }
}
