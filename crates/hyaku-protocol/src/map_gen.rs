use crate::entity::{EnemyKind, EnemyState, FloorCarving};
use crate::map::{FloorMap, Rect};
use crate::position::Position;
use crate::tile::Tile;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Sanctuary Lobby ASCII template string (80x30).
/// Legend:
/// - `#`: Solid Wall
/// - `.`: Stone Floor
/// - `P`: Player Sanctuary Spawn point
/// - `C`: Resting Campfire
/// - `S`: Central Shrine [百]
/// - `H`: Hall of Fame ancestral monument pedestal (with initial carving)
/// - `_`: Runic Pressure Plate
/// - `+`: Ceremonial Barrier Gate (locked initially)
/// - `E`: Ascension Portal Stairs
/// - `~`: Reflective Chasm Pool
/// - `=`: Decorative Stone Bridge
pub const SANCTUARY_LOBBY_ASCII: &str = "\
################################################################################\n\
#..............................................................................#\n\
#...#################...################################...#################...#\n\
#...#...............#...#...H......H........H......H...#...#...............#...#\n\
#...#...............#...#..............................#...#...............#...#\n\
#...#...#########...#...#..............................#...#...#########...#...#\n\
#...#...#.......#...#...#...##...##............##...##.#...#...#.......#...#...#\n\
#...#...#.......#...........##...##............##...##.........#.......#...#...#\n\
#...#...#.......#.......#..............................#.......#.......#...#...#\n\
#...#...#########.......#..............................#.......#########...#...#\n\
#...#...................#......##................##....#.......#################\n\
#...#...................#......##................##....#.......#..............##\n\
#...#...#########..............................................#..............##\n\
#...#...#.......#..............................................#..............##\n\
#...#...#...C...#......................S.......................#..............##\n\
#...#...#......................................................+.............E##\n\
#...#...#.......#..............................................#..............##\n\
#...#...#...P...#..............................................#..............##\n\
#...#...#########..............................................#..............##\n\
#...#...................#......##................##....#.......#..............##\n\
#...#...................#......##................##....#.......#################\n\
#...#...................#..............................#...................#...#\n\
#...#...#########.......#..............................#.......#########...#...#\n\
#...#...#.......#.......#...~~~~~~~~~......~~~~~~~~~...#.......#.......#...#...#\n\
#...#...#.......#...........~.......~......~.......~...........#.......#...#...#\n\
#...#...#.......#...#...#...~...=...~......~...=...~...#...#...#.......#...#...#\n\
#...#...#########...#...#...~...=...~......~...=...~...#...#...#########...#...#\n\
#...#...............#...#...~~~~=~~~~._.._.~~~~=~~~~...#...#...............#...#\n\
#...#################...################################...#################...#\n\
################################################################################";

/// Deterministically computes a floor seed using integer hashing
pub fn compute_floor_seed(world_seed: u64, floor_num: u32) -> u64 {
    world_seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add((floor_num as u64).wrapping_mul(1442695040888963407))
        .wrapping_add(0x9e3779b97f4a7c15)
}

