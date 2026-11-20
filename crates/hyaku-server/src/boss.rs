use hyaku_protocol::{BossPhase, EnemyKind, EnemyState, PlayerState, Position, ProjectileState, Tile};
use std::collections::{HashMap, HashSet};
use std::f32::consts::TAU;

const WARNING_TICKS: u16 = 20;
const BOLT_DAMAGE: i32 = 16;
const BLAST_DAMAGE: i32 = 30;

#[derive(Default)]
pub struct BossRaid {
    phase_ticks: u16,
    bolts: Vec<Bolt>,
    warning: HashSet<Position>,
    warning_age: u16,
}

impl BossRaid {
    pub fn step(
        &mut self,
        enemies: &mut [EnemyState],
        players: &mut HashMap<String, PlayerState>,
        tile_at: impl Fn(Position) -> Option<Tile>,
        tick: u64,
    ) {
        let Some(boss) = enemies.iter_mut().find(|e| e.kind == EnemyKind::FloorBoss && e.is_alive()) else {
            self.bolts.clear();
            self.warning.clear();
            return;
        };
        if !players.values().any(PlayerState::is_alive) {
            self.bolts.clear();
            self.warning.clear();
            return;
        }

        self.bolts.retain_mut(|bolt| bolt.advance(players, &tile_at));
        if !self.warning.is_empty() {
            self.warning_age += 1;
            if self.warning_age == WARNING_TICKS {
                for player in players.values_mut() {
                    if player.is_alive() && self.warning.contains(&player.pos) {
                        player.take_damage(BLAST_DAMAGE);
                    }
                }
                self.warning.clear();
            }
        }

        self.phase_ticks = self.phase_ticks.saturating_add(1);
        match boss.boss_phase {
            Some(BossPhase::Barrage) => {
                let age = self.phase_ticks;
                if age == 1 || age % 48 == 0 {
                    self.volley(boss.pos, 16, age as f32 * 0.06);
                } else if age % 6 == 0 {
                    self.volley(boss.pos, 3, age as f32 * 0.13);
                }
                if age >= 144 || (age >= 48 && boss.hp <= boss.max_hp * 2 / 3) {
                    boss.boss_phase = Some(BossPhase::Tremor);
                    self.phase_ticks = 0;
                }
            }
            Some(BossPhase::Tremor) => {
                let age = self.phase_ticks;
                if age % 30 == 1 {
                    self.mark_tiles(boss, players, &tile_at);
                }
                if age >= 120 || (age >= 52 && boss.hp <= boss.max_hp / 3) {
                    boss.boss_phase = Some(BossPhase::Shielded);
                }
            }
            Some(BossPhase::Exposed) => {
                if tick % 60 == 0 {
                    self.volley(boss.pos, 12, (tick % 360) as f32 * 0.04);
                } else if tick % 60 == 30 {
                    self.mark_tiles(boss, players, &tile_at);
                }
            }
            _ => {}
        }
    }

    fn volley(&mut self, pos: Position, count: u16, offset: f32) {
        for spoke in 0..count {
            let angle = offset + TAU * f32::from(spoke) / f32::from(count);
            let (sin, cos) = angle.sin_cos();
            self.bolts.push(Bolt {
                x: pos.x as f32 + 0.5,
                y: pos.y as f32 + 0.5,
                dx: cos * 0.55,
                dy: sin * 0.55,
                remaining: 120,
            });
        }
    }

    fn mark_tiles(
        &mut self,
        boss: &EnemyState,
        players: &HashMap<String, PlayerState>,
        tile_at: &impl Fn(Position) -> Option<Tile>,
    ) {
        if !self.warning.is_empty() {
            return;
        }
        self.warning_age = 0;
        for player in players.values().filter(|p| p.is_alive()) {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let pos = Position::new(player.pos.x + dx, player.pos.y + dy);
                    if !boss.occupies(pos) && tile_at(pos).is_some_and(|tile| tile.is_walkable()) {
                        self.warning.insert(pos);
                    }
                }
            }
        }
    }

    pub fn effects(&self) -> impl Iterator<Item = ProjectileState> + '_ {
        let (glyph, color) = match self.warning_age {
            0..=4 => ('░', (224, 148, 74)),
            5..=9 => ('▒', (244, 134, 62)),
            10..=14 => ('▓', (255, 101, 66)),
            _ => ('█', (255, 68, 55)),
        };
        self.warning.iter().map(move |&pos| ProjectileState::new(pos, glyph, color))
            .chain(self.bolts.iter().map(|bolt| {
                ProjectileState::new(cell(bolt.x, bolt.y), 'o', (255, 170, 80))
            }))
    }
}

