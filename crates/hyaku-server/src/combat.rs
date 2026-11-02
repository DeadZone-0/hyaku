use hyaku_protocol::{
    Direction, EnemyKind, EnemyState, FloorMap, PlayerState, Position, ProjectileState, Tile,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::{HashMap, HashSet};

/// Maximum distance (in tiles) allowed for an active Co-op Lightning Tether
pub const TETHER_MAX_RANGE: u32 = 8;

/// Continuous electric damage dealt each tick to any enemy intersecting the tether beam
pub const TETHER_DAMAGE_PER_TICK: i32 = 6;

/// A single tile in the 3-tile directional melee sweep arc
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashTile {
    pub pos: Position,
    pub glyph: char,
}

/// Computes the 3-tile directional melee sweep arc based on player facing direction:
/// - Up:    (x-1, y-1) '╱', (x, y-1) '─', (x+1, y-1) '╲'
/// - Down:  (x-1, y+1) '╲', (x, y+1) '─', (x+1, y+1) '╱'
/// - Left:  (x-1, y-1) '╲', (x-1, y) '│', (x-1, y+1) '╱'
/// - Right: (x+1, y-1) '╱', (x+1, y) '│', (x+1, y+1) '╲'
pub fn compute_directional_slash_arc(origin: Position, dir: Direction) -> [SlashTile; 3] {
    match dir {
        Direction::Up => [
            SlashTile {
                pos: Position::new(origin.x - 1, origin.y - 1),
                glyph: '╱',
            },
            SlashTile {
                pos: Position::new(origin.x, origin.y - 1),
                glyph: '─',
            },
            SlashTile {
                pos: Position::new(origin.x + 1, origin.y - 1),
                glyph: '╲',
            },
        ],
        Direction::Down => [
            SlashTile {
                pos: Position::new(origin.x - 1, origin.y + 1),
                glyph: '╲',
            },
            SlashTile {
                pos: Position::new(origin.x, origin.y + 1),
                glyph: '─',
            },
            SlashTile {
                pos: Position::new(origin.x + 1, origin.y + 1),
                glyph: '╱',
            },
        ],
        Direction::Left => [
            SlashTile {
                pos: Position::new(origin.x - 1, origin.y - 1),
                glyph: '╲',
            },
            SlashTile {
                pos: Position::new(origin.x - 1, origin.y),
                glyph: '│',
            },
            SlashTile {
                pos: Position::new(origin.x - 1, origin.y + 1),
                glyph: '╱',
            },
        ],
        Direction::Right => [
            SlashTile {
                pos: Position::new(origin.x + 1, origin.y - 1),
                glyph: '╱',
            },
            SlashTile {
                pos: Position::new(origin.x + 1, origin.y),
                glyph: '│',
            },
            SlashTile {
                pos: Position::new(origin.x + 1, origin.y + 1),
                glyph: '╲',
            },
        ],
    }
}

/// Bresenham's integer line algorithm connecting two positions
pub fn bresenham_line(start: Position, end: Position) -> Vec<Position> {
    let mut points = Vec::new();
    let dx = (end.x - start.x).abs();
    let dy = (end.y - start.y).abs();
    let sx = if start.x < end.x { 1 } else { -1 };
    let sy = if start.y < end.y { 1 } else { -1 };
    let mut err = dx - dy;

    let mut cur = start;
    while cur != end {
        points.push(cur);
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            cur.x += sx;
        }
        if e2 < dx {
            err += dx;
            cur.y += sy;
        }
    }
    points.push(end);
    points
}

/// Result of executing a directional melee slash attack
#[derive(Debug, Default)]
pub struct SlashResult {
    pub hit_count: usize,
    pub total_damage: i32,
    pub shards_dropped: u32,
    pub announcements: Vec<String>,
    pub slash_projectiles: Vec<(ProjectileState, u8)>,
}