/// Parses an ASCII art template map string into a FloorMap.
/// Used for Floor 1 (Sanctuary Lobby) and pre-designed levels.
pub fn parse_ascii_map(floor_num: u32, template: &str, width: i32, height: i32) -> FloorMap {
    let mut map = FloorMap::new_empty(floor_num, width, height);
    let lines: Vec<&str> = template.lines().collect();

    let mut hof_counter = 0;

    for (y, line) in lines.iter().enumerate() {
        if y as i32 >= height {
            break;
        }
        for (x, ch) in line.chars().enumerate() {
            if x as i32 >= width {
                break;
            }
            let pos = Position::new(x as i32, y as i32);

            match ch {
                '#' => {
                    map.set_tile(pos, Tile::Wall);
                }
                '.' => {
                    map.set_tile(pos, Tile::Floor);
                }
                'P' => {
                    map.set_tile(pos, Tile::Floor);
                    map.spawn_pos = pos;
                }
                'C' => {
                    map.set_tile(pos, Tile::Campfire);
                }
                'S' => {
                    map.set_tile(pos, Tile::Shrine);
                    map.shrine_pos = Some(pos);
                }
                'E' | '>' => {
                    map.set_tile(pos, Tile::StairsUp);
                    map.stairs_pos = pos;
                }
                '+' => {
                    map.set_tile(pos, Tile::Gate { id: 1, open: false });
                    map.gates.push((1, pos));
                }
                '/' => {
                    map.set_tile(pos, Tile::Gate { id: 1, open: true });
                    map.gates.push((1, pos));
                }
                '_' | 'o' => {
                    map.set_tile(pos, Tile::PressurePlate { id: 1, active: false });
                    map.plates.push((1, pos));
                }
                '~' => {
                    map.set_tile(pos, Tile::Chasm);
                }
                '=' => {
                    map.set_tile(pos, Tile::Bridge);
                }
                'H' => {
                    map.set_tile(pos, Tile::Floor);
                    hof_counter += 1;
                    let (author, msg) = match hof_counter {
                        1 => (
                            "Pioneer",
                            "Echoes of the Ancients: 100 floors ascend into the heavens. Seek the Shrines.",
                        ),
                        2 => (
                            "Ze",
                            "Hall of Fame: Climber Ze forged the Hyaku engine and lit the first Beacon.",
                        ),
                        3 => (
                            "Vanguard",
                            "Guardian Inscription: In unity lies strength. Stand together upon the plates.",
                        ),
                        _ => (
                            "Elder",
                            "Ancient Decree: Only those who claim the shards may challenge the 100th Gate.",
                        ),
                    };
                    map.initial_carvings.push(FloorCarving::new(
                        pos,
                        author.to_string(),
                        msg.to_string(),
                    ));
                }
                ' ' => {
                    map.set_tile(pos, Tile::Wall);
                }
                _ => {
                    map.set_tile(pos, Tile::Floor);
                }
            }
        }
    }

    // Additional atmospheric runestone inscriptions for Floor 1 (placed on adjacent tiles so they never cover plates/campfires)
    if floor_num == 1 {
        map.initial_carvings.push(FloorCarving::new(
            Position::new(39, 25),
            "Watcher".to_string(),
            "Runestone Mechanism: Depress the southern plate [o] to unseal the ceremonial Gateway.".to_string(),
        ));
        map.initial_carvings.push(FloorCarving::new(
            Position::new(10, 14),
            "Wanderer".to_string(),
            "Traveler's Note: Rest by the campfire [*] to heal your spirit before ascending.".to_string(),
        ));
    }

    map
}

