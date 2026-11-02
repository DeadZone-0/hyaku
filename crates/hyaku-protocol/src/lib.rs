pub mod direction;
pub mod entity;
pub mod map;
pub mod map_gen;
pub mod packet;
pub mod position;
pub mod tile;

pub use direction::Direction;
pub use entity::{
    get_class_preset, DecoyState, EnemyKind, EnemyState, FloorCarving, PlayerClassPreset, PlayerState,
    ProjectileState, PLAYER_CLASSES,
};
pub use map::{FloorMap, Rect};
pub use map_gen::{
    compute_floor_seed, generate_circular_boss_arena, generate_floor_map,
    generate_floor_map_with_size, generate_procedural_floor, generate_static_floor_map,
    parse_ascii_map, SANCTUARY_LOBBY_ASCII,
};
pub use packet::{ClientPacket, ServerPacket, TickState};
pub use position::Position;
pub use tile::Tile;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direction_properties() {
        assert_eq!(Direction::Up.opposite(), Direction::Down);
        assert_eq!(Direction::Down.opposite(), Direction::Up);
        assert_eq!(Direction::Left.opposite(), Direction::Right);
        assert_eq!(Direction::Right.opposite(), Direction::Left);

        assert_eq!(Direction::Up.delta(), (0, -1));
        assert_eq!(Direction::Down.delta(), (0, 1));
        assert_eq!(Direction::Left.delta(), (-1, 0));
        assert_eq!(Direction::Right.delta(), (1, 0));

        assert_eq!(Direction::from_delta(0, -5), Some(Direction::Up));
        assert_eq!(Direction::from_delta(5, 0), Some(Direction::Right));
    }

    #[test]
    fn test_position_math_and_bounds() {
        let p1 = Position::new(3, 4);
        let p2 = Position::new(7, 1);

        assert_eq!(p1.manhattan_distance(p2), 7);
        assert_eq!(p1.chebyshev_distance(p2), 4);
        assert_eq!(p1.distance_squared(p2), 25);

        let stepped = p1.step(Direction::Up);
        assert_eq!(stepped, Position::new(3, 3));

        assert!(p1.in_bounds(10, 10));
        assert!(!p1.in_bounds(3, 4));
        assert!(!Position::new(-1, 0).in_bounds(10, 10));

        assert_eq!(p1.direction_to(Position::new(10, 4)), Some(Direction::Right));
        assert_eq!(p1.direction_to(Position::new(3, 1)), Some(Direction::Up));

        let line = Position::new(0, 0).line_to(Position::new(3, 0));
        assert_eq!(line.len(), 4);
        assert_eq!(line[0], Position::new(0, 0));
        assert_eq!(line[3], Position::new(3, 0));
    }

    #[test]
    fn test_tile_walkability() {
        assert!(!Tile::Wall.is_walkable());
        assert!(Tile::Floor.is_walkable());
        assert!(!Tile::Chasm.is_walkable());
        assert!(Tile::Bridge.is_walkable());
        assert!(Tile::Campfire.is_walkable());
        assert!(Tile::Shrine.is_walkable());
        assert!(Tile::StairsUp.is_walkable());
        assert!(Tile::PressurePlate { id: 1, active: false }.is_walkable());
        assert!(!Tile::Gate { id: 1, open: false }.is_walkable());
        assert!(Tile::Gate { id: 1, open: true }.is_walkable());
    }

    #[test]
    fn test_player_and_enemy_state() {
        let mut player = PlayerState::new(
            "p1".to_string(),
            "Vanguard".to_string(),
            0,
            Position::new(10, 10),
            1,
        );
        assert!(player.is_alive());
        assert_eq!(player.hp, player.max_hp);

        player.take_damage(30);
        assert_eq!(player.hp, player.max_hp - 30);
        player.heal(15);
        assert_eq!(player.hp, player.max_hp - 15);

        let mut enemy = EnemyState::new(1, EnemyKind::Crawler, Position::new(12, 10), 1.0);
        assert!(enemy.is_alive());
        enemy.take_damage(500);
        assert!(!enemy.is_alive());
        assert_eq!(enemy.hp, 0);
    }

    #[test]
    fn test_packet_json_serialization_roundtrip() {
        let client_packet = ClientPacket::Move(Direction::Right);
        let serialized = client_packet.to_json().expect("serialize client packet");
        let deserialized = ClientPacket::from_json(&serialized).expect("deserialize client packet");
        assert_eq!(client_packet, deserialized);

        let tick_state = TickState {
            tick: 42,
            floor: 1,
            players: vec![PlayerState::new(
                "p1".to_string(),
                "Alice".to_string(),
                0,
                Position::new(5, 5),
                1,
            )],
            enemies: vec![EnemyState::new(1, EnemyKind::Archer, Position::new(8, 8), 1.0)],
            projectiles: vec![ProjectileState::new(Position::new(6, 5), '*', (255, 255, 0))],
            tiles_override: vec![(Position::new(4, 4), Tile::Bridge)],
            carvings: vec![FloorCarving::new(
                Position::new(5, 6),
                "Bob".to_string(),
                "Praise the shrine!".to_string(),
            )],
            tower_shards_bank: 15,
            decoys: vec![DecoyState::new(1, "Alice".to_string(), Position::new(5, 5), 300)],
            shrine_shards: 10,
            shrine_goal: 50,
            stairs_unlocked: false,
        };

        let server_packet = ServerPacket::Tick(tick_state.clone());
        let json = server_packet.to_json().expect("serialize server packet");
        let roundtrip = ServerPacket::from_json(&json).expect("deserialize server packet");
        assert_eq!(server_packet, roundtrip);
    }

    #[test]
    fn test_map_generation_determinism() {
        let seed = 123456789;
        let floor = 7;
        let map_a = FloorMap::generate(seed, floor);
        let map_b = FloorMap::generate(seed, floor);

        assert_eq!(map_a.spawn_pos, map_b.spawn_pos);
        assert_eq!(map_a.stairs_pos, map_b.stairs_pos);
        assert_eq!(map_a.tiles, map_b.tiles);
        assert_eq!(map_a.initial_enemies.len(), map_b.initial_enemies.len());
        assert!(map_a.is_walkable(map_a.spawn_pos));
        assert!(map_a.is_walkable(map_a.stairs_pos));
    }

    #[test]
    fn test_puzzle_gate_blocks_corridor() {
        let map = FloorMap::generate(42, 3);
        if let Some(&(_id, gate_pos)) = map.gates.first() {
            let horiz_blocked = map.get_tile(Position::new(gate_pos.x, gate_pos.y - 1)) == Some(Tile::Wall)
                && map.get_tile(Position::new(gate_pos.x, gate_pos.y + 1)) == Some(Tile::Wall);
            let vert_blocked = map.get_tile(Position::new(gate_pos.x - 1, gate_pos.y)) == Some(Tile::Wall)
                && map.get_tile(Position::new(gate_pos.x + 1, gate_pos.y)) == Some(Tile::Wall);
            assert!(horiz_blocked || vert_blocked, "Gate at {:?} must be flanked by walls", gate_pos);
        }
    }

    #[test]
    fn test_floor_1_sanctuary_lobby() {
        let map = FloorMap::generate(999, 1);
        assert_eq!(map.floor, 1);
        assert!(map.is_walkable(map.spawn_pos));
        assert!(map.is_walkable(map.stairs_pos));
        assert_eq!(map.initial_enemies.len(), 0, "Floor 1 sanctuary must be peaceful with zero enemies");
        assert!(map.shrine_pos.is_some(), "Floor 1 must contain the central Shrine");
        assert_eq!(map.get_tile(map.shrine_pos.unwrap()), Some(Tile::Shrine));
        assert!(map.initial_carvings.len() >= 4, "Floor 1 must have Hall of Fame runestone carvings");

        // Verify Hall of Fame entries
        let has_ze = map.initial_carvings.iter().any(|c| c.message.contains("Ze"));
        let has_ancients = map.initial_carvings.iter().any(|c| c.message.contains("Ancients") || c.message.contains("floors"));
        assert!(has_ze && has_ancients);

        // Verify gate and plate
        assert!(!map.gates.is_empty(), "Floor 1 has ceremonial barrier gate");
        assert!(!map.plates.is_empty(), "Floor 1 has runestone pressure plate");

        // Verify pressure plate is not obscured by a carving
        let plate_pos = map.plates[0].1;
        assert!(
            map.initial_carvings.iter().all(|c| c.pos != plate_pos),
            "Pressure plate at {:?} must not be obscured by a carving",
            plate_pos
        );
    }

    #[test]
    fn test_circular_boss_arena_every_10th_floor() {
        for floor in [10, 20, 50, 100] {
            let map = FloorMap::generate(777, floor);
            assert_eq!(map.floor, floor);
            assert!(map.is_walkable(map.spawn_pos));
            assert!(map.is_walkable(map.stairs_pos));
            assert!(map.shrine_pos.is_some());

            // Floor boss must be present
            let boss = map.initial_enemies.iter().find(|e| e.is_boss);
            assert!(boss.is_some(), "Floor {} must feature an elite FloorBoss", floor);
            assert_eq!(boss.unwrap().kind, EnemyKind::FloorBoss);

            // Minions must be present
            assert!(map.initial_enemies.len() >= 4, "Floor {} must have minion escorts", floor);

            // Chasm ring present
            let has_chasm = map.tiles.iter().any(|t| *t == Tile::Chasm);
            assert!(has_chasm, "Boss arena on floor {} must have an abyssal chasm moat", floor);

            // Gate present and flanked by walls
            assert!(!map.gates.is_empty());
            let (_id, gate_pos) = map.gates[0];
            let horiz_blocked = map.get_tile(Position::new(gate_pos.x, gate_pos.y - 1)) == Some(Tile::Wall)
                && map.get_tile(Position::new(gate_pos.x, gate_pos.y + 1)) == Some(Tile::Wall);
            assert!(horiz_blocked, "Boss arena stairs gate must be flanked by walls");
        }
    }

    #[test]
    fn test_floors_2_to_99_procedural_generation() {
        for floor in [2, 3, 4, 5, 8, 15, 23, 77, 99] {
            let map = FloorMap::generate(12345, floor);
            assert_eq!(map.floor, floor);
            assert!(map.is_walkable(map.spawn_pos));
            assert!(map.is_walkable(map.stairs_pos));
            assert!(map.shrine_pos.is_some());
            assert!(!map.gates.is_empty());
            assert!(!map.plates.is_empty());

            // Elite guardian must be present
            let guardian = map.initial_enemies.iter().find(|e| e.is_boss);
            assert!(guardian.is_some(), "Floor {} must have an elite guardian", floor);

            // Corridors & gates must flank
            let (_id, gate_pos) = map.gates[0];
            let horiz_blocked = map.get_tile(Position::new(gate_pos.x, gate_pos.y - 1)) == Some(Tile::Wall)
                && map.get_tile(Position::new(gate_pos.x, gate_pos.y + 1)) == Some(Tile::Wall);
            assert!(horiz_blocked, "Ascension gate on floor {} must be flanked by walls", floor);
        }
    }

    #[test]
    fn test_static_floor_map_generation() {
        let static_f1 = generate_static_floor_map(1);
        let static_f10 = generate_static_floor_map(10);
        let static_f42 = generate_static_floor_map(42);

        assert_eq!(static_f1.floor, 1);
        assert_eq!(static_f10.floor, 10);
        assert_eq!(static_f42.floor, 42);

        // Deterministic repeat
        assert_eq!(static_f42, generate_static_floor_map(42));
    }
}
