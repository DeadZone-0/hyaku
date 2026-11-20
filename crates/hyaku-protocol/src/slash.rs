use crate::{Direction, EnemyState, Position, Tile, BOW_RANGE, RANGER_CLASS};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashTile {
    pub pos: Position,
    pub glyph: char,
}

pub fn basic_attack_tiles(
    class_idx: usize,
    origin: Position,
    facing: Direction,
    enemies: &[EnemyState],
    tile_at: impl Fn(Position) -> Option<Tile>,
) -> Vec<SlashTile> {
    if class_idx != RANGER_CLASS {
        return slash_tiles(origin, facing, enemies, tile_at);
    }
    let mut tiles = Vec::new();
    let mut pos = origin;
    for _ in 0..BOW_RANGE {
        pos = pos.step(facing);
        if matches!(tile_at(pos), None | Some(Tile::Wall | Tile::Gate { open: false, .. })) {
            break;
        }
        tiles.push(SlashTile { pos, glyph: '·' });
        if enemies.iter().any(|enemy| enemy.is_alive() && enemy.occupies(pos)) {
            break;
        }
    }
    if let Some(last) = tiles.last_mut() {
        last.glyph = facing.arrow_glyph();
    }
    tiles
}

fn compute_directional_slash_arc(origin: Position, facing: Direction) -> [SlashTile; 4] {
    let front = origin.step(facing);
    let (dx, dy, glyphs) = match facing {
        Direction::Up => (1, 0, ['╱', '─', '╲']),
        Direction::Down => (1, 0, ['╲', '─', '╱']),
        Direction::Left => (0, 1, ['╲', '│', '╱']),
        Direction::Right => (0, 1, ['╱', '│', '╲']),
    };
    [
        SlashTile { pos: front.add_offset(-dx, -dy), glyph: glyphs[0] },
        SlashTile { pos: front, glyph: glyphs[1] },
        SlashTile { pos: front.add_offset(dx, dy), glyph: glyphs[2] },
        SlashTile { pos: front.step(facing), glyph: glyphs[1] },
    ]
}

pub fn slash_tiles(
    origin: Position,
    facing: Direction,
    enemies: &[EnemyState],
    tile_at: impl Fn(Position) -> Option<Tile>,
) -> Vec<SlashTile> {
    let clear = |pos| !matches!(tile_at(pos), None | Some(Tile::Wall | Tile::Gate { open: false, .. }));
    let clear_path = |pos: Position| {
        if !clear(pos) {
            return false;
        }
        let dx = pos.x - origin.x;
        let dy = pos.y - origin.y;
        if dx != 0 && dy != 0 {
            clear(origin.add_offset(dx, 0)) || clear(origin.add_offset(0, dy))
        } else if dx.abs() + dy.abs() > 1 {
            clear(origin.add_offset(dx.signum(), dy.signum()))
        } else {
            true
        }
    };
    let has_enemy = |pos| enemies.iter().any(|enemy| enemy.is_alive() && enemy.occupies(pos));
    let sweep = |dir| {
        compute_directional_slash_arc(origin, dir).into_iter().filter(|tile| clear_path(tile.pos))
    };

    let mut tiles: Vec<_> = sweep(facing).collect();
    if has_enemy(origin) {
        tiles.push(SlashTile { pos: origin, glyph: '✦' });
    } else if !tiles.iter().any(|tile| has_enemy(tile.pos)) {
        // Keep swings in front. Only turn if the swing would miss.
        if let Some(dir) = origin.all_neighbors().into_iter()
            .find(|&pos| clear_path(pos) && has_enemy(pos))
            .and_then(|pos| origin.direction_to(pos))
        {
            tiles = sweep(dir).collect();
        }
    }
    tiles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_directional_slash_arc_glyphs_and_positions() {
        let origin = Position::new(10, 10);

        let up_arc = compute_directional_slash_arc(origin, Direction::Up);
        assert_eq!(up_arc[0].pos, Position::new(9, 9));
        assert_eq!(up_arc[0].glyph, '╱');
        assert_eq!(up_arc[1].pos, Position::new(10, 9));
        assert_eq!(up_arc[1].glyph, '─');
        assert_eq!(up_arc[2].pos, Position::new(11, 9));
        assert_eq!(up_arc[2].glyph, '╲');

        let down_arc = compute_directional_slash_arc(origin, Direction::Down);
        assert_eq!(down_arc[0].pos, Position::new(9, 11));
        assert_eq!(down_arc[0].glyph, '╲');
        assert_eq!(down_arc[1].pos, Position::new(10, 11));
        assert_eq!(down_arc[1].glyph, '─');
        assert_eq!(down_arc[2].pos, Position::new(11, 11));
        assert_eq!(down_arc[2].glyph, '╱');

        let left_arc = compute_directional_slash_arc(origin, Direction::Left);
        assert_eq!(left_arc[0].pos, Position::new(9, 9));
        assert_eq!(left_arc[0].glyph, '╲');
        assert_eq!(left_arc[1].pos, Position::new(9, 10));
        assert_eq!(left_arc[1].glyph, '│');
        assert_eq!(left_arc[2].pos, Position::new(9, 11));
        assert_eq!(left_arc[2].glyph, '╱');

        // Right
        let right_arc = compute_directional_slash_arc(origin, Direction::Right);
        assert_eq!(right_arc[0].pos, Position::new(11, 9));
        assert_eq!(right_arc[0].glyph, '╱');
        assert_eq!(right_arc[1].pos, Position::new(11, 10));
        assert_eq!(right_arc[1].glyph, '│');
        assert_eq!(right_arc[2].pos, Position::new(11, 11));
        assert_eq!(right_arc[2].glyph, '╲');
    }

    #[test]
    fn slash_assists_only_nearby_targets_and_respects_walls() {
        use crate::EnemyKind;

        let origin = Position::new(10, 10);
        let front = origin.step(Direction::Up);
        let reach = front.step(Direction::Up);
        let behind = origin.step(Direction::Down);
        let far_behind = behind.step(Direction::Down);
        let enemy = |pos| EnemyState::new(1, EnemyKind::Crawler, pos, 1.0);

        let tiles = slash_tiles(origin, Direction::Up, &[enemy(reach), enemy(behind)], |_| Some(Tile::Floor));
        assert!(tiles.iter().any(|tile| tile.pos == reach));
        assert!(!tiles.iter().any(|tile| tile.pos == behind));

        let tiles = slash_tiles(origin, Direction::Up, &[enemy(behind)], |_| Some(Tile::Floor));
        assert!(tiles.iter().any(|tile| tile.pos == behind));

        let tiles = slash_tiles(origin, Direction::Up, &[enemy(far_behind)], |_| Some(Tile::Floor));
        assert!(!tiles.iter().any(|tile| tile.pos == far_behind));

        let tiles = slash_tiles(origin, Direction::Up, &[enemy(reach)], |pos| {
            Some(if pos == front { Tile::Gate { id: 1, open: false } } else { Tile::Floor })
        });
        assert!(!tiles.iter().any(|tile| tile.pos == reach));

        let tiles = slash_tiles(origin, Direction::Up, &[enemy(origin)], |_| Some(Tile::Floor));
        assert!(tiles.iter().any(|tile| tile.pos == origin));
    }
}