/// Executes a directional melee sweep (`Space` key) across the 3 arc tiles
pub fn execute_directional_slash(
    player_pos: Position,
    player_name: &str,
    dir: Direction,
    class_idx: usize,
    map_width: i32,
    map_height: i32,
    enemies: &mut Vec<EnemyState>,
    tick_counter: u64,
) -> SlashResult {
    let class_preset = hyaku_protocol::get_class_preset(class_idx);
    let base_dmg = class_preset.attack_power;
    let arc_tiles = compute_directional_slash_arc(player_pos, dir);

    let mut hit_count = 0;
    let mut total_damage = 0;
    let mut shards_dropped = 0;
    let mut announcements = Vec::new();
    let mut slash_projectiles = Vec::new();

    // 1. Spawn directional slash arc projectiles (lasts 3 ticks on clients)
    for tile in &arc_tiles {
        if tile.pos.in_bounds(map_width, map_height) {
            slash_projectiles.push((
                ProjectileState::new(tile.pos, tile.glyph, class_preset.color_rgb),
                3,
            ));
        }
    }

    // 2. Check collisions with enemies in the sweep arc
    for tile in &arc_tiles {
        if !tile.pos.in_bounds(map_width, map_height) {
            continue;
        }

        for enemy in enemies.iter_mut() {
            if enemy.is_alive() && enemy.pos == tile.pos {
                hit_count += 1;

                // Critical roll: 25% chance
                let roll = ((tick_counter.wrapping_mul(7919))
                    ^ ((tile.pos.x as u64).wrapping_mul(104729))
                    ^ ((tile.pos.y as u64).wrapping_mul(31337))
                    ^ (enemy.id as u64).wrapping_mul(61))
                    % 100;
                let is_crit = roll < 25;
                let dmg = if is_crit {
                    (base_dmg as f32 * 1.75).round() as i32
                } else {
                    base_dmg
                };

                enemy.take_damage(dmg);
                total_damage += dmg;

                let enemy_name = enemy.kind.name();
                if !enemy.is_alive() {
                    let dropped = if enemy.is_boss {
                        10
                    } else if enemy.kind == EnemyKind::Brute {
                        4
                    } else {
                        2
                    };
                    shards_dropped += dropped;

                    // Drop Aether Shards (*) at death position
                    slash_projectiles.push((
                        ProjectileState::new(tile.pos, '*', (255, 215, 0)),
                        8,
                    ));

                    announcements.push(format!(
                        ":: SLAIN :: {} swept through {}! (+{} Aether Shards dropped)",
                        player_name, enemy_name, dropped
                    ));
                } else if is_crit {
                    announcements.push(format!(
                        ":: CRIT :: {}'s directional sweep struck {} for {} DMG!",
                        player_name, enemy_name, dmg
                    ));
                }
            }
        }
    }

    SlashResult {
        hit_count,
        total_damage,
        shards_dropped,
        announcements,
        slash_projectiles,
    }
}

/// Result of continuous Lightning Tether execution during a single tick
#[derive(Debug, Default)]
pub struct TetherTickResult {
    pub total_tether_damage: i32,
    pub announcements: Vec<String>,
    pub spark_projectiles: Vec<(ProjectileState, u8)>,
    pub player_shards_earned: HashMap<String, u32>,
}

