pub mod direction;
pub mod entity;
pub mod map;
pub mod packet;
pub mod position;
pub mod tile;

pub use direction::Direction;
pub use entity::{
    get_class_preset, EnemyKind, EnemyState, FloorCarving, PlayerClassPreset, PlayerState,
    ProjectileState, PLAYER_CLASSES,
};
pub use map::{FloorMap, Rect};
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
}