/// Generates the giant circular Gatekeeper Boss Arena for every 10th floor (10, 20... 100).
pub fn generate_circular_boss_arena(
    floor_num: u32,
    seed: u64,
    width: i32,
    height: i32,
) -> FloorMap {
    let floor_seed = compute_floor_seed(seed, floor_num);
    let mut _rng = StdRng::seed_from_u64(floor_seed);
    let mut map = FloorMap::new_empty(floor_num, width, height);

    let cx = width / 2;
    let cy = height / 2;
    let rx = (width as f32 / 2.0) - 8.0;
    let ry = (height as f32 / 2.0) - 3.0;

    // 1. Carve circular arena with surrounding abyssal moat
    for y in 0..height {
        for x in 0..width {
            let dx = (x - cx) as f32 / rx;
            let dy = (y - cy) as f32 / ry;
            let d = dx * dx + dy * dy;

            if d <= 0.82 {
                map.set_tile(Position::new(x, y), Tile::Floor);
            } else if d <= 1.0 {
                map.set_tile(Position::new(x, y), Tile::Chasm);
            } else {
                map.set_tile(Position::new(x, y), Tile::Wall);
            }
        }
    }

    // 2. 8 radial tactical stone pillars
    for i in 0..8 {
        let angle = (i as f32) * (std::f32::consts::PI * 2.0 / 8.0);
        let px = cx + ((rx * 0.55) * angle.cos()) as i32;
        let py = cy + ((ry * 0.55) * angle.sin()) as i32;
        map.set_tile(Position::new(px, py), Tile::Wall);
    }

    // 3. Shrine [百] at North, Pressure Plate at South
    let shrine_pos = Position::new(cx, cy - 7);
    map.set_tile(shrine_pos, Tile::Shrine);
    map.shrine_pos = Some(shrine_pos);

    let plate_pos = Position::new(cx, cy + 7);
    map.set_tile(plate_pos, Tile::PressurePlate { id: 1, active: false });
    map.plates.push((1, plate_pos));

    // 4. Safe Spawn point at West + Campfire
    let spawn_x = cx - (rx * 0.70) as i32;
    let spawn_pos = Position::new(spawn_x, cy);
    map.spawn_pos = spawn_pos;
    map.set_tile(spawn_pos, Tile::Floor);

    let campfire_pos = Position::new(spawn_x + 3, cy);
    map.set_tile(campfire_pos, Tile::Campfire);

    // 5. Sealed Ascension Stairs Vault at East
    let alcove_x = cx + (rx * 0.70) as i32;
    let alcove_w = 7;
    let alcove_h = 7;
    let alcove_y = cy - alcove_h / 2;

    for ay in alcove_y..(alcove_y + alcove_h) {
        for ax in alcove_x..(alcove_x + alcove_w) {
            let p = Position::new(ax, ay);
            if ax == alcove_x {
                if ay == cy {
                    map.set_tile(p, Tile::Gate { id: 1, open: false });
                    map.gates.push((1, p));
                } else {
                    map.set_tile(p, Tile::Wall);
                }
            } else if ax == alcove_x + alcove_w - 1 || ay == alcove_y || ay == alcove_y + alcove_h - 1 {
                map.set_tile(p, Tile::Wall);
            } else {
                map.set_tile(p, Tile::Floor);
            }
        }
    }

    let stairs_pos = Position::new(alcove_x + 4, cy);
    map.stairs_pos = stairs_pos;
    map.set_tile(stairs_pos, Tile::StairsUp);

    // 6. Populate Gatekeeper Boss and Minions
    let hp_mult = 1.0 + (floor_num as f32) * 0.20;
    let mut boss = EnemyState::new(1, EnemyKind::FloorBoss, Position::new(cx, cy), hp_mult * 2.0);
    boss.is_boss = true;
    map.initial_enemies.push(boss);

    // Minion escorts
    let mut enemy_id = 2;
    let minion_offsets = [(-4, -2), (4, -2), (-4, 2), (4, 2)];
    for (i, &(ox, oy)) in minion_offsets.iter().enumerate() {
        let kind = if i % 2 == 0 {
            EnemyKind::Brute
        } else {
            EnemyKind::Archer
        };
        let m_pos = Position::new(cx + ox, cy + oy);
        map.initial_enemies
            .push(EnemyState::new(enemy_id, kind, m_pos, hp_mult));
        enemy_id += 1;
    }

    // Atmospheric runestone warning
    map.initial_carvings.push(FloorCarving::new(
        Position::new(spawn_x + 1, cy - 2),
        "Gatekeeper Guardian".to_string(),
        format!(
            "Ascension Trial {}: The Gatekeeper commands the floor. Slay the titan to claim passage.",
            floor_num
        ),
    ));

    map
}

/// Helper function to carve orthogonal corridors between two positions
fn carve_corridor(map: &mut FloorMap, start: Position, end: Position) {
    let mut cur = start;
    while cur.x != end.x {
        map.set_tile(cur, Tile::Floor);
        cur.x += if end.x > cur.x { 1 } else { -1 };
    }
    while cur.y != end.y {
        map.set_tile(cur, Tile::Floor);
        cur.y += if end.y > cur.y { 1 } else { -1 };
    }
    map.set_tile(end, Tile::Floor);
}