/// Processes continuous electric damage and spark particles for Co-op Lightning Tethers (`[Q]` key).
/// Links players or decoys within 8 tiles and zaps any enemy intersecting the Bresenham line.
pub fn process_continuous_tether_tick(
    players: &mut HashMap<String, PlayerState>,
    decoys: &[hyaku_protocol::DecoyState],
    enemies: &mut Vec<EnemyState>,
    tick_counter: u64,
) -> TetherTickResult {
    let mut total_tether_damage = 0;
    let mut announcements = Vec::new();
    let mut spark_projectiles = Vec::new();
    let mut shards_earned: HashMap<String, u32> = HashMap::new();

    let mut processed_pairs: HashSet<(String, String)> = HashSet::new();

    // Collect tether pairs
    let active_tethers: Vec<(String, String, Position, String)> = players
        .values()
        .filter(|p| p.is_alive() && p.tether_target.is_some())
        .map(|p| (p.id.clone(), p.tether_target.clone().unwrap(), p.pos, p.username.clone()))
        .collect();

    for (p1_id, p2_id, p1_pos, p1_name) in active_tethers {
        let pair_key = if p1_id < p2_id {
            (p1_id.clone(), p2_id.clone())
        } else {
            (p2_id.clone(), p1_id.clone())
        };

        if processed_pairs.contains(&pair_key) {
            continue;
        }
        processed_pairs.insert(pair_key);

        // Target can be either another player or a player's Holo-Decoy (enables solo playtesting)
        let target_info = if let Some(p2) = players.get(&p2_id) {
            if p2.is_alive() && p1_pos.manhattan_distance(p2.pos) <= TETHER_MAX_RANGE {
                Some((p2.pos, p2.username.clone(), true))
            } else {
                None
            }
        } else if p2_id.starts_with("decoy-") {
            let decoy_id = p2_id.trim_start_matches("decoy-").parse::<u64>().ok();
            decoys
                .iter()
                .find(|d| Some(d.id) == decoy_id && d.remaining_ticks > 0 && p1_pos.manhattan_distance(d.pos) <= TETHER_MAX_RANGE)
                .map(|d| (d.pos, "Holo-Decoy §".to_string(), false))
        } else {
            None
        };

        if let Some((p2_pos, p2_name, is_other_player)) = target_info {
            let line_points = bresenham_line(p1_pos, p2_pos);

            for (idx, &pt) in line_points.iter().enumerate() {
                if pt == p1_pos || pt == p2_pos {
                    continue;
                }

                // Electric spark particle along the beam (single-width '↯', '⠶', '*')
                let spark_glyph = match (tick_counter + idx as u64) % 3 {
                    0 => '↯',
                    1 => '⠶',
                    _ => '*',
                };
                spark_projectiles.push((
                    ProjectileState::new(pt, spark_glyph, (120, 240, 255)),
                    1,
                ));

                // Deal continuous electric damage each tick to intersecting enemies
                for enemy in enemies.iter_mut() {
                    if enemy.is_alive() && enemy.pos == pt {
                        enemy.take_damage(TETHER_DAMAGE_PER_TICK);
                        total_tether_damage += TETHER_DAMAGE_PER_TICK;

                        if !enemy.is_alive() {
                            let dropped = if enemy.is_boss { 10 } else { 2 };
                            *shards_earned.entry(p1_id.clone()).or_insert(0) += dropped;
                            if is_other_player {
                                *shards_earned.entry(p2_id.clone()).or_insert(0) += dropped;
                            }

                            // Drop Aether Shards (*) at death position
                            spark_projectiles.push((
                                ProjectileState::new(pt, '*', (255, 215, 0)),
                                6,
                            ));

                            announcements.push(format!(
                                ":: AETHERIC ELECTROCUTION :: @{} & {} vaporized {} with the Lightning Tether! (+{} Shards dropped)",
                                p1_name, p2_name, enemy.kind.name(), dropped
                            ));
                        }
                    }
                }
            }
        }
    }

    // Award shards directly to tethered players
    for (pid, shards) in &shards_earned {
        if let Some(player) = players.get_mut(pid) {
            player.shards += *shards;
        }
    }

    TetherTickResult {
        total_tether_damage,
        announcements,
        spark_projectiles,
        player_shards_earned: shards_earned,
    }
}

