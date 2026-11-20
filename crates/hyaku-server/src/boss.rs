use hyaku_protocol::{BossPhase, EnemyKind, EnemyState, PlayerState, Position, ProjectileState, RaidBoss, Tile};
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
        let Some(target) = players.values().filter(|p| p.is_alive())
            .min_by_key(|p| (p.pos.manhattan_distance(boss.pos), &p.id))
            .map(|p| p.pos)
        else {
            self.bolts.clear();
            self.warning.clear();
            return;
        };

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
                self.fire(boss, target, age);
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
                if age >= 52 && boss.hp <= boss.max_hp / 3 {
                    boss.boss_phase = Some(BossPhase::Shielded);
                    self.phase_ticks = 0;
                    self.bolts.clear();
                    self.warning.clear();
                }
            }
            Some(BossPhase::Shielded) => self.phase_ticks = 0,
            Some(BossPhase::Exposed) => {
                if self.phase_ticks <= 60 {
                    return;
                }
                let beat = (tick % 90) as u16;
                if beat < 24 {
                    self.fire(boss, target, beat + 1);
                } else if beat == 30 {
                    self.mark_tiles(boss, players, &tile_at);
                }
            }
            _ => {}
        }
    }

    fn fire(&mut self, boss: &EnemyState, target: Position, age: u16) {
        use RaidBoss::*;

        let pos = boss.pos;
        let aim = ((target.y - pos.y) as f32).atan2((target.x - pos.x) as f32);
        let spin = f32::from(age) * 0.13;
        let burst = age == 1 || age % 48 == 0;
        let pulse = age == 1 || age % 12 == 0;
        let quarter = TAU / 4.0;
        match boss.raid_boss.unwrap_or(AshWarden) {
            AshWarden if burst => self.volley(pos, 16, f32::from(age) * 0.06, 0.55),
            AshWarden if age % 6 == 0 => self.volley(pos, 3, spin, 0.55),
            BellKeeper if pulse => {
                let turn = f32::from(age / 12 % 2) * quarter / 2.0;
                self.volley(pos, 4, turn, 0.38);
                self.volley(pos, 4, turn, 0.72);
            }
            MireMatron if pulse => self.fan(pos, aim, 7, 0.23, 0.38),
            ChainKnight if pulse => {
                let lane = f32::from(age / 12 % 2) * quarter;
                for angle in [lane, lane + TAU / 2.0] {
                    self.fan(pos, angle, 3, 0.09, 0.74);
                }
            }
            GlassRegent if burst => {
                self.volley(pos, 12, 0.0, 0.42);
                self.volley(pos, 12, TAU / 24.0, 0.72);
            }
            GlassRegent if age % 12 == 6 => self.volley(pos, 6, -spin, 0.68),
            HollowHuntsman if (age - 1) % 24 < 9 && age % 4 == 1 => {
                self.fan(pos, aim, 3, 0.11, 0.90);
            }
            LanternSaint if age == 1 || age % 8 == 0 => {
                self.volley(pos, 3, spin, 0.44);
                self.volley(pos, 3, -spin + quarter, 0.60);
            }
            FrostReaver if pulse => {
                for turn in [-quarter, quarter] {
                    self.fan(pos, aim + turn, 5, 0.27, 0.66);
                }
            }
            CinderColossus if burst => self.volley(pos, 24, spin, 0.33),
            CinderColossus if age % 12 == 0 => self.fan(pos, aim, 5, 0.30, 0.63),
            AshSovereign if burst => {
                self.volley(pos, 24, spin, 0.40);
                self.volley(pos, 8, spin + TAU / 48.0, 0.68);
            }
            AshSovereign if age % 18 == 0 => self.fan(pos, aim, 7, 0.16, 0.82),
            AshSovereign if age % 6 == 0 => self.volley(pos, 4, -spin, 0.62),
            _ => {}
        }
    }

    fn volley(&mut self, pos: Position, count: u16, offset: f32, speed: f32) {
        for spoke in 0..count {
            let angle = offset + TAU * f32::from(spoke) / f32::from(count);
            self.shoot(pos, angle, speed);
        }
    }

    fn fan(&mut self, pos: Position, angle: f32, count: i32, spread: f32, speed: f32) {
        for spoke in -(count / 2)..=(count / 2) {
            self.shoot(pos, angle + spoke as f32 * spread, speed);
        }
    }

    fn shoot(&mut self, pos: Position, angle: f32, speed: f32) {
        let (sin, cos) = angle.sin_cos();
        self.bolts.push(Bolt {
            x: pos.x as f32 + 0.5,
            y: pos.y as f32 + 0.5,
            dx: cos * speed,
            dy: sin * speed,
            remaining: 120,
        });
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
        use RaidBoss::*;
        let identity = boss.raid_boss.unwrap_or(AshWarden);
        for player in players.values().filter(|p| p.is_alive()) {
            for dy in -4_i32..=4 {
                for dx in -4_i32..=4 {
                    let (x, y) = (dx.abs(), dy.abs());
                    let marked = match identity {
                        AshWarden => x <= 1 && y <= 1,
                        BellKeeper => (x == 0 || y == 0) && x + y <= 3,
                        MireMatron => x + y <= 3,
                        ChainKnight => y <= 1,
                        GlassRegent => x == y && x <= 3,
                        HollowHuntsman => x <= 1 && y <= 4,
                        LanternSaint => matches!(x + y, 0 | 3),
                        FrostReaver => x <= 2 && y <= 2 && (x + y) % 2 == 0,
                        CinderColossus => x <= 1 || y <= 1,
                        AshSovereign => x + y <= 5 && (x <= 1 || y <= 1 || x + y == 5),
                    };
                    let pos = Position::new(player.pos.x + dx, player.pos.y + dy);
                    if marked && !boss.occupies(pos) && tile_at(pos).is_some_and(|tile| tile.is_walkable()) {
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
    fn each_raid_has_a_distinct_volley_and_warning() {
        let mut volleys = HashSet::new();
        let mut warnings = HashSet::new();
        let center = Position::new(10, 10);
        let target = Position::new(20, 20);
        let player = PlayerState::new("p1".into(), "Alice".into(), 0, target, 10);
        let players = HashMap::from([(player.id.clone(), player)]);

        for floor in (10..=100).step_by(10) {
            let mut boss = EnemyState::new(1, EnemyKind::FloorBoss, center, 1.0);
            boss.raid_boss = RaidBoss::for_floor(floor);
            let mut raid = BossRaid::default();
            raid.fire(&boss, target, 1);
            let velocities: Vec<_> = raid.bolts.iter().map(|bolt| {
                (bolt.dx.to_bits(), bolt.dy.to_bits())
            }).collect();
            assert!(!velocities.is_empty());
            assert!(volleys.insert(velocities), "reused volley on floor {floor}");

            raid.mark_tiles(&boss, &players, &|_| Some(Tile::Floor));
            let mut tiles: Vec<_> = raid.warning.iter().map(|pos| (pos.x, pos.y)).collect();
            tiles.sort_unstable();
            assert!(raid.warning.contains(&target));
            assert!(warnings.insert(tiles), "reused warning on floor {floor}");
        }
    }

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
        assert_eq!(advance(&mut raid, 166, 264).0, Some(BossPhase::Tremor));
        let threshold = enemies[0].max_hp / 3;
        enemies[0].take_damage(enemies[0].hp);
        assert_eq!(enemies[0].hp, threshold);
        raid.step(&mut enemies, &mut players, |_| Some(Tile::Floor), 265);
        assert_eq!(enemies[0].boss_phase, Some(BossPhase::Shielded));
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