/// Generates a rich, non-repetitive procedural floor using 1 of 5 distinct layout archetypes:
/// - Archetype 0: "Crossroads Citadel" (Central Grand Hub connecting 4 cardinal wings)
/// - Archetype 1: "The Twin Trials" (Bifurcated paths: Puzzle route vs Combat route)
/// - Archetype 2: "Serpentine Catacombs" (6 winding interconnected chambers in an S-curve)
/// - Archetype 3: "Sunken Ruin Islands" (Chasm-separated stone plateaus with bridge causeways)
/// - Archetype 4: "Colosseum Perimeter Loop" (Outer ring of chambers with flanking avenues)
pub fn generate_procedural_floor(
    floor_num: u32,
    seed: u64,
    width: i32,
    height: i32,
) -> FloorMap {
    let floor_seed = compute_floor_seed(seed, floor_num);
    let mut rng = StdRng::seed_from_u64(floor_seed);
    let mut map = FloorMap::new_empty(floor_num, width, height);

    // All archetypes terminate at the Eastern Ascension Vault
    let r_vault = Rect::new(
        rng.gen_range((width - 14)..=(width - 12)),
        rng.gen_range(10..=14),
        rng.gen_range(9..=11),
        rng.gen_range(6..=8),
    );
    map.stairs_pos = r_vault.center();
    let gate_pos = Position::new(r_vault.x, r_vault.center().y);
    let approach_pos = Position::new(gate_pos.x - 1, gate_pos.y);

    // Carve vault interior
    for ry in r_vault.y..(r_vault.y + r_vault.h) {
        for rx in r_vault.x..(r_vault.x + r_vault.w) {
            map.set_tile(Position::new(rx, ry), Tile::Floor);
        }
    }

    let archetype = rng.gen_range(0..5);

    let (room_spawn, room_puzzle, room_sanctum, room_guardian) = match archetype {
        0 => {
            // Archetype 0: Crossroads Citadel
            let hub = Rect::new(
                rng.gen_range(28..=32),
                rng.gen_range(9..=11),
                rng.gen_range(14..=18),
                rng.gen_range(8..=10),
            );
            let r_west = Rect::new(
                rng.gen_range(3..=5),
                rng.gen_range(10..=14),
                rng.gen_range(10..=12),
                rng.gen_range(6..=8),
            );
            let r_north = Rect::new(
                rng.gen_range(30..=34),
                rng.gen_range(3..=4),
                rng.gen_range(12..=14),
                rng.gen_range(5..=6),
            );
            let r_south = Rect::new(
                rng.gen_range(30..=34),
                rng.gen_range((height - 8)..=(height - 7)),
                rng.gen_range(12..=14),
                rng.gen_range(5..=6),
            );

            for r in &[hub, r_west, r_north, r_south] {
                for ry in r.y..(r.y + r.h) {
                    for rx in r.x..(r.x + r.w) {
                        map.set_tile(Position::new(rx, ry), Tile::Floor);
                    }
                }
            }

            // Decorative pillars in central hub
            map.set_tile(Position::new(hub.center().x - 2, hub.center().y - 2), Tile::Wall);
            map.set_tile(Position::new(hub.center().x + 2, hub.center().y - 2), Tile::Wall);
            map.set_tile(Position::new(hub.center().x - 2, hub.center().y + 2), Tile::Wall);
            map.set_tile(Position::new(hub.center().x + 2, hub.center().y + 2), Tile::Wall);

            carve_corridor(&mut map, r_west.center(), hub.center());
            carve_corridor(&mut map, hub.center(), r_north.center());
            carve_corridor(&mut map, hub.center(), r_south.center());
            carve_corridor(&mut map, hub.center(), approach_pos);

            (r_west, r_north, r_south, hub)
        }
        1 => {
            // Archetype 1: The Twin Trials (Bifurcated Route)
            let r_spawn = Rect::new(
                rng.gen_range(3..=5),
                rng.gen_range(10..=14),
                rng.gen_range(10..=12),
                rng.gen_range(6..=8),
            );
            let r_puz1 = Rect::new(
                rng.gen_range(20..=24),
                rng.gen_range(3..=4),
                rng.gen_range(12..=14),
                rng.gen_range(5..=6),
            );
            let r_puz2 = Rect::new(
                rng.gen_range(40..=44),
                rng.gen_range(3..=4),
                rng.gen_range(12..=14),
                rng.gen_range(5..=6),
            );
            let r_guard = Rect::new(
                rng.gen_range(24..=30),
                rng.gen_range((height - 9)..=(height - 8)),
                rng.gen_range(14..=16),
                rng.gen_range(6..=7),
            );

            for r in &[r_spawn, r_puz1, r_puz2, r_guard] {
                for ry in r.y..(r.y + r.h) {
                    for rx in r.x..(r.x + r.w) {
                        map.set_tile(Position::new(rx, ry), Tile::Floor);
                    }
                }
            }

            carve_corridor(&mut map, r_spawn.center(), r_puz1.center());
            carve_corridor(&mut map, r_spawn.center(), r_guard.center());
            carve_corridor(&mut map, r_puz1.center(), r_puz2.center());
            carve_corridor(&mut map, r_puz2.center(), approach_pos);
            carve_corridor(&mut map, r_guard.center(), approach_pos);

            (r_spawn, r_puz2, r_guard, r_guard)
        }
        2 => {
            // Archetype 2: Serpentine Catacombs
            let r0 = Rect::new(
                rng.gen_range(3..=5),
                rng.gen_range((height - 9)..=(height - 8)),
                rng.gen_range(10..=12),
                rng.gen_range(5..=6),
            );
            let r1 = Rect::new(
                rng.gen_range(14..=18),
                rng.gen_range(3..=4),
                rng.gen_range(10..=12),
                rng.gen_range(5..=6),
            );
            let r2 = Rect::new(
                rng.gen_range(28..=32),
                rng.gen_range(3..=4),
                rng.gen_range(12..=14),
                rng.gen_range(5..=6),
            );
            let r3 = Rect::new(
                rng.gen_range(28..=32),
                rng.gen_range((height - 9)..=(height - 8)),
                rng.gen_range(12..=14),
                rng.gen_range(5..=6),
            );
            let r4 = Rect::new(
                rng.gen_range(44..=48),
                rng.gen_range(8..=11),
                rng.gen_range(12..=14),
                rng.gen_range(7..=8),
            );

            for r in &[r0, r1, r2, r3, r4] {
                for ry in r.y..(r.y + r.h) {
                    for rx in r.x..(r.x + r.w) {
                        map.set_tile(Position::new(rx, ry), Tile::Floor);
                    }
                }
            }

            carve_corridor(&mut map, r0.center(), r1.center());
            carve_corridor(&mut map, r1.center(), r2.center());
            carve_corridor(&mut map, r2.center(), r3.center());
            carve_corridor(&mut map, r3.center(), r4.center());
            carve_corridor(&mut map, r4.center(), approach_pos);

            (r0, r2, r1, r4)
        }
        3 => {
            // Archetype 3: Sunken Ruin Islands
            let r_spawn = Rect::new(
                rng.gen_range(3..=5),
                rng.gen_range(10..=14),
                rng.gen_range(10..=12),
                rng.gen_range(6..=7),
            );
            let r_isle1 = Rect::new(
                rng.gen_range(20..=24),
                rng.gen_range(9..=12),
                rng.gen_range(11..=13),
                rng.gen_range(6..=7),
            );
            let r_puz = Rect::new(
                rng.gen_range(38..=42),
                rng.gen_range(3..=4),
                rng.gen_range(12..=14),
                rng.gen_range(5..=6),
            );
            let r_guard = Rect::new(
                rng.gen_range(38..=42),
                rng.gen_range((height - 9)..=(height - 8)),
                rng.gen_range(12..=14),
                rng.gen_range(6..=7),
            );

            for r in &[r_spawn, r_isle1, r_puz, r_guard] {
                for ry in r.y..(r.y + r.h) {
                    for rx in r.x..(r.x + r.w) {
                        map.set_tile(Position::new(rx, ry), Tile::Floor);
                    }
                }
            }

            carve_corridor(&mut map, r_spawn.center(), r_isle1.center());
            carve_corridor(&mut map, r_isle1.center(), r_puz.center());
            carve_corridor(&mut map, r_isle1.center(), r_guard.center());
            carve_corridor(&mut map, r_puz.center(), approach_pos);
            carve_corridor(&mut map, r_guard.center(), approach_pos);

            (r_spawn, r_puz, r_isle1, r_guard)
        }
        _ => {
            // Archetype 4: Colosseum Perimeter Loop
            let r0 = Rect::new(3, 10, 11, 8);
            let r1 = Rect::new(20, 3, 12, 6);
            let r2 = Rect::new(42, 3, 12, 6);
            let r3 = Rect::new(20, 20, 12, 6);
            let r4 = Rect::new(42, 20, 12, 6);

            for r in &[r0, r1, r2, r3, r4] {
                for ry in r.y..(r.y + r.h) {
                    for rx in r.x..(r.x + r.w) {
                        map.set_tile(Position::new(rx, ry), Tile::Floor);
                    }
                }
            }

            carve_corridor(&mut map, r0.center(), r1.center());
            carve_corridor(&mut map, r0.center(), r3.center());
            carve_corridor(&mut map, r1.center(), r2.center());
            carve_corridor(&mut map, r3.center(), r4.center());
            carve_corridor(&mut map, r2.center(), approach_pos);
            carve_corridor(&mut map, r4.center(), approach_pos);

            (r0, r2, r3, r4)
        }
    };

    // Final connection into the Ascension Vault through the Gate
    let stairs_pos = map.stairs_pos;
    carve_corridor(&mut map, approach_pos, gate_pos);
    carve_corridor(&mut map, gate_pos, stairs_pos);

    // Seal the vault wall partition along column gate_pos.x
    for vy in r_vault.y..(r_vault.y + r_vault.h) {
        if vy != gate_pos.y {
            map.set_tile(Position::new(gate_pos.x, vy), Tile::Wall);
        }
    }

    // Set Gate tile and ensure it is flanked vertically
    map.set_tile(Position::new(gate_pos.x, gate_pos.y - 1), Tile::Wall);
    map.set_tile(Position::new(gate_pos.x, gate_pos.y + 1), Tile::Wall);
    map.set_tile(gate_pos, Tile::Gate { id: 1, open: false });
    map.gates.push((1, gate_pos));

    // Room 0: Spawn point and Safe Campfire
    map.spawn_pos = Position::new(room_spawn.x + 2, room_spawn.center().y);
    let campfire_pos = room_spawn.center();
    map.set_tile(campfire_pos, Tile::Campfire);

    // Room 1: Stamp 1 of 4 pre-designed puzzle room templates
    let puzzle_tpl = rng.gen_range(0..4);
    let plate_pos = match puzzle_tpl {
        0 => {
            // Template 0: The Chasm Trench & Dual Bridges
            let mid_x = room_puzzle.center().x;
            for py in (room_puzzle.y + 1)..(room_puzzle.y + room_puzzle.h - 1) {
                map.set_tile(Position::new(mid_x, py), Tile::Chasm);
            }
            map.set_tile(Position::new(mid_x, room_puzzle.y + 1), Tile::Bridge);
            map.set_tile(
                Position::new(mid_x, room_puzzle.y + room_puzzle.h - 2),
                Tile::Bridge,
            );
            Position::new(room_puzzle.x + room_puzzle.w - 3, room_puzzle.center().y)
        }
        1 => {
            // Template 1: The Runic Cross
            let cx = room_puzzle.center().x;
            let cy = room_puzzle.center().y;
            for px in (cx - 2)..=(cx + 2) {
                if px != cx {
                    map.set_tile(Position::new(px, cy - 1), Tile::Chasm);
                    map.set_tile(Position::new(px, cy + 1), Tile::Chasm);
                }
            }
            map.set_tile(Position::new(cx - 1, cy - 1), Tile::Bridge);
            map.set_tile(Position::new(cx + 1, cy - 1), Tile::Bridge);
            map.set_tile(Position::new(cx - 1, cy + 1), Tile::Bridge);
            map.set_tile(Position::new(cx + 1, cy + 1), Tile::Bridge);
            Position::new(cx, cy)
        }
        2 => {
            // Template 2: The Pillar Labyrinth
            for py in (room_puzzle.y + 2)..(room_puzzle.y + room_puzzle.h - 2) {
                if py % 2 == 0 {
                    for px in (room_puzzle.x + 2)..(room_puzzle.x + room_puzzle.w - 2) {
                        if px % 3 == 0 {
                            map.set_tile(Position::new(px, py), Tile::Wall);
                        }
                    }
                }
            }
            Position::new(room_puzzle.x + room_puzzle.w - 2, room_puzzle.y + 2)
        }
        _ => {
            // Template 3: The Stepping Stone Causeway
            for py in (room_puzzle.y + 1)..(room_puzzle.y + room_puzzle.h - 1) {
                for px in (room_puzzle.x + 2)..(room_puzzle.x + 4) {
                    map.set_tile(Position::new(px, py), Tile::Chasm);
                }
            }
            map.set_tile(Position::new(room_puzzle.x + 3, room_puzzle.y + 1), Tile::Bridge);
            map.set_tile(
                Position::new(room_puzzle.x + 3, room_puzzle.y + room_puzzle.h - 2),
                Tile::Bridge,
            );
            Position::new(room_puzzle.x + room_puzzle.w - 2, room_puzzle.center().y)
        }
    };

    map.set_tile(plate_pos, Tile::PressurePlate { id: 1, active: false });
    map.plates.push((1, plate_pos));

    // Place second co-op pressure plate for Gate 1 in puzzle room
    let plate_pos2 = Position::new(room_puzzle.x + 2, room_puzzle.y + room_puzzle.h - 3);
    map.set_tile(plate_pos2, Tile::PressurePlate { id: 1, active: false });
    map.plates.push((1, plate_pos2));

    // Room 2: Sanctum Shrine of Renewal [百]
    let shrine_pos = room_sanctum.center();
    map.set_tile(shrine_pos, Tile::Shrine);
    map.shrine_pos = Some(shrine_pos);

    // Room 3: Guardian Hall Hazards (Chasm & Bridge on every 3rd floor)
    if floor_num % 3 == 0 {
        let chasm_x = room_guardian.center().x;
        for cy in room_guardian.y..(room_guardian.y + room_guardian.h) {
            let p = Position::new(chasm_x, cy);
            if map.get_tile(p) == Some(Tile::Floor) {
                map.set_tile(p, Tile::Chasm);
            }
        }
        map.set_tile(Position::new(chasm_x, room_guardian.center().y), Tile::Bridge);
    }

    // Room 4: Ascension Stairs
    map.set_tile(map.stairs_pos, Tile::StairsUp);

    // Populate Enemies with tactical roles
    let mut enemy_id = 1;
    let hp_mult = 1.0 + (floor_num as f32) * 0.08;

    // Puzzle room guard
    let guard_pos1 = Position::new(room_puzzle.x + 2, room_puzzle.center().y);
    map.initial_enemies
        .push(EnemyState::new(enemy_id, EnemyKind::Crawler, guard_pos1, hp_mult));
    enemy_id += 1;

    if rng.gen_bool(0.5) {
        let guard_pos2 = Position::new(room_puzzle.center().x, room_puzzle.y + 1);
        if guard_pos2 != plate_pos {
            map.initial_enemies
                .push(EnemyState::new(enemy_id, EnemyKind::Archer, guard_pos2, hp_mult));
            enemy_id += 1;
        }
    }

    // Sanctum defender
    let sanctum_guard = Position::new(room_sanctum.x + 2, room_sanctum.center().y);
    if sanctum_guard != shrine_pos {
        map.initial_enemies
            .push(EnemyState::new(enemy_id, EnemyKind::Phantom, sanctum_guard, hp_mult));
        enemy_id += 1;
    }

    // Guardian Hall: Elite Floor Guardian
    let guardian_pos = Position::new(room_guardian.center().x + 1, room_guardian.center().y);
    let mut guardian = EnemyState::new(enemy_id, EnemyKind::Brute, guardian_pos, hp_mult * 1.5);
    guardian.is_boss = true;
    map.initial_enemies.push(guardian);
    enemy_id += 1;

    // Guardian minions
    let escort_pos1 = Position::new(room_guardian.x + 2, room_guardian.y + 2);
    map.initial_enemies
        .push(EnemyState::new(enemy_id, EnemyKind::Crawler, escort_pos1, hp_mult));
    enemy_id += 1;

    let escort_pos2 = Position::new(room_guardian.x + 2, room_guardian.y + room_guardian.h - 2);
    map.initial_enemies
        .push(EnemyState::new(enemy_id, EnemyKind::Archer, escort_pos2, hp_mult));
    enemy_id += 1;

    // Stairs Vault guard
    let stairs_guard_pos = Position::new(r_vault.x + 2, r_vault.center().y);
    if stairs_guard_pos != map.stairs_pos {
        map.initial_enemies.push(EnemyState::new(
            enemy_id,
            EnemyKind::Phantom,
            stairs_guard_pos,
            hp_mult,
        ));
    }

    // Guidepost carving near spawn (placed safely 2 tiles from spawn, never on interactive tiles)
    let archetype_name = match archetype {
        0 => "Crossroads Citadel",
        1 => "The Twin Trials",
        2 => "Serpentine Catacombs",
        3 => "Sunken Ruin Islands",
        _ => "Colosseum Loop",
    };

    map.initial_carvings.push(FloorCarving::new(
        Position::new(room_spawn.x + 2, room_spawn.y + 1),
        "Pioneer".to_string(),
        format!(
            "Floor {} [{}]: Step on the yellow plate [o] or defeat the Floor Guardian to unseal the eastern gate.",
            floor_num, archetype_name
        ),
    ));

    map
}

/// Unified map generation entry point.
/// Guarantees that server and client generate the exact same static base map from floor_num and seed.
pub fn generate_floor_map(floor_num: u32, seed: u64) -> FloorMap {
    generate_floor_map_with_size(floor_num, seed, FloorMap::DEFAULT_WIDTH, FloorMap::DEFAULT_HEIGHT)
}

/// Generates a map with custom dimensions.
pub fn generate_floor_map_with_size(
    floor_num: u32,
    seed: u64,
    width: i32,
    height: i32,
) -> FloorMap {
    if floor_num == 1 {
        parse_ascii_map(floor_num, SANCTUARY_LOBBY_ASCII, width, height)
    } else if floor_num % 10 == 0 {
        generate_circular_boss_arena(floor_num, seed, width, height)
    } else {
        generate_procedural_floor(floor_num, seed, width, height)
    }
}

/// Generates a static floor map using the canonical tower seed.
pub fn generate_static_floor_map(floor_num: u32) -> FloorMap {
    generate_floor_map(floor_num, 0x5461_6b65_5f31_3030)
}