/// Updates Enemy AI (Patrol, Chase, Contact Damage) on the 20-TPS loop
pub fn update_enemy_ai(
    enemies: &mut Vec<EnemyState>,
    players: &mut HashMap<String, PlayerState>,
    decoys: &[hyaku_protocol::DecoyState],
    map: &FloorMap,
    tiles_override: &HashMap<Position, Tile>,
    tick_counter: u64,
) -> (Vec<(ProjectileState, u8)>, Vec<String>) {
    let mut projectiles = Vec::new();
    let mut announcements = Vec::new();

    // AI logic evaluates every 10 ticks (0.5s)
    if tick_counter % 10 != 0 || players.is_empty() {
        return (projectiles, announcements);
    }

    // (enemy_pos, target_pos, damage, attacker_name)
    let mut attacks: Vec<(Position, Position, i32, String)> = Vec::new();

    for enemy in enemies.iter_mut() {
        if !enemy.is_alive() {
            continue;
        }

        // 1. Target evaluation: Check Holo-Decoys first, then nearest alive player
        let mut closest_target: Option<Position> = None;
        let mut min_dist = u32::MAX;

        for decoy in decoys {
            let d = enemy.pos.manhattan_distance(decoy.pos);
            if d < min_dist && d <= 10 {
                min_dist = d;
                closest_target = Some(decoy.pos);
            }
        }

        if closest_target.is_none() {
            for player in players.values() {
                if !player.is_alive() {
                    continue;
                }
                let dist = enemy.pos.manhattan_distance(player.pos);
                if dist < min_dist {
                    min_dist = dist;
                    closest_target = Some(player.pos);
                }
            }
        }

        // 2. Chase vs Patrol
        if let Some(target) = closest_target {
            if min_dist <= 8 {
                // CHASE MODE: Player/decoy is within aggro radius
                if min_dist == 1 {
                    // Contact damage
                    let dmg = match enemy.kind {
                        EnemyKind::Crawler => 8,
                        EnemyKind::Archer => 10,
                        EnemyKind::Brute => 20,
                        EnemyKind::Phantom => 15,
                        EnemyKind::FloorBoss => 28,
                    };
                    attacks.push((enemy.pos, target, dmg, enemy.kind.name().to_string()));
                } else if enemy.kind == EnemyKind::Archer && min_dist >= 2 && min_dist <= 5 {
                    // Archer ranged projectile: spawn arrow along the path
                    let arrow_glyph = if target.x > enemy.pos.x {
                        '»'
                    } else if target.x < enemy.pos.x {
                        '«'
                    } else if target.y > enemy.pos.y {
                        '▼'
                    } else {
                        '▲'
                    };
                    let mid_pos = if let Some(dir) = enemy.pos.direction_to(target) {
                        enemy.pos.step(dir)
                    } else {
                        target
                    };
                    projectiles.push((
                        ProjectileState::new(mid_pos, arrow_glyph, (255, 200, 60)),
                        2,
                    ));
                    attacks.push((enemy.pos, target, 12, "Archer arrow".to_string()));
                } else {
                    // Step toward target
                    if let Some(step_dir) = enemy.pos.direction_to(target) {
                        let next_pos = enemy.pos.step(step_dir);
                        if is_tile_walkable(map, tiles_override, next_pos) {
                            enemy.pos = next_pos;
                        }
                    }
                }
            } else {
                // PATROL MODE: Wander in a random cardinal direction
                let mut rng = StdRng::seed_from_u64(tick_counter.wrapping_add(enemy.id as u64));
                let random_dir = Direction::ALL[rng.gen_range(0..4)];
                let next_pos = enemy.pos.step(random_dir);
                if is_tile_walkable(map, tiles_override, next_pos) {
                    enemy.pos = next_pos;
                }
            }
        }
    }

    // Apply queued attacks / contact damage to players
    for (enemy_pos, target_pos, dmg, attacker_name) in attacks {
        // Dramatic attack strike particle at enemy and target positions
        projectiles.push((
            ProjectileState::new(enemy_pos, 'X', (255, 60, 60)),
            2,
        ));
        projectiles.push((
            ProjectileState::new(target_pos, '!', (255, 60, 60)),
            2,
        ));

        for player in players.values_mut() {
            if player.is_alive() && player.pos == target_pos {
                player.take_damage(dmg);
                announcements.push(format!(
                    ":: CONTACT DAMAGE :: {} struck {} for {} DMG! HP: {}/{}",
                    attacker_name, player.username, dmg, player.hp, player.max_hp
                ));
                break;
            }
        }
    }

    (projectiles, announcements)
}

