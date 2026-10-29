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

        // Wall partition sealing off the stairs alcove with a locked gate
        let wall_x = self.stairs_pos.x - 2;
        for y in start_y..(start_y + arena_h) {
            let pos = Position::new(wall_x, y);
            if y == self.stairs_pos.y {
                self.set_tile(pos, Tile::Gate { id: 1, open: false });
                self.gates.push((1, pos));
            } else {
                self.set_tile(pos, Tile::Wall);
            }
        }

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
        let w = self.width;
        let h = self.height;

        // 1. Define 5 purposeful chambers with deterministic spatial separation
        // Room 0: Spawn Sanctuary (West)
        let r0_w = prng.gen_range(10, 13);
        let r0_h = prng.gen_range(6, 8);
        let r0_x = prng.gen_range(3, 5);
        let r0_y = prng.gen_range(h / 3, h / 2);
        let room_spawn = Rect::new(r0_x, r0_y, r0_w, r0_h);

        // Room 1: The Puzzle Chamber (North)
        let r1_w = prng.gen_range(11, 14);
        let r1_h = prng.gen_range(6, 8);
        let r1_x = prng.gen_range(20, 25);
        let r1_y = prng.gen_range(3, 5);
        let room_puzzle = Rect::new(r1_x, r1_y, r1_w, r1_h);

        // Room 2: The Sanctum of Renewal (South)
        let r2_w = prng.gen_range(11, 14);
        let r2_h = prng.gen_range(6, 8);
        let r2_x = prng.gen_range(20, 25);
        let r2_y = prng.gen_range((h - 11).max(18), (h - 9).max(20));
        let room_sanctum = Rect::new(r2_x, r2_y, r2_w, r2_h);

        // Room 3: Guardian Hall (East-Central)
        let r3_w = prng.gen_range(14, 18);
        let r3_h = prng.gen_range(8, 11);
        let r3_x = prng.gen_range(40, 45);
        let r3_y = prng.gen_range((h / 3 - 2).max(7), (h / 3 + 2).max(11));
        let room_guardian = Rect::new(r3_x, r3_y, r3_w, r3_h);

        // Room 4: The Ascension Vault (Far East)
        let r4_w = prng.gen_range(9, 12);
        let r4_h = prng.gen_range(6, 8);
        let r4_x = prng.gen_range((w - 15).max(64), (w - 13).max(66));
        let r4_y = prng.gen_range(h / 3, h / 2);
        let room_stairs = Rect::new(r4_x, r4_y, r4_w, r4_h);

        let rooms = [room_spawn, room_puzzle, room_sanctum, room_guardian, room_stairs];

        // Carve all rooms
        for r in &rooms {
            for ry in r.y..(r.y + r.h) {
                for rx in r.x..(r.x + r.w) {
                    self.set_tile(Position::new(rx, ry), Tile::Floor);
                }
            }
        }

        // 2. Carve corridors with clear topological flow
        // Spawn -> Puzzle
        self.carve_corridor(
            Position::new(room_spawn.x + room_spawn.w, room_spawn.center().y),
            Position::new(room_puzzle.center().x, room_puzzle.y + room_puzzle.h),
        );
        // Spawn -> Sanctum
        self.carve_corridor(
            Position::new(room_spawn.x + room_spawn.w, room_spawn.center().y),
            Position::new(room_sanctum.center().x, room_sanctum.y),
        );
        // Puzzle -> Guardian Hall
        self.carve_corridor(
            Position::new(room_puzzle.x + room_puzzle.w, room_puzzle.center().y),
            Position::new(room_guardian.x, room_guardian.y + 2),
        );
        // Sanctum -> Guardian Hall
        self.carve_corridor(
            Position::new(room_sanctum.x + room_sanctum.w, room_sanctum.center().y),
            Position::new(room_guardian.x, room_guardian.y + room_guardian.h - 3),
        );

        // Guardian Hall -> Ascension Vault
        let corridor_y = room_guardian.center().y;
        let start_pos = Position::new(room_guardian.x + room_guardian.w, corridor_y);
        let end_pos = Position::new(room_stairs.center().x, corridor_y);
        self.carve_corridor(start_pos, end_pos);

        if corridor_y != room_stairs.center().y {
            self.carve_corridor(end_pos, room_stairs.center());
        }

        // 3. Place Ascension Gate blocking the corridor between Guardian Hall and Stairs Vault
        let gate_x = (start_pos.x + room_stairs.x) / 2;
        let gate_pos = Position::new(gate_x, corridor_y);

        // Seal corridor above and below the gate with solid walls
        self.set_tile(Position::new(gate_pos.x, gate_pos.y - 1), Tile::Wall);
        self.set_tile(Position::new(gate_pos.x, gate_pos.y + 1), Tile::Wall);
        self.set_tile(gate_pos, Tile::Gate { id: 1, open: false });
        self.gates.push((1, gate_pos));

        // 4. Room 0 Features: Spawn point and Safe Campfire
        self.spawn_pos = Position::new(room_spawn.x + 2, room_spawn.center().y);
        let campfire_pos = room_spawn.center();
        self.set_tile(campfire_pos, Tile::Campfire);

        // 5. Room 1 Features: Runic Pressure Plate (unlocks gate 1)
        let plate_pos = room_puzzle.center();
        self.set_tile(plate_pos, Tile::PressurePlate { id: 1, active: false });
        self.plates.push((1, plate_pos));

        // 6. Room 2 Features: Shrine of Renewal
        let shrine_pos = room_sanctum.center();
        self.set_tile(shrine_pos, Tile::Shrine);
        self.shrine_pos = Some(shrine_pos);

        // 7. Room 3 Features: Chasm Hazard (every 3rd floor) and Floor Guardian
        if self.floor % 3 == 0 {
            let chasm_x = room_guardian.center().x;
            for cy in room_guardian.y..(room_guardian.y + room_guardian.h) {
                let pos = Position::new(chasm_x, cy);
                if self.get_tile(pos) == Some(Tile::Floor) {
                    self.set_tile(pos, Tile::Chasm);
                }
            }
            let bridge_pos = Position::new(chasm_x, room_guardian.center().y);
            self.set_tile(bridge_pos, Tile::Bridge);
        }

        // 8. Room 4 Features: Ascension Stairs
        self.stairs_pos = room_stairs.center();
        self.set_tile(self.stairs_pos, Tile::StairsUp);

        // 9. Populate Enemies with clear roles
        let mut enemy_id = 1;
        let hp_mult = 1.0 + (self.floor as f32) * 0.08;

        // Room 1 (Puzzle): 1-2 guards defending the plate
        let guard_pos1 = Position::new(room_puzzle.center().x - 2, room_puzzle.center().y);
        self.initial_enemies.push(EnemyState::new(enemy_id, EnemyKind::Crawler, guard_pos1, hp_mult));
        enemy_id += 1;

        if prng.gen_bool(50) {
            let guard_pos2 = Position::new(room_puzzle.center().x + 2, room_puzzle.center().y);
            self.initial_enemies.push(EnemyState::new(enemy_id, EnemyKind::Archer, guard_pos2, hp_mult));
            enemy_id += 1;
        }

        // Room 2 (Sanctum): 1 guard defending the shrine
        let sanctum_guard = Position::new(room_sanctum.center().x - 2, room_sanctum.center().y);
        self.initial_enemies.push(EnemyState::new(enemy_id, EnemyKind::Phantom, sanctum_guard, hp_mult));
        enemy_id += 1;

        // Room 3 (Guardian Hall): Elite Floor Guardian (defeat unlocks the gate!)
        let guardian_pos = Position::new(room_guardian.center().x + 2, room_guardian.center().y);
        let mut guardian = EnemyState::new(enemy_id, EnemyKind::Brute, guardian_pos, hp_mult * 1.5);
        guardian.is_boss = true;
        self.initial_enemies.push(guardian);
        enemy_id += 1;

        // Guardian's escort minions
        let escort_pos1 = Position::new(room_guardian.x + 3, room_guardian.y + 2);
        self.initial_enemies.push(EnemyState::new(enemy_id, EnemyKind::Crawler, escort_pos1, hp_mult));
        enemy_id += 1;

        let escort_pos2 = Position::new(room_guardian.x + 3, room_guardian.y + room_guardian.h - 3);
        self.initial_enemies.push(EnemyState::new(enemy_id, EnemyKind::Archer, escort_pos2, hp_mult));
        enemy_id += 1;

        // Room 4 (Stairs Vault): 1 final stairs guard
        let stairs_guard_pos = Position::new(room_stairs.x + 2, room_stairs.center().y);
        if stairs_guard_pos != self.stairs_pos {
            self.initial_enemies.push(EnemyState::new(enemy_id, EnemyKind::Phantom, stairs_guard_pos, hp_mult));
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