struct Bolt {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    remaining: u16,
}

impl Bolt {
    fn advance(
        &mut self,
        players: &mut HashMap<String, PlayerState>,
        tile_at: &impl Fn(Position) -> Option<Tile>,
    ) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        let from = cell(self.x, self.y);
        let to = cell(self.x + self.dx, self.y + self.dy);
        let blocked = |pos| matches!(tile_at(pos), None | Some(Tile::Wall | Tile::Gate { open: false, .. }));

        if blocked(from) || hit_players(players, from) {
            return false;
        }

        if from.x != to.x && from.y != to.y {
            // A diagonal step can cross an intermediate cell before it reaches the next row.
            let edge_x = from.x.max(to.x) as f32;
            let edge_y = from.y.max(to.y) as f32;
            let cross_x = (edge_x - self.x) / self.dx;
            let cross_y = (edge_y - self.y) / self.dy;
            let across_x = Position::new(to.x, from.y);
            let across_y = Position::new(from.x, to.y);
            let clipped = if cross_x < cross_y {
                blocked(across_x) || hit_players(players, across_x)
            } else if cross_y < cross_x {
                blocked(across_y) || hit_players(players, across_y)
            } else {
                blocked(across_x) && blocked(across_y)
            };
            if clipped {
                return false;
            }
        }

        if from != to && (blocked(to) || hit_players(players, to)) {
            return false;
        }
        self.x += self.dx;
        self.y += self.dy;
        true
    }
}

fn cell(x: f32, y: f32) -> Position {
    Position::new(x.floor() as i32, y.floor() as i32)
}

fn hit_players(players: &mut HashMap<String, PlayerState>, pos: Position) -> bool {
    let mut hit = false;
    for player in players.values_mut() {
        if player.is_alive() && player.pos == pos {
            player.take_damage(BOLT_DAMAGE);
            hit = true;
        }
    }
    hit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barrage_rotates_and_tremor_hits_on_tick_twenty() {
        let mut raid = BossRaid::default();
        let pos = Position::new(80, 80);
        let mut player = PlayerState::new("p1".into(), "Alice".into(), 0, pos, 10);
        player.hp = 500;
        player.max_hp = 500;
        let max_hp = player.hp;
        let mut players = HashMap::from([(player.id.clone(), player)]);
        let mut enemies = [EnemyState::new(1, EnemyKind::FloorBoss, Position::new(10, 10), 1.0)];
        let mut advance = |raid: &mut BossRaid, from, to| {
            for tick in from..=to {
                raid.step(&mut enemies, &mut players, |_| Some(Tile::Floor), tick);
            }
            (enemies[0].boss_phase, players["p1"].hp)
        };

        advance(&mut raid, 1, 1);
        assert_eq!(raid.bolts.len(), 16);
        advance(&mut raid, 2, 6);
        assert_eq!(raid.bolts.len(), 19);
        assert!(raid.effects().any(|bolt| bolt.pos != Position::new(10, 10)));
        assert_eq!(advance(&mut raid, 7, 144).0, Some(BossPhase::Tremor));

        for (from, to, glyph) in [(145, 145, '░'), (146, 150, '▒'), (151, 155, '▓'), (156, 164, '█')] {
            assert_eq!(advance(&mut raid, from, to).1, max_hp);
            assert!(raid.effects().any(|effect| effect.pos == pos && effect.glyph == glyph));
        }
        assert_eq!(advance(&mut raid, 165, 165).1, max_hp - BLAST_DAMAGE);
        assert!(!raid.effects().any(|effect| effect.pos == pos));
        assert_eq!(advance(&mut raid, 166, 264).0, Some(BossPhase::Shielded));
    }

    #[test]
    fn bolt_checks_the_cell_crossed_during_a_diagonal_step() {
        let bolt = || Bolt {
            x: 3.9,
            y: 4.8,
            dx: 0.4,
            dy: 0.3,
            remaining: 1,
        };
        let crossed = Position::new(4, 4);
        let player = PlayerState::new("p1".into(), "Alice".into(), 0, crossed, 10);
        let hp = player.hp;

        let mut players = HashMap::from([(player.id.clone(), player)]);
        assert!(!bolt().advance(&mut players, &|_| Some(Tile::Floor)));
        assert_eq!(players["p1"].hp, hp - BOLT_DAMAGE);

        let mut projectile = bolt();
        assert!(!projectile.advance(&mut players, &|pos| {
            Some(if pos == crossed { Tile::Wall } else { Tile::Floor })
        }));
        assert_eq!(players["p1"].hp, hp - BOLT_DAMAGE);
        assert!(!projectile.advance(&mut players, &|_| Some(Tile::Floor)));
    }
}