/// Spawns floor monsters periodically to maintain active monsters on the floor
pub fn spawn_floor_monsters_if_needed(
    enemies: &mut Vec<EnemyState>,
    map: &FloorMap,
    tiles_override: &HashMap<Position, Tile>,
    floor_num: u32,
    tick_counter: u64,
    min_monsters: usize,
) {
    if floor_num % 10 == 0 {
        // Boss floors don't respawn extra trash monsters
        return;
    }

    // Check every 200 ticks (10 seconds)
    if tick_counter % 200 != 0 {
        return;
    }

    let alive_count = enemies.iter().filter(|e| e.is_alive()).count();
    if alive_count >= min_monsters {
        return;
    }

    let mut rng = StdRng::seed_from_u64(tick_counter.wrapping_mul(41).wrapping_add(floor_num as u64));
    let next_id = enemies.iter().map(|e| e.id).max().unwrap_or(0) + 1;

    // Pick a candidate position in the floor bounds
    for _ in 0..20 {
        let rx = rng.gen_range(2..(map.width - 2));
        let ry = rng.gen_range(2..(map.height - 2));
        let candidate = Position::new(rx, ry);

        if is_tile_walkable(map, tiles_override, candidate)
            && candidate.chebyshev_distance(map.spawn_pos) > 6
            && !enemies.iter().any(|e| e.is_alive() && e.pos == candidate)
        {
            let kind = if floor_num >= 15 && rng.gen_bool(0.25) {
                EnemyKind::Brute
            } else if floor_num >= 8 && rng.gen_bool(0.35) {
                EnemyKind::Archer
            } else {
                EnemyKind::Crawler
            };

            let hp_mult = 1.0 + (floor_num as f32) * 0.15;
            enemies.push(EnemyState::new(next_id, kind, candidate, hp_mult));
            break;
        }
    }
}

/// Helper to check walkability of a position
fn is_tile_walkable(
    map: &FloorMap,
    tiles_override: &HashMap<Position, Tile>,
    pos: Position,
) -> bool {
    if !pos.in_bounds(map.width, map.height) {
        return false;
    }
    if let Some(t) = tiles_override.get(&pos) {
        t.is_walkable()
    } else if let Some(t) = map.get_tile(pos) {
        t.is_walkable()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_directional_slash_arc_glyphs_and_positions() {
        let origin = Position::new(10, 10);

        // Up: (9, 9) '╱', (10, 9) '─', (11, 9) '╲'
        let up_arc = compute_directional_slash_arc(origin, Direction::Up);
        assert_eq!(up_arc[0].pos, Position::new(9, 9));
        assert_eq!(up_arc[0].glyph, '╱');
        assert_eq!(up_arc[1].pos, Position::new(10, 9));
        assert_eq!(up_arc[1].glyph, '─');
        assert_eq!(up_arc[2].pos, Position::new(11, 9));
        assert_eq!(up_arc[2].glyph, '╲');

        // Down: (9, 11) '╲', (10, 11) '─', (11, 11) '╱'
        let down_arc = compute_directional_slash_arc(origin, Direction::Down);
        assert_eq!(down_arc[0].pos, Position::new(9, 11));
        assert_eq!(down_arc[0].glyph, '╲');
        assert_eq!(down_arc[1].pos, Position::new(10, 11));
        assert_eq!(down_arc[1].glyph, '─');
        assert_eq!(down_arc[2].pos, Position::new(11, 11));
        assert_eq!(down_arc[2].glyph, '╱');

        // Left: (9, 9) '╲', (9, 10) '│', (9, 11) '╱'
        let left_arc = compute_directional_slash_arc(origin, Direction::Left);
        assert_eq!(left_arc[0].pos, Position::new(9, 9));
        assert_eq!(left_arc[0].glyph, '╲');
        assert_eq!(left_arc[1].pos, Position::new(9, 10));
        assert_eq!(left_arc[1].glyph, '│');
        assert_eq!(left_arc[2].pos, Position::new(9, 11));
        assert_eq!(left_arc[2].glyph, '╱');

        // Right: (11, 9) '╱', (11, 10) '│', (11, 11) '╲'
        let right_arc = compute_directional_slash_arc(origin, Direction::Right);
        assert_eq!(right_arc[0].pos, Position::new(11, 9));
        assert_eq!(right_arc[0].glyph, '╱');
        assert_eq!(right_arc[1].pos, Position::new(11, 10));
        assert_eq!(right_arc[1].glyph, '│');
        assert_eq!(right_arc[2].pos, Position::new(11, 11));
        assert_eq!(right_arc[2].glyph, '╲');
    }

    #[test]
    fn test_bresenham_line_algorithm() {
        let p0 = Position::new(2, 2);
        let p1 = Position::new(6, 2);
        let line = bresenham_line(p0, p1);

        assert_eq!(line.len(), 5);
        assert_eq!(line[0], Position::new(2, 2));
        assert_eq!(line[4], Position::new(6, 2));

        let diag = bresenham_line(Position::new(0, 0), Position::new(3, 3));
        assert_eq!(diag.len(), 4);
        assert_eq!(diag[1], Position::new(1, 1));
        assert_eq!(diag[2], Position::new(2, 2));
    }

    #[test]
    fn test_execute_directional_slash_cleaves_enemies_and_drops_shards() {
        let origin = Position::new(10, 10);
        let mut enemies = vec![
            EnemyState::new(1, EnemyKind::Crawler, Position::new(9, 9), 1.0),
            EnemyState::new(2, EnemyKind::Crawler, Position::new(10, 9), 1.0),
            EnemyState::new(3, EnemyKind::Crawler, Position::new(11, 9), 1.0),
        ];

        // Strike upwards in melee sweep
        let res = execute_directional_slash(
            origin,
            "ZeHero",
            Direction::Up,
            0, // Striker
            50,
            50,
            &mut enemies,
            100,
        );

        assert_eq!(res.hit_count, 3, "Sweep arc must hit all 3 enemies in front");
        assert!(res.total_damage > 0);
        assert_eq!(res.slash_projectiles.len() >= 3, true);

        // Verify slash arc glyphs generated
        assert!(res.slash_projectiles.iter().any(|(p, _)| p.glyph == '╲'));
        assert!(res.slash_projectiles.iter().any(|(p, _)| p.glyph == '─'));
        assert!(res.slash_projectiles.iter().any(|(p, _)| p.glyph == '╱'));
    }

    #[test]
    fn test_continuous_lightning_tether_damage_and_drops() {
        let mut players = HashMap::new();
        let mut p1 = PlayerState::new("p1".to_string(), "Alice".to_string(), 0, Position::new(5, 5), 1);
        p1.tether_target = Some("p2".to_string());
        let p2 = PlayerState::new("p2".to_string(), "Bob".to_string(), 1, Position::new(9, 5), 1);

        players.insert("p1".to_string(), p1);
        players.insert("p2".to_string(), p2);

        // Enemy standing directly on the line at (7, 5) with 5 HP (so 1 tick kills it)
        let mut enemy = EnemyState::new(10, EnemyKind::Crawler, Position::new(7, 5), 1.0);
        enemy.hp = 5;
        let mut enemies = vec![enemy];

        let result = process_continuous_tether_tick(&mut players, &[], &mut enemies, 1);

        assert_eq!(result.total_tether_damage, TETHER_DAMAGE_PER_TICK);
        assert!(!enemies[0].is_alive(), "Enemy must be slain by tether electric damage");

        // Verify Aether Shards (*) drop particle
        assert!(result.spark_projectiles.iter().any(|(p, _)| p.pos == Position::new(7, 5) && p.glyph == '*'));

        // Verify players earned dropped shards
        assert_eq!(players.get("p1").unwrap().shards, 2);
        assert_eq!(players.get("p2").unwrap().shards, 2);
    }

    #[test]
    fn test_tether_to_holo_decoy_solo() {
        let mut players = HashMap::new();
        let mut p1 = PlayerState::new("p1".to_string(), "Alice".to_string(), 0, Position::new(5, 5), 1);
        p1.tether_target = Some("decoy-42".to_string());
        players.insert("p1".to_string(), p1);

        let decoy = hyaku_protocol::DecoyState::new(42, "Alice".to_string(), Position::new(9, 5), 300);
        let decoys = vec![decoy];

        let mut enemy = EnemyState::new(10, EnemyKind::Crawler, Position::new(7, 5), 1.0);
        enemy.hp = 5;
        let mut enemies = vec![enemy];

        let result = process_continuous_tether_tick(&mut players, &decoys, &mut enemies, 1);

        assert_eq!(result.total_tether_damage, TETHER_DAMAGE_PER_TICK);
        assert!(!enemies[0].is_alive(), "Enemy must be slain by tether between player and decoy");
        assert_eq!(players.get("p1").unwrap().shards, 2, "Solo player receives all dropped shards");
    }
}
