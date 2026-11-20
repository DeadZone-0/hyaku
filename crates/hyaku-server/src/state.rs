use crate::boss::BossRaid;
use crate::db::{Database, FloorProgress, SavedCarving, SavedFloor, SavedLocation};
use hyaku_protocol::{
    ClientPacket, DecoyState, Direction, EnemyKind, EnemyState, FloorCarving, FloorMap, PlayerState,
    ProjectileState, RaidBoss, ResumeKey, ServerPacket, TickState, Tile, Position,
};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedSender;

/// Sanitizes user-supplied text to prevent ANSI escape sequence injection, terminal hijacking, and buffer bloat.
pub fn sanitize_text(input: &str, max_chars: usize) -> String {
    input
        .chars()
        .filter(|c| !c.is_control() && *c != '\x1b')
        .take(max_chars)
        .collect::<String>()
        .trim()
        .to_string()
}

fn resume_id(name: &str, key: &ResumeKey) -> Option<String> {
    if key.0.len() != 64 || !key.0.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    let mut hash = Sha256::new();
    hash.update(b"hyaku saved location v1\0");
    hash.update(name.as_bytes());
    hash.update([0]);
    hash.update(key.0.as_bytes());
    Some(format!("key:{:x}", hash.finalize()))
}

const LOCATION_CHECKPOINT: Duration = Duration::from_secs(5);

pub struct FloorInstance {
    pub floor: u32,
    raid: BossRaid,
    practice_lobby: bool,
    pub map: FloorMap,
    pub players: HashMap<String, PlayerState>,
    pub enemies: Vec<EnemyState>,
    pub projectiles: Vec<(ProjectileState, u8)>,
    pub tiles_override: HashMap<Position, Tile>,
    pub carvings: Vec<FloorCarving>,
    pub tick_counter: u64,
    pub player_campfires: HashMap<String, Position>,
    pub decoys: Vec<DecoyState>,
    pub next_decoy_id: u64,
    pub shrine_shards: u32,
    pub shrine_goal: u32,
    pub stairs_unlocked: bool,
    pub unlocked_gates: HashSet<u8>,
    pub pending_announcements: Vec<String>,
}

impl FloorInstance {
    pub fn new(floor: u32, seed: u64) -> Self {
        let map = FloorMap::generate(seed, floor);
        let enemies = map.initial_enemies.clone();
        let mut tiles_override = HashMap::new();

        // Copy initial dynamic tiles into override map
        for &(id, pos) in &map.gates {
            tiles_override.insert(pos, Tile::Gate { id, open: false });
        }
        for &(id, pos) in &map.plates {
            tiles_override.insert(pos, Tile::PressurePlate { id, active: false });
        }

        let (shrine_goal, stairs_unlocked) = if floor == 1 {
            (0, true)
        } else if floor % 10 == 0 {
            (0, false)
        } else {
            (10 + (floor - 2) * 5, false)
        };

        Self {
            floor,
            raid: BossRaid::default(),
            practice_lobby: false,
            map: map.clone(),
            players: HashMap::new(),
            enemies,
            projectiles: Vec::new(),
            tiles_override,
            carvings: map.initial_carvings.clone(),
            tick_counter: 0,
            player_campfires: HashMap::new(),
            decoys: Vec::new(),
            next_decoy_id: 1,
            shrine_shards: 0,
            shrine_goal,
            stairs_unlocked,
            unlocked_gates: HashSet::new(),
            pending_announcements: Vec::new(),
        }
    }

    fn progress(&self) -> FloorProgress {
        FloorProgress {
            shrine_shards: self.shrine_shards,
            stairs_unlocked: self.stairs_unlocked,
            unlocked_gates: self.unlocked_gates.clone(),
        }
    }

    fn restore(&mut self, saved: &SavedFloor, reached_next_floor: bool) {
        self.shrine_shards = saved.progress.shrine_shards;
        self.stairs_unlocked |= saved.progress.stairs_unlocked || reached_next_floor;
        if reached_next_floor {
            self.shrine_shards = self.shrine_shards.max(self.shrine_goal);
        }
        for &(id, pos) in &self.map.gates {
            if saved.progress.unlocked_gates.contains(&id) || reached_next_floor {
                self.tiles_override.insert(pos, Tile::Gate { id, open: true });
                self.unlocked_gates.insert(id);
            }
        }
        if self.floor % 10 == 0 && self.stairs_unlocked {
            self.enemies.retain(|enemy| enemy.kind != EnemyKind::FloorBoss);
        }

        for &(pos, tile) in &saved.structures {
            if matches!((self.map.get_tile(pos), tile),
                (Some(Tile::Chasm), Tile::Bridge) | (Some(Tile::Floor), Tile::Campfire))
            {
                self.tiles_override.insert(pos, tile);
            }
        }
        self.player_campfires.extend(saved.campfires_by_builder.iter()
            .filter(|(_, pos)| self.tiles_override.get(pos) == Some(&Tile::Campfire))
            .map(|(name, &pos)| (name.clone(), pos)));

        for entry in &saved.carvings {
            if self.map.get_tile(entry.carving.pos) != Some(Tile::Floor) {
                continue;
            }
            if let Some(current) = self.carvings.iter_mut().find(|c| c.pos == entry.carving.pos) {
                *current = entry.carving.clone();
            } else {
                self.carvings.push(entry.carving.clone());
            }
        }
    }

    fn respawn_position(&self, username: &str) -> Position {
        self.player_campfires.get(&username.to_lowercase())
            .copied()
            .unwrap_or(self.map.spawn_pos)
    }

    fn saved_location(&self, player: &PlayerState) -> SavedLocation {
        let pos = if player.is_alive() { player.pos } else { self.respawn_position(&player.username) };
        SavedLocation { floor: self.floor, pos, temper: player.temper }
    }

    fn can_rejoin_at(&self, pos: Position) -> bool {
        self.is_walkable(pos) && !self.enemies.iter().any(|enemy| {
            enemy.kind == EnemyKind::FloorBoss && enemy.is_alive() && enemy.occupies(pos)
        })
    }

    pub fn drain_announcements(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_announcements)
    }

    pub fn check_pressure_plates(&mut self) -> Vec<String> {
        let mut announcements = Vec::new();

        // Check each plate on the map and update its active state
        for &(gate_id, plate_pos) in &self.map.plates {
            let occupied = self.players.values().any(|p| p.is_alive() && p.pos == plate_pos)
                || self.decoys.iter().any(|d| d.pos == plate_pos);

            let current_active = matches!(
                self.tiles_override.get(&plate_pos),
                Some(Tile::PressurePlate { active: true, .. })
            );

            if occupied != current_active {
                self.tiles_override.insert(
                    plate_pos,
                    Tile::PressurePlate {
                        id: gate_id,
                        active: occupied,
                    },
                );
            }
        }

        // Group plates by gate_id
        let mut plates_by_gate: HashMap<u8, Vec<Position>> = HashMap::new();
        for &(gid, pos) in &self.map.plates {
            plates_by_gate.entry(gid).or_default().push(pos);
        }

        for (gid, plate_positions) in plates_by_gate {
            if plate_positions.is_empty() {
                continue;
            }

            let all_held = plate_positions.iter().all(|pos| {
                self.players.values().any(|p| p.is_alive() && p.pos == *pos)
                    || self.decoys.iter().any(|d| d.pos == *pos)
            });

            if all_held && !self.unlocked_gates.contains(&gid) {
                self.unlocked_gates.insert(gid);

                for &(gate_id, gate_pos) in &self.map.gates {
                    if gate_id == gid {
                        self.tiles_override
                            .insert(gate_pos, Tile::Gate { id: gate_id, open: true });
                    }
                }

                let plate_count = plate_positions.len();
                let msg = if plate_count > 1 {
                    format!(
                        "* [CO-OP] All {} Pressure Plates for Gate #{} are held down! The Barrier Gate is UNSEALED! *",
                        plate_count, gid
                    )
                } else {
                    format!(
                        "* [CO-OP] Pressure Plate #{} held down! The Barrier Gate is UNSEALED! *",
                        gid
                    )
                };
                announcements.push(msg);
            }
        }

        announcements
    }

    pub fn get_effective_tile(&self, pos: Position) -> Option<Tile> {
        Self::resolve_tile(&self.map, &self.tiles_override, pos)
    }

    pub fn is_walkable(&self, pos: Position) -> bool {
        self.get_effective_tile(pos).map_or(false, |t| t.is_walkable())
    }

    fn resolve_tile(
        map: &FloorMap,
        tiles_override: &HashMap<Position, Tile>,
        pos: Position,
    ) -> Option<Tile> {
        tiles_override.get(&pos).copied().or_else(|| map.get_tile(pos))
    }

    pub fn add_player(&mut self, player: PlayerState) {
        self.players.insert(player.id.clone(), player);
    }

    pub fn remove_player(&mut self, player_id: &str) -> Option<PlayerState> {
        self.players.remove(player_id)
    }

    #[allow(dead_code)]
    pub fn move_player(&mut self, player_id: &str, dir: Direction) -> bool {
        self.move_player_with_events(player_id, dir).0
    }

    pub fn move_player_with_events(
        &mut self,
        player_id: &str,
        dir: Direction,
    ) -> (bool, Option<String>) {
        let (current_pos, is_alive, username) = match self.players.get(player_id) {
            Some(p) => (p.pos, p.is_alive(), p.username.clone()),
            None => return (false, None),
        };

        if !is_alive {
            return (false, None);
        }

        let target_pos = current_pos.step(dir);
        if !target_pos.in_bounds(self.map.width, self.map.height) {
            return (false, None);
        }

        if !self.is_walkable(target_pos)
            || self.enemies.iter().any(|e| {
                e.kind == EnemyKind::FloorBoss && e.is_alive() && e.occupies(target_pos)
            })
        {
            return (false, None);
        }

        if let Some(player) = self.players.get_mut(player_id) {
            player.pos = target_pos;
        }

        let mut event = None;

        let plate_events = self.check_pressure_plates();
        if !plate_events.is_empty() {
            event = Some(plate_events.join("\n"));
        } else if let Some(Tile::PressurePlate { id, .. }) = self.get_effective_tile(target_pos) {
            event = Some(format!(
                "* [CO-OP] @{} stepped on Pressure Plate #{}! *",
                username, id
            ));
        }

        (true, event)
    }

    pub fn hit_enemy_at(
        &mut self,
        target_pos: Position,
        base_dmg: i32,
        crit_chance: u32,
        knockback_dir: Option<Direction>,
        attacker_name: &str,
        hit_gatekeeper: &mut bool,
    ) -> Option<(i32, bool, u32, Vec<String>)> {
        let enemy_idx = self
            .enemies
            .iter()
            .position(|e| {
                e.is_alive() && e.occupies(target_pos)
                    && (e.kind != EnemyKind::FloorBoss || !*hit_gatekeeper)
            })?;
        if self.enemies[enemy_idx].kind == EnemyKind::FloorBoss {
            *hit_gatekeeper = true;
        }

        let enemy_id = self.enemies[enemy_idx].id;
        let enemy_name = self.enemies[enemy_idx].name();
        let is_boss = self.enemies[enemy_idx].is_boss;

        let roll = ((self.tick_counter.wrapping_mul(7919))
            ^ ((target_pos.x as u64).wrapping_mul(104729))
            ^ ((target_pos.y as u64).wrapping_mul(31337))
            ^ (enemy_id as u64).wrapping_mul(61))
            % 100;
        let is_crit = (roll as u32) < crit_chance;
        let actual_dmg = if is_crit {
            (base_dmg as f32 * 1.75).round() as i32
        } else {
            base_dmg
        };

        let dealt = self.enemies[enemy_idx].take_damage(actual_dmg);
        let actual_dmg = if self.enemies[enemy_idx].kind == EnemyKind::FloorBoss {
            dealt
        } else {
            actual_dmg
        };
        if actual_dmg == 0 {
            return None;
        }

        let particle_glyph = if is_crit { '!' } else { '*' };
        let particle_rgb = if is_crit {
            (255, 230, 40)
        } else {
            (255, 120, 60)
        };
        self.projectiles.push((
            ProjectileState::new(target_pos, particle_glyph, particle_rgb),
            3,
        ));

        let mut events = Vec::new();
        let mut extra_dmg = 0;

        if self.enemies[enemy_idx].is_alive() && !is_boss {
            if let Some(dir) = knockback_dir {
                let pushed_pos = target_pos.step(dir);
                let tile = self.get_effective_tile(pushed_pos);

                match tile {
                    Some(Tile::Chasm) => {
                        self.enemies[enemy_idx].hp = 0;
                        self.projectiles.push((
                            ProjectileState::new(pushed_pos, 'v', (180, 50, 255)),
                            4,
                        ));
                        events.push(format!(
                            ":: ABYSS EXECUTION :: {} knocked {} into the void chasm!",
                            attacker_name, enemy_name
                        ));
                    }
                    Some(Tile::Wall) | Some(Tile::Gate { open: false, .. }) => {
                        extra_dmg += 8;
                        self.enemies[enemy_idx].take_damage(8);
                        self.projectiles.push((
                            ProjectileState::new(pushed_pos, '#', (200, 200, 220)),
                            2,
                        ));
                        events.push(format!(
                            ":: WALL SLAM :: {} slammed {} into the stone wall (+8 DMG)!",
                            attacker_name, enemy_name
                        ));
                    }
                    _ => {
                        let occupied = self
                            .enemies
                            .iter()
                            .position(|e| e.is_alive() && e.occupies(pushed_pos));
                        if let Some(other_idx) = occupied {
                            extra_dmg += 5;
                            self.enemies[enemy_idx].take_damage(5);
                            if self.enemies[other_idx].kind != EnemyKind::FloorBoss {
                                self.enemies[other_idx].take_damage(5);
                            }
                            events.push(format!(
                                ":: FOE COLLISION :: {} crashed into another foe (+5 DMG)!",
                                enemy_name
                            ));
                        } else if self.is_walkable(pushed_pos) {
                            self.enemies[enemy_idx].pos = pushed_pos;
                        }
                    }
                }
            }
        }

        let total_dmg = actual_dmg + extra_dmg;
        let dead = !self.enemies[enemy_idx].is_alive();
        let shards = if dead {
            if is_boss {
                10
            } else if self.enemies[enemy_idx].kind == EnemyKind::Brute {
                4
            } else {
                2
            }
        } else {
            0
        };

        if dead {
            events.push(format!(
                ":: SLAIN :: {} vanquished {}! (+{} Shards)",
                attacker_name, enemy_name, shards
            ));
        } else if is_crit {
            events.push(format!(
                ":: CRIT :: {} struck {} for {} DMG!",
                attacker_name, enemy_name, actual_dmg
            ));
        }

        Some((total_dmg, is_crit, shards, events))
    }

    pub fn step_tick(&mut self, tower_shards: u32) -> TickState {
        self.tick_counter = self.tick_counter.wrapping_add(1);

        // Decoy timer decay (lasts remaining_ticks, decrement each tick)
        self.decoys.retain_mut(|d| {
            if d.remaining_ticks > 0 {
                d.remaining_ticks -= 1;
                d.remaining_ticks > 0
            } else {
                false
            }
        });

        // Check pressure plates and record any unseal announcements
        let plate_events = self.check_pressure_plates();
        self.pending_announcements.extend(plate_events);

        for player in self.players.values_mut().filter(|p| p.is_alive()) {
            let tile_at = |pos| Self::resolve_tile(&self.map, &self.tiles_override, pos);
            if self.tick_counter % 20 == 0 && tile_at(player.pos) == Some(Tile::Shrine) {
                player.heal(player.max_hp);
                player.restore_mana(player.max_mana);
            } else if (-2..=2).any(|dy| {
                (-2..=2).any(|dx| tile_at(player.pos.add_offset(dx, dy)) == Some(Tile::Campfire))
            }) {
                player.heal(1);
                if self.tick_counter % 20 == 0 {
                    player.restore_mana(6);
                }
            }
        }

        if !self.practice_lobby {
            let (ai_projectiles, ai_announcements) = crate::combat::update_enemy_ai(
                &mut self.enemies,
                &mut self.players,
                &self.decoys,
                &self.map,
                &self.tiles_override,
                self.tick_counter,
            );
            self.projectiles.extend(ai_projectiles);
            self.pending_announcements.extend(ai_announcements);

            crate::combat::spawn_floor_monsters_if_needed(
                &mut self.enemies,
                &self.map,
                &self.tiles_override,
                self.floor,
                self.tick_counter,
                4,
            );
        }

        if self.floor % 10 == 0 {
            self.raid.step(
                &mut self.enemies,
                &mut self.players,
                |pos| Self::resolve_tile(&self.map, &self.tiles_override, pos),
                self.tick_counter,
            );
        }

        // 3. Process Co-op Lightning Tether Continuous Electric Damage (each tick)
        let tether_result = crate::combat::process_continuous_tether_tick(
            &mut self.players,
            &self.decoys,
            &mut self.enemies,
            self.tick_counter,
            RaidBoss::for_floor(self.floor).is_some(),
        );
        self.projectiles.extend(tether_result.spark_projectiles);
        self.pending_announcements.extend(tether_result.announcements);

        // Projectile decay (lasts specified TTL ticks)
        self.projectiles.retain_mut(|(proj, ttl)| {
            if *ttl > 0 && proj.pos.in_bounds(self.map.width, self.map.height) {
                *ttl -= 1;
                true
            } else {
                false
            }
        });

        let fallen_boss = self.enemies.iter().find(|e| {
            e.kind == EnemyKind::FloorBoss && !e.is_alive()
        }).map(EnemyState::name);
        self.enemies.retain(|e| e.is_alive());

        if let Some(name) = fallen_boss {
            self.raid = BossRaid::default();
            self.stairs_unlocked = true;
            let next = if self.map.gates.iter().all(|(id, _)| self.unlocked_gates.contains(id)) {
                "The stair lies open."
            } else {
                "Hold both plates to open the stair gate."
            };
            self.pending_announcements.push(format!("{name} has fallen. {next}"));
        }

        TickState {
            tick: self.tick_counter,
            floor: self.floor,
            players: self.players.values().cloned().collect(),
            enemies: self.enemies.clone(),
            projectiles: self.projectiles.iter()
                .map(|(p, _)| p.clone())
                .chain(self.raid.effects())
                .collect(),
            tiles_override: self.tiles_override.iter().map(|(&p, &t)| (p, t)).collect(),
            carvings: self.carvings.clone(),
            tower_shards_bank: tower_shards,
            decoys: self.decoys.clone(),
            shrine_shards: self.shrine_shards,
            shrine_goal: self.shrine_goal,
            stairs_unlocked: self.stairs_unlocked,
        }
    }
}

pub struct ServerState {
    pub clients: HashMap<String, UnboundedSender<ServerPacket>>,
    pub client_floors: HashMap<String, u32>,
    pub floors: HashMap<u32, FloorInstance>,
    pub world_seed: u64,
    pub tower_shards_bank: u32,
    db: Database,
    progress_retry: Option<Instant>,
    next_location_checkpoint: Instant,
    pending_locations: HashMap<String, SavedLocation>,
    resume_ids: HashMap<String, String>,
    demo_spawns: Option<[Position; 2]>,
    raid_start_floor: Option<u32>,
}

impl ServerState {
    pub fn new(seed: u64) -> Self {
        Self::with_database(Database::memory(seed), seed)
    }

    pub fn load(seed: u64, save_path: PathBuf) -> io::Result<Self> {
        Ok(Self::with_database(Database::load(save_path, seed)?, seed))
    }

    fn with_database(db: Database, seed: u64) -> Self {
        let mut state = Self {
            clients: HashMap::new(),
            client_floors: HashMap::new(),
            floors: HashMap::new(),
            world_seed: db.saved().world_seed.unwrap_or(seed),
            tower_shards_bank: db.saved().tower_shards_bank,
            db,
            progress_retry: None,
            next_location_checkpoint: Instant::now() + LOCATION_CHECKPOINT,
            pending_locations: HashMap::new(),
            resume_ids: HashMap::new(),
            demo_spawns: None,
            raid_start_floor: None,
        };
        state.get_or_create_floor(1);
        state
    }

    pub fn raid_practice(seed: u64, floor: u32) -> Self {
        let mut db = Database::memory(seed);
        db.update(|save| save.highest_unlocked_floor = floor)
            .expect("practice save is in memory");
        let mut state = Self::with_database(db, seed);
        state.raid_start_floor = Some(floor);
        state
    }

    pub fn enable_demo_lobby(&mut self) {
        let floor = self.get_or_create_floor(1);
        let plates = &floor.map.plates;
        let left = plates[0].1.add_offset(0, -1);
        let right = plates[1].1.add_offset(0, -1);
        let target = Position::new((left.x + right.x) / 2, left.y);
        floor.practice_lobby = true;
        floor.enemies.push(EnemyState::new(1, EnemyKind::Crawler, target, 2.0));
        self.demo_spawns = Some([left, right]);
    }

    pub fn get_or_create_floor(&mut self, floor: u32) -> &mut FloorInstance {
        self.floors.entry(floor).or_insert_with(|| {
            let mut instance = FloorInstance::new(floor, self.world_seed);
            let completed = floor < self.db.saved().highest_unlocked_floor;
            if let Some(saved) = self.db.saved().floors.get(&floor) {
                instance.restore(saved, completed);
            } else if completed {
                instance.restore(&SavedFloor::default(), true);
            }
            instance
        })
    }

    pub fn sync_progress(&mut self) {
        let now = Instant::now();
        if self.progress_retry.is_some_and(|at| now < at) {
            return;
        }
        if now >= self.next_location_checkpoint {
            self.next_location_checkpoint = now + LOCATION_CHECKPOINT;
            for floor in self.floors.values() {
                self.pending_locations.extend(floor.players.values().filter_map(|player| {
                    let id = self.resume_ids.get(&player.id)?;
                    Some((id.clone(), floor.saved_location(player)))
                }));
            }
        }
        self.pending_locations.retain(|name, location| {
            self.db.saved().last_locations.get(name) != Some(location)
        });
        let changed: Vec<_> = self.floors.iter().filter_map(|(&id, floor)| {
            let progress = floor.progress();
            let changed = match self.db.saved().floors.get(&id) {
                Some(saved) => saved.progress != progress,
                None => progress != FloorProgress::default(),
            };
            changed.then_some((id, progress))
        }).collect();
        if changed.is_empty() && self.pending_locations.is_empty() {
            return;
        }
        if let Err(error) = self.db.update(|save| {
            save.last_locations.extend(self.pending_locations.iter().map(|(name, &location)| {
                (name.clone(), location)
            }));
            for (id, progress) in changed {
                save.floors.entry(id).or_default().progress = progress;
            }
        }) {
            eprintln!("[SAVE] World checkpoint: {error}");
            self.progress_retry = Some(Instant::now() + Duration::from_secs(1));
        } else {
            self.pending_locations.clear();
            self.progress_retry = None;
        }
    }

    fn hall_of_fame_packet(&self) -> ServerPacket {
        ServerPacket::HallOfFame {
            highest_unlocked_floor: self.db.saved().highest_unlocked_floor,
            hall_of_fame: self.db.saved().hall_of_fame.clone(),
        }
    }

    fn tell(&self, player_id: &str, text: impl Into<String>) {
        if let Some(tx) = self.clients.get(player_id) {
            let _ = tx.send(ServerPacket::Announcement { text: text.into() });
        }
    }

    fn save_failed(&self, player_id: &str, error: io::Error) {
        eprintln!("[SAVE] {error}");
        self.tell(player_id, "The tower could not save that change. Try again.");
    }

    pub fn register_client(&mut self, player_id: String, sender: UnboundedSender<ServerPacket>) {
        self.clients.insert(player_id, sender);
    }

    pub fn unregister_client(&mut self, player_id: &str) {
        self.clients.remove(player_id);
        let resume_id = self.resume_ids.remove(player_id);
        if let Some(floor_num) = self.client_floors.remove(player_id) {
            if let Some(floor) = self.floors.get_mut(&floor_num) {
                if let Some(player) = floor.remove_player(player_id) {
                    if let Some(id) = resume_id {
                        self.pending_locations.insert(id, floor.saved_location(&player));
                        self.sync_progress();
                    }
                    let msg = format!("{} left the floor.", player.username);
                    self.broadcast_to_floor(
                        floor_num,
                        ServerPacket::Announcement { text: msg },
                    );
                }
            }
        }
    }

    pub fn broadcast_to_floor(&self, floor: u32, packet: ServerPacket) {
        for (client_id, client_floor) in &self.client_floors {
            if *client_floor == floor {
                if let Some(tx) = self.clients.get(client_id) {
                    let _ = tx.send(packet.clone());
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn broadcast_to_all(&self, packet: ServerPacket) {
        for tx in self.clients.values() {
            let _ = tx.send(packet.clone());
        }
    }

    fn light_campfire(&mut self, player_id: &str) {
        const COST: u32 = 15;

        let Some(&id) = self.client_floors.get(player_id) else { return };
        let Some(floor) = self.floors.get_mut(&id) else { return };
        let Some(player) = floor.players.get(player_id).filter(|p| p.is_alive()) else { return };
        let pos = player.pos;
        if floor.get_effective_tile(pos) != Some(Tile::Floor) || floor.carvings.iter().any(|c| c.pos == pos) {
            self.tell(player_id, "A campfire needs a clear patch of stone.");
            return;
        }
        if player.shards < COST {
            self.tell(player_id, "You need 15 shards to kindle a campfire.");
            return;
        }
        let name = player.username.clone();
        let builder = name.to_lowercase();
        if let Err(error) = self.db.update(|save| {
            let saved = save.floors.entry(id).or_default();
            saved.structures.push((pos, Tile::Campfire));
            saved.campfires_by_builder.insert(builder.clone(), pos);
        }) {
            self.save_failed(player_id, error);
            return;
        }

        floor.players.get_mut(player_id).unwrap().shards -= COST;
        floor.tiles_override.insert(pos, Tile::Campfire);
        floor.player_campfires.insert(builder, pos);
        self.broadcast_to_floor(id, ServerPacket::Announcement {
            text: format!("@{name} kindled a campfire. Rest near the embers."),
        });
    }

    fn carve(&mut self, player_id: &str, message: &str) {
        let message = sanitize_text(message, 80);
        if message.is_empty() {
            return;
        }
        let Some(&id) = self.client_floors.get(player_id) else { return };
        let Some(floor) = self.floors.get_mut(&id) else { return };
        let Some(player) = floor.players.get(player_id).filter(|p| p.is_alive()) else { return };
        let pos = player.pos;
        if floor.get_effective_tile(pos) != Some(Tile::Floor) || floor.carvings.iter().any(|c| c.pos == pos) {
            self.tell(player_id, "Find bare stone to leave a carving.");
            return;
        }
        let carving = FloorCarving::new(pos, player.username.clone(), message);
        if let Err(error) = self.db.update(|save| {
            save.floors.entry(id).or_default().carvings.push(SavedCarving {
                carving: carving.clone(),
                voters: Default::default(),
            });
        }) {
            self.save_failed(player_id, error);
            return;
        }

        floor.carvings.push(carving);
        self.tell(player_id, "Your words are carved into the stone.");
    }

    fn upvote(&mut self, player_id: &str, pos: Position) {
        const HEAL: i32 = 15;

        let Some(&id) = self.client_floors.get(player_id) else { return };
        let Some(floor) = self.floors.get_mut(&id) else { return };
        let Some(player) = floor.players.get(player_id).filter(|p| p.is_alive()) else { return };
        if player.pos.manhattan_distance(pos) > 2 {
            return;
        }
        let Some(carving) = floor.carvings.iter_mut().find(|c| c.pos == pos) else { return };
        let voter = player.username.to_lowercase();
        let author = carving.author.to_lowercase();
        if voter == author {
            self.tell(player_id, "Only another climber can praise your carving.");
            return;
        }
        if self.db.saved().floors.get(&id).is_some_and(|saved| {
            saved.carvings.iter().any(|entry| entry.carving.pos == pos && entry.voters.contains(&voter))
        }) {
            self.tell(player_id, "You already praised this carving.");
            return;
        }
        let mut updated = carving.clone();
        updated.upvotes = updated.upvotes.saturating_add(1);
        if let Err(error) = self.db.update(|save| {
            let carvings = &mut save.floors.entry(id).or_default().carvings;
            if let Some(entry) = carvings.iter_mut().find(|entry| entry.carving.pos == pos) {
                entry.carving = updated.clone();
                entry.voters.insert(voter);
            } else {
                carvings.push(SavedCarving { carving: updated.clone(), voters: [voter].into() });
            }
        }) {
            self.save_failed(player_id, error);
            return;
        }
        let name = updated.author.clone();
        *carving = updated;

        let healed = self.floors.values_mut().flat_map(|floor| floor.players.values_mut())
            .find(|player| player.is_alive() && player.username.to_lowercase() == author)
            .map(|player| {
                player.heal(HEAL);
                player.id.clone()
            });
        self.tell(player_id, format!("You praised @{name}'s carving."));
        if let Some(author_id) = healed {
            self.tell(&author_id, "A wandering climber praised your carving.");
        }
    }

    pub fn handle_packet(&mut self, player_id: &str, packet: ClientPacket) {
        match packet {
            ClientPacket::Join { username, class_idx, resume_key } => {
                if self.client_floors.contains_key(player_id) {
                    return;
                }
                let clean_name = sanitize_text(&username, 24);
                let username = if clean_name.is_empty() {
                    "Climber".to_string()
                } else {
                    clean_name
                };

                let name = username.to_lowercase();
                if self.floors.values().any(|floor| {
                    floor.players.values().any(|player| player.username.to_lowercase() == name)
                }) {
                    if let Some(tx) = self.clients.get(player_id) {
                        let _ = tx.send(ServerPacket::Error {
                            message: "A climber with that name is already in the tower.".into(),
                        });
                    }
                    return;
                }
                let resume_id = resume_key.as_ref().and_then(|key| resume_id(&name, key));
                let saved = resume_id.as_ref().and_then(|id| {
                    self.pending_locations.get(id).or_else(|| self.db.saved().last_locations.get(id))
                }).copied()
                    .filter(|location| (1..=self.db.saved().highest_unlocked_floor).contains(&location.floor));

                let start_floor = saved.map_or(self.raid_start_floor.unwrap_or(1), |location| location.floor);
                let world_seed = self.world_seed;
                let practice = self.raid_start_floor == Some(start_floor);
                let demo_spawns = self.demo_spawns.filter(|_| start_floor == 1);
                let floor_inst = self.get_or_create_floor(start_floor);
                let spawn = demo_spawns.map_or(floor_inst.map.spawn_pos, |spawns| {
                    spawns[floor_inst.players.len() % spawns.len()]
                });
                let spawn_pos = saved.filter(|location| floor_inst.can_rejoin_at(location.pos))
                    .map_or(spawn, |location| location.pos);

                let mut player = PlayerState::new(
                    player_id.to_string(),
                    username.clone(),
                    class_idx,
                    spawn_pos,
                    start_floor,
                );
                let practice_rank = if practice { (start_floor / 10 - 1).min(8) as u8 } else { 0 };
                player.set_temper(saved.map_or(practice_rank, |location| location.temper));
                if practice && saved.is_none() {
                    player.shards = player.temper_cost().unwrap_or_default();
                }

                floor_inst.add_player(player);
                self.client_floors.insert(player_id.to_string(), start_floor);
                if let Some(id) = resume_id {
                    self.resume_ids.insert(player_id.to_string(), id);
                }

                if let Some(tx) = self.clients.get(player_id) {
                    let _ = tx.send(ServerPacket::Welcome {
                        player_id: player_id.to_string(),
                        floor: start_floor,
                        map_seed: world_seed,
                        skip_prologue: saved.is_some() || demo_spawns.is_some(),
                    });
                    let _ = tx.send(self.hall_of_fame_packet());
                }

                let join_announcement = format!("{} entered floor {}!", username, start_floor);
                self.broadcast_to_floor(
                    start_floor,
                    ServerPacket::Announcement {
                        text: join_announcement,
                    },
                );
            }

            ClientPacket::Move(dir) => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        let (_moved, event) = floor.move_player_with_events(player_id, dir);
                        if let Some(announcement) = event {
                            self.broadcast_to_floor(
                                floor_num,
                                ServerPacket::Announcement { text: announcement },
                            );
                        }
                    }
                }
            }

            ClientPacket::Attack(dir) => {
                let mut announcements = Vec::new();
                let mut target_floor = None;

                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    target_floor = Some(floor_num);
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        if let Some(player) = floor.players.get(player_id).filter(|p| p.is_alive()) {
                            let attack = crate::combat::execute_basic_attack(
                                player,
                                dir,
                                |pos| FloorInstance::resolve_tile(&floor.map, &floor.tiles_override, pos),
                                &mut floor.enemies,
                                floor.tick_counter,
                            );

                            floor.projectiles.extend(attack.projectiles);
                            announcements.extend(attack.announcements);

                            if attack.shards_dropped > 0 {
                                if let Some(p) = floor.players.get_mut(player_id) {
                                    p.shards += attack.shards_dropped;
                                }
                            }
                        }
                    }
                }

                if let Some(floor_num) = target_floor {
                    for event in announcements {
                        self.broadcast_to_floor(floor_num, ServerPacket::Announcement { text: event });
                    }
                }
            }

            ClientPacket::ClassSkill(dir) => {
                let mut announcements = Vec::new();
                let mut target_floor = None;

                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    target_floor = Some(floor_num);
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        let player_info = floor.players.get(player_id).and_then(|p| {
                            if p.is_alive() {
                                Some((p.class_idx, p.username.clone(), p.pos, p.mana, p.attack_power()))
                            } else {
                                None
                            }
                        });

                        if let Some((class_idx, player_name, start_pos, cur_mana, base_dmg)) = player_info {
                            let (mana_cost, skill_name) = match class_idx {
                                0 => (15, "Iai: Eightfold Bloom"),
                                1 => (20, "Aether Rupture: Singularity"),
                                2 => (15, "Phantom Rain: Eclipse Volley"),
                                _ => (15, "Titan's Cataclysm: Faultline"),
                            };

                            if cur_mana < mana_cost {
                                announcements.push(format!(
                                    "Need {} MP for {}! (You have {})",
                                    mana_cost, skill_name, cur_mana
                                ));
                            } else {
                                if let Some(p) = floor.players.get_mut(player_id) {
                                    p.spend_mana(mana_cost);
                                }

                                let mut shards_earned = 0;
                                let mut hit_gatekeeper = false;

                                match class_idx {
                                    // 0: Blade - Iai: Eightfold Bloom (all 8 surrounding tiles)
                                    0 => {
                                        announcements.push(format!(
                                            ":: EIGHTFOLD BLOOM :: {} drew their blade in a blinding 360-degree storm of crimson steel!",
                                            player_name
                                        ));
                                        let offsets = [
                                            (-1, -1), (0, -1), (1, -1),
                                            (-1, 0),           (1, 0),
                                            (-1, 1),  (0, 1),  (1, 1),
                                        ];
                                        for (dx, dy) in offsets {
                                            let target_pos = Position::new(start_pos.x + dx, start_pos.y + dy);
                                            let k_dir = Direction::from_delta(dx, dy);
                                            let slash_glyph = match (dx, dy) {
                                                (0, -1) | (0, 1) => '|',
                                                (-1, 0) | (1, 0) => '-',
                                                (-1, -1) | (1, 1) => '\\',
                                                _ => '/',
                                            };
                                            floor.projectiles.push((
                                                ProjectileState::new(target_pos, slash_glyph, (255, 90, 50)),
                                                3,
                                            ));
                                            if let Some((_d, _c, s, events)) = floor.hit_enemy_at(
                                                target_pos,
                                                base_dmg + 12,
                                                30,
                                                k_dir,
                                                &player_name,
                                                &mut hit_gatekeeper,
                                            ) {
                                                shards_earned += s;
                                                announcements.extend(events);
                                            }
                                        }
                                    }

                                    // 1: Arcanist - Aether Rupture: Singularity (all tiles within Chebyshev <= 3)
                                    1 => {
                                        announcements.push(format!(
                                            ":: AETHER RUPTURE :: {} fractured reality in a roaring celestial singularity!",
                                            player_name
                                        ));
                                        for dy in -3..=3 {
                                            for dx in -3..=3 {
                                                if dx == 0 && dy == 0 {
                                                    continue;
                                                }
                                                let target_pos = Position::new(start_pos.x + dx, start_pos.y + dy);
                                                if !target_pos.in_bounds(floor.map.width, floor.map.height) {
                                                    continue;
                                                }
                                                let k_dir = Direction::from_delta(dx, dy);
                                                if let Some((_d, _c, s, events)) = floor.hit_enemy_at(
                                                    target_pos,
                                                    base_dmg + 16,
                                                    35,
                                                    k_dir,
                                                    &player_name,
                                                    &mut hit_gatekeeper,
                                                ) {
                                                    floor.projectiles.push((
                                                        ProjectileState::new(target_pos, '*', (80, 220, 255)),
                                                        3,
                                                    ));
                                                    shards_earned += s;
                                                    announcements.extend(events);
                                                }
                                            }
                                        }
                                        for dir_opt in Direction::ALL {
                                            let p = start_pos.step(dir_opt);
                                            floor.projectiles.push((
                                                ProjectileState::new(p, '⠶', (140, 240, 255)),
                                                3,
                                            ));
                                        }
                                    }

                                    // 2: Ranger - Phantom Rain: Eclipse Volley (3-line piercing volley)
                                    2 => {
                                        announcements.push(format!(
                                            ":: ECLIPSE VOLLEY :: {} loosed a three-line phantom storm, piercing the abyss!",
                                            player_name
                                        ));
                                        let (left_dir, right_dir) = match dir {
                                            Direction::Up => (Direction::Left, Direction::Right),
                                            Direction::Down => (Direction::Right, Direction::Left),
                                            Direction::Left => (Direction::Down, Direction::Up),
                                            Direction::Right => (Direction::Up, Direction::Down),
                                        };

                                        let lines = [
                                            (start_pos, dir),
                                            (start_pos.step(left_dir), dir),
                                            (start_pos.step(right_dir), dir),
                                        ];

                                        for (origin, line_dir) in lines {
                                            let mut cur = origin;
                                            for _ in 1..=5 {
                                                cur = cur.step(line_dir);
                                                if !cur.in_bounds(floor.map.width, floor.map.height) {
                                                    break;
                                                }
                                                floor.projectiles.push((
                                                    ProjectileState::new(cur, line_dir.arrow_glyph(), (80, 255, 120)),
                                                    3,
                                                ));
                                                if let Some((_d, _c, s, events)) = floor.hit_enemy_at(
                                                    cur,
                                                    base_dmg + 10,
                                                    40,
                                                    Some(line_dir),
                                                    &player_name,
                                                    &mut hit_gatekeeper,
                                                ) {
                                                    shards_earned += s;
                                                    announcements.extend(events);
                                                }
                                                if !floor.is_walkable(cur) {
                                                    break;
                                                }
                                            }
                                        }
                                    }

                                    // 3: Architect - Titan's Cataclysm: Faultline (shockwave cone facing dir)
                                    _ => {
                                        announcements.push(format!(
                                            ":: TITAN'S FAULTLINE :: {} struck the earth, fracturing bedrock into a crushing faultline!",
                                            player_name
                                        ));
                                        let perp_dir = match dir {
                                            Direction::Up | Direction::Down => Direction::Left,
                                            Direction::Left | Direction::Right => Direction::Up,
                                        };
                                        let opposite_perp = perp_dir.opposite();

                                        let forward1 = start_pos.step(dir);
                                        let forward2 = forward1.step(dir);

                                        let targets = [
                                            forward1,
                                            forward1.step(perp_dir),
                                            forward1.step(opposite_perp),
                                            forward2,
                                            forward2.step(perp_dir),
                                            forward2.step(opposite_perp),
                                        ];

                                        for target_pos in targets {
                                            if target_pos.in_bounds(floor.map.width, floor.map.height) {
                                                floor.projectiles.push((
                                                    ProjectileState::new(target_pos, '=', (255, 200, 60)),
                                                    4,
                                                ));
                                                if let Some((_d, _c, s, events)) = floor.hit_enemy_at(
                                                    target_pos,
                                                    base_dmg + 14,
                                                    25,
                                                    Some(dir),
                                                    &player_name,
                                                    &mut hit_gatekeeper,
                                                ) {
                                                    shards_earned += s;
                                                    announcements.extend(events);
                                                }
                                            }
                                        }
                                    }
                                }

                                if shards_earned > 0 {
                                    if let Some(p) = floor.players.get_mut(player_id) {
                                        p.shards += shards_earned;
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(floor_num) = target_floor {
                    for event in announcements {
                        self.broadcast_to_floor(floor_num, ServerPacket::Announcement { text: event });
                    }
                }
            }

            ClientPacket::ToggleTether { target_id } => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    let mut tether_msg = None;
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        let my_name = floor.players.get(player_id).map(|p| p.username.clone()).unwrap_or_default();
                        if let Some(player) = floor.players.get_mut(player_id) {
                            if player.tether_target == target_id {
                                player.tether_target = None;
                                tether_msg = Some(format!("* [TETHER] @{} severed their Aetheric Lightning Tether. *", my_name));
                            } else {
                                player.tether_target = target_id.clone();
                                if let Some(ref tid) = target_id {
                                    let target_name = if tid.starts_with("decoy-") {
                                        "Holo-Decoy §".to_string()
                                    } else {
                                        floor.players.get(tid).map(|p| p.username.clone()).unwrap_or_else(|| "Ally".to_string())
                                    };
                                    tether_msg = Some(format!("* [CO-OP] @{} forged an Aetheric Lightning Tether with {}! Electric arcing connects you! *", my_name, target_name));
                                }
                            }
                        }
                    }
                    if let Some(text) = tether_msg {
                        self.broadcast_to_floor(floor_num, ServerPacket::Announcement { text });
                    }
                }
            }

            ClientPacket::ReviveAlly { target_id } => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    let mut revive_payload = None;
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        let my_info = floor.players.get(player_id).and_then(|p| {
                            if p.is_alive() {
                                Some((p.pos, p.username.clone()))
                            } else {
                                None
                            }
                        });

                        if let Some((my_pos, my_name)) = my_info {
                            if let Some(target) = floor.players.get_mut(&target_id) {
                                if !target.is_alive() && my_pos.manhattan_distance(target.pos) <= 2 {
                                    target.hp = target.max_hp / 2;
                                    target.mana = target.max_mana / 2;
                                    let target_name = target.username.clone();
                                    let target_pos = target.pos;

                                    floor.projectiles.push((
                                        ProjectileState::new(target_pos, '✚', (120, 255, 160)),
                                        6,
                                    ));

                                    revive_payload = Some((
                                        format!("* [CO-OP REVIVAL] @{} channeled spirit energy to resurrect fallen comrade @{}! *", my_name, target_name),
                                        "++ CLIMBER RESURRECTED ++".to_string(),
                                        format!(":: HEROIC RESCUE :: @{} brought @{} back into the fight with 50% HP!", my_name, target_name)
                                    ));
                                }
                            }
                        }
                    }

                    if let Some((announcement, banner_title, banner_msg)) = revive_payload {
                        self.broadcast_to_floor(floor_num, ServerPacket::Announcement { text: announcement });
                        self.broadcast_to_floor(floor_num, ServerPacket::ServerBanner {
                            title: banner_title,
                            message: banner_msg,
                        });
                    }
                }
            }

            ClientPacket::DeployDecoy => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    let mut unseal_events = Vec::new();
                    let mut deploy_announcement = None;
                    let mut direct_msg = None;

                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        let player_opt = floor.players.get_mut(player_id).and_then(|p| {
                            if p.is_alive() {
                                Some((p.pos, p.username.clone(), p.spend_mana(20)))
                            } else {
                                None
                            }
                        });

                        if let Some((pos, username, mana_spent)) = player_opt {
                            if mana_spent {
                                let decoy_id = floor.next_decoy_id;
                                floor.next_decoy_id += 1;

                                let lifetime = if RaidBoss::for_floor(floor_num).is_some() { 900 } else { 300 };
                                let decoy = DecoyState::new(decoy_id, username.clone(), pos, lifetime);
                                floor.decoys.push(decoy);

                                floor.projectiles.push((
                                    ProjectileState::new(pos, '§', (120, 220, 255)),
                                    6,
                                ));

                                unseal_events = floor.check_pressure_plates();

                                println!(
                                    "[HYAKU ACTION] Player '{}' deployed Holo-Decoy at ({}, {}) (-20 MP, {} ticks)",
                                    username, pos.x, pos.y, lifetime
                                );

                                deploy_announcement = Some(format!(
                                    "* [HOLO-DECOY] @{} projected a Holo-Decoy at ({}, {})! (Holds pressure plates) *",
                                    username, pos.x, pos.y
                                ));
                            } else {
                                let cur_mana = floor.players.get(player_id).map(|p| p.mana).unwrap_or(0);
                                direct_msg = Some(format!("Need 20 Mana for Holo-Decoy! (You have {})", cur_mana));
                            }
                        }
                    }

                    if let Some(msg) = direct_msg {
                        if let Some(tx) = self.clients.get(player_id) {
                            let _ = tx.send(ServerPacket::Announcement { text: msg });
                        }
                    }

                    if let Some(ann) = deploy_announcement {
                        self.broadcast_to_floor(floor_num, ServerPacket::Announcement { text: ann });
                    }

                    for event in unseal_events {
                        self.broadcast_to_floor(floor_num, ServerPacket::Announcement { text: event });
                    }
                }
            }

            ClientPacket::TemperGear => {
                let Some(&floor_num) = self.client_floors.get(player_id) else { return };
                let floor = &self.floors[&floor_num];
                let Some(player) = floor.players.get(player_id).filter(|p| p.is_alive()) else { return };
                let Some(cost) = player.temper_cost() else {
                    self.tell(player_id, "Your gear is fully tempered.");
                    return;
                };
                let at_fire = (-2..=2).any(|dy| (-2..=2).any(|dx| {
                    floor.get_effective_tile(player.pos.add_offset(dx, dy)) == Some(Tile::Campfire)
                }));
                if !at_fire || player.shards < cost {
                    self.tell(player_id, format!("Forge an Ember at a campfire with {cost} Shards."));
                    return;
                }

                let mut tempered = player.clone();
                tempered.shards -= cost;
                tempered.set_temper(player.temper + 1);
                if let Some(id) = self.resume_ids.get(player_id).cloned() {
                    let location = floor.saved_location(&tempered);
                    if let Err(error) = self.db.update(|save| {
                        save.last_locations.insert(id.clone(), location);
                    }) {
                        self.save_failed(player_id, error);
                        return;
                    }
                    self.pending_locations.remove(&id);
                }
                let message = format!("Tempered by ember. Gear +{}  •  +9 ATK  +12 HP  +5 MP", tempered.temper);
                self.floors.get_mut(&floor_num).unwrap().players.insert(player_id.into(), tempered);
                self.tell(player_id, message);
            }

            ClientPacket::DepositShards { amount } => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    let mut unsealed_stairs = false;
                    let mut banner_payload = None;
                    let mut deposit_announcement = None;
                    let mut direct_msg = None;

                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        let player_info = floor.players.get(player_id).and_then(|p| {
                            if p.is_alive() {
                                Some((p.pos, p.shards, p.username.clone()))
                            } else {
                                None
                            }
                        });

                        if let Some((pos, shards, username)) = player_info {
                            let near_shrine = floor.map.shrine_pos.map_or(false, |sp| {
                                pos.chebyshev_distance(sp) <= 3
                            }) || matches!(floor.get_effective_tile(pos), Some(Tile::Shrine));

                            if floor.floor % 10 == 0 {
                                direct_msg = Some("Rest here and recover. This shrine requires no offering.".to_string());
                            } else if !near_shrine {
                                direct_msg = Some("You must stand near the Floor Shrine [百] to deposit shards!".to_string());
                            } else {
                                let deposit = amount.min(shards);
                                if deposit > 0 {
                                    let mut progress = floor.progress();
                                    progress.shrine_shards = floor.shrine_shards.saturating_add(deposit);
                                    progress.stairs_unlocked |= progress.shrine_shards >= floor.shrine_goal;
                                    let bank = self.tower_shards_bank.saturating_add(deposit);
                                    if let Err(error) = self.db.update(|save| {
                                        save.floors.entry(floor_num).or_default().progress = progress;
                                        save.tower_shards_bank = bank;
                                    }) {
                                        self.save_failed(player_id, error);
                                        return;
                                    }
                                    if let Some(p) = floor.players.get_mut(player_id) {
                                        p.shards -= deposit;
                                    }
                                    floor.shrine_shards = floor.shrine_shards.saturating_add(deposit);
                                    self.tower_shards_bank = bank;

                                    println!(
                                        "[HYAKU SHRINE] Player '{}' deposited {} shards into Floor {} Shrine ({}/{} Shards). Bank: {}",
                                        username, deposit, floor_num, floor.shrine_shards, floor.shrine_goal, self.tower_shards_bank
                                    );

                                    deposit_announcement = Some(format!(
                                        "@{} laid {} shards before the shrine [百]. Offering {}/{}.",
                                        username, deposit, floor.shrine_shards, floor.shrine_goal
                                    ));

                                    if floor.shrine_shards >= floor.shrine_goal && !floor.stairs_unlocked {
                                        floor.stairs_unlocked = true;
                                        unsealed_stairs = true;
                                        banner_payload = Some((
                                            format!("THE SHRINE OF FLOOR {floor_num} AWAKENS"),
                                            format!(
                                                "The offering is complete ({}/{} shards). The stair is unsealed.",
                                                floor.shrine_shards, floor.shrine_goal
                                            ),
                                        ));
                                    }
                                } else {
                                    direct_msg = Some("You have 0 shards to deposit!".to_string());
                                }
                            }
                        }
                    }

                    if let Some(msg) = direct_msg {
                        if let Some(tx) = self.clients.get(player_id) {
                            let _ = tx.send(ServerPacket::Announcement { text: msg });
                        }
                    }

                    if let Some(ann) = deposit_announcement {
                        self.broadcast_to_floor(floor_num, ServerPacket::Announcement { text: ann });
                    }

                    if unsealed_stairs {
                        if let Some((title, message)) = banner_payload {
                            self.broadcast_to_all(ServerPacket::ServerBanner { title, message });
                        }
                    }
                }
            }

            ClientPacket::BuildBridge(dir) => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    let mut direct_msg = None;
                    let mut bridge_announcement = None;

                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        let player_info = floor.players.get(player_id).and_then(|p| {
                            if p.is_alive() {
                                Some((p.pos, p.shards, p.username.clone()))
                            } else {
                                None
                            }
                        });

                        if let Some((pos, shards, username)) = player_info {
                            let target_pos = pos.step(dir);
                            if !target_pos.in_bounds(floor.map.width, floor.map.height) {
                                direct_msg = Some("Target bridge position is out of bounds!".to_string());
                            } else {
                                let tile = floor.get_effective_tile(target_pos);
                                if tile.map_or(true, |t| !t.is_chasm()) {
                                    direct_msg = Some("No abyssal chasm in front of you to bridge!".to_string());
                                } else {
                                    const BRIDGE_COST: u32 = 5;
                                    if shards >= BRIDGE_COST {
                                        if let Err(error) = self.db.update(|save| {
                                            save.floors.entry(floor_num).or_default()
                                                .structures.push((target_pos, Tile::Bridge));
                                        }) {
                                            self.save_failed(player_id, error);
                                            return;
                                        }
                                        if let Some(p) = floor.players.get_mut(player_id) {
                                            p.shards -= BRIDGE_COST;
                                        }
                                        floor.tiles_override.insert(target_pos, Tile::Bridge);
                                        floor.projectiles.push((
                                            ProjectileState::new(target_pos, '═', (255, 215, 80)),
                                            4,
                                        ));
                                        println!(
                                            "[HYAKU ACTION] Player '{}' built bridge at ({}, {}) (-{} shards)",
                                            username, target_pos.x, target_pos.y, BRIDGE_COST
                                        );
                                        bridge_announcement = Some(format!(
                                            "* [CO-OP] @{} constructed a permanent Bridge (═) across the chasm! (-{} Shards) *",
                                            username, BRIDGE_COST
                                        ));
                                    } else {
                                        direct_msg = Some(format!(
                                            "Need {} shards to build a bridge! (You have {})",
                                            BRIDGE_COST, shards
                                        ));
                                    }
                                }
                            }
                        }
                    }

                    if let Some(msg) = direct_msg {
                        if let Some(tx) = self.clients.get(player_id) {
                            let _ = tx.send(ServerPacket::Announcement { text: msg });
                        }
                    }

                    if let Some(ann) = bridge_announcement {
                        self.broadcast_to_floor(floor_num, ServerPacket::Announcement { text: ann });
                    }
                }
            }

            ClientPacket::LightCampfire => self.light_campfire(player_id),
            ClientPacket::CarveFloor { message } => self.carve(player_id, &message),
            ClientPacket::UpvoteCarving { pos } => self.upvote(player_id, pos),

            ClientPacket::AscendStairs => {
                if let Some(&current_floor) = self.client_floors.get(player_id) {
                    let mut player_opt = None;
                    let mut can_ascend = false;
                    let mut discovered = false;
                    let mut rejection_msg = None;

                    if let Some(floor) = self.floors.get_mut(&current_floor) {
                        if let Some(player) = floor.players.get(player_id).filter(|p| p.is_alive()) {
                            let tile = floor.get_effective_tile(player.pos);
                            if tile == Some(Tile::StairsUp) || player.pos == floor.map.stairs_pos {
                                // Check if any Ascension Gate on this floor is still locked
                                let is_gate_locked = floor.map.gates.iter().any(|&(_id, gate_pos)| {
                                    matches!(
                                        floor.get_effective_tile(gate_pos),
                                        Some(Tile::Gate { open: false, .. })
                                    )
                                });

                                if let Some(boss) = floor.enemies.iter().find(|enemy| {
                                    enemy.kind == EnemyKind::FloorBoss && enemy.is_alive()
                                }) {
                                    rejection_msg = Some(format!("{} yet lives. Slay the keeper to unseal the stairs.", boss.name()));
                                } else if is_gate_locked && RaidBoss::for_floor(current_floor).is_some() {
                                    rejection_msg = Some("Hold both pressure plates to open the stair gate.".to_string());
                                } else if is_gate_locked {
                                    rejection_msg = Some("The gate is sealed. Stand upon its plate or slay its guardian.".to_string());
                                } else if !floor.stairs_unlocked {
                                    rejection_msg = Some(format!(
                                        "The gate is open. Lay shards before the shrine [百] to wake the stair ({}/{}).",
                                        floor.shrine_shards, floor.shrine_goal
                                    ));
                                } else {
                                    can_ascend = true;
                                }
                            } else {
                                rejection_msg = Some("You must stand on the Stairs to ascend!".to_string());
                            }
                        }
                        if can_ascend {
                            let next = (current_floor + 1).min(100);
                            discovered = current_floor < 100 && next > self.db.saved().highest_unlocked_floor;
                            let author = floor.players[player_id].username.clone();
                            let progress = floor.progress();
                            if let Err(error) = self.db.update(|save| {
                                save.floors.entry(current_floor).or_default().progress = progress;
                                if discovered {
                                    save.highest_unlocked_floor = next;
                                    save.hall_of_fame.insert(next, author);
                                }
                            }) {
                                self.save_failed(player_id, error);
                                return;
                            }
                            if current_floor == 100 {
                                if let Some(tx) = self.clients.get(player_id) {
                                    let _ = tx.send(ServerPacket::Victory);
                                }
                                return;
                            }
                            player_opt = floor.remove_player(player_id);
                        }
                    }

                    if let Some(msg) = rejection_msg {
                        if let Some(tx) = self.clients.get(player_id) {
                            let _ = tx.send(ServerPacket::Announcement {
                                text: msg.to_string(),
                            });
                        }
                    }

                    if can_ascend {
                        if let Some(mut player) = player_opt {
                            let next_floor = (current_floor + 1).min(100);
                            let seed = self.world_seed;
                            let next_inst = self.get_or_create_floor(next_floor);
                            player.pos = next_inst.map.spawn_pos;
                            player.floor = next_floor;
                            next_inst.add_player(player);
                            self.client_floors.insert(player_id.to_string(), next_floor);

                            let transition_msg = format!("Ascended to Floor {}", next_floor);
                            if let Some(tx) = self.clients.get(player_id) {
                                let _ = tx.send(ServerPacket::FloorTransition {
                                    floor: next_floor,
                                    map_seed: seed,
                                    message: transition_msg,
                                });
                            }

                            if discovered {
                                self.broadcast_to_all(self.hall_of_fame_packet());
                            }
                        }
                    }
                }
            }

            ClientPacket::Respawn => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        let mut username = String::new();
                        let respawn_pos = floor.players.get(player_id)
                            .map(|player| floor.respawn_position(&player.username))
                            .unwrap_or(floor.map.spawn_pos);
                        if let Some(player) = floor.players.get_mut(player_id) {
                            if !player.is_alive() {
                                player.pos = respawn_pos;
                                player.hp = player.max_hp;
                                player.mana = player.max_mana;
                                username = player.username.clone();
                            }
                        }

                        if !username.is_empty() {
                            if let Some(tx) = self.clients.get(player_id) {
                                let _ = tx.send(ServerPacket::Respawned);
                            }
                            println!(
                                "[HYAKU RESPAWN] Player '{}' respawned at ({}, {})",
                                username, respawn_pos.x, respawn_pos.y
                            );
                            self.broadcast_to_floor(
                                floor_num,
                                ServerPacket::Announcement {
                                    text: format!("{} has risen and respawned!", username),
                                },
                            );
                        }
                    }
                }
            }

            ClientPacket::Chat { text } => {
                let clean_text = sanitize_text(&text, 120);
                if clean_text.is_empty() {
                    return;
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    let sender_name = self
                        .floors
                        .get_mut(&floor_num)
                        .and_then(|f| f.players.get_mut(player_id))
                        .map(|p| {
                            p.last_chat = Some((clean_text.clone(), now));
                            p.username.clone()
                        })
                        .unwrap_or_else(|| "Adventurer".to_string());

                    self.broadcast_to_floor(
                        floor_num,
                        ServerPacket::ChatMessage {
                            sender: sender_name,
                            text: clean_text,
                            timestamp: now,
                        },
                    );
                }
            }

            ClientPacket::Ping { seq } => {
                if let Some(tx) = self.clients.get(player_id) {
                    let _ = tx.send(ServerPacket::Pong { seq });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyaku_protocol::Direction;
    use tokio::sync::mpsc;

    #[test]
    fn structures_carvings_and_first_ascent_survive_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("hyaku_save.json");
        let mut server = ServerState::load(42, path.clone()).unwrap();
        for (id, name) in [("alice", "Alice"), ("bob", "Bob")] {
            server.handle_packet(id, ClientPacket::Join { username: name.into(), class_idx: 0, resume_key: (id == "alice").then(|| ResumeKey("a".repeat(64))) });
        }

        let floor = server.floors.get_mut(&1).unwrap();
        let mut open = (0..floor.map.height).flat_map(|y| (0..floor.map.width).map(move |x| Position::new(x, y)))
            .filter(|&pos| floor.get_effective_tile(pos) == Some(Tile::Floor)
                && floor.carvings.iter().all(|c| c.pos != pos));
        let fire = open.next().unwrap();
        let second_fire = open.next().unwrap();
        let note = open.find(|pos| pos.chebyshev_distance(fire) > 2).unwrap();
        let author = floor.players.get_mut("alice").unwrap();
        author.pos = fire;
        author.shards = 30;
        server.handle_packet("alice", ClientPacket::LightCampfire);
        let floor = server.floors.get_mut(&1).unwrap();
        let author = floor.players.get_mut("alice").unwrap();
        assert_eq!(author.shards, 15);
        author.pos = second_fire;
        server.handle_packet("alice", ClientPacket::LightCampfire);
        let floor = server.floors.get_mut(&1).unwrap();
        assert_eq!(floor.get_effective_tile(fire), Some(Tile::Campfire));
        let near_fire = fire.all_neighbors().into_iter()
            .find(|&pos| floor.get_effective_tile(pos) == Some(Tile::Floor))
            .unwrap();
        let reader = floor.players.get_mut("bob").unwrap();
        reader.pos = near_fire;
        reader.hp = 20;
        floor.step_tick(0);
        assert_eq!(floor.players["bob"].hp, 21);

        let author = floor.players.get_mut("alice").unwrap();
        author.pos = note;
        author.hp = 20;
        floor.players.get_mut("bob").unwrap().pos = note;
        server.handle_packet("alice", ClientPacket::CarveFloor { message: "Mind the gate.".into() });
        server.handle_packet("alice", ClientPacket::UpvoteCarving { pos: note });
        server.handle_packet("bob", ClientPacket::UpvoteCarving { pos: note });
        server.handle_packet("bob", ClientPacket::UpvoteCarving { pos: note });
        assert_eq!(server.floors[&1].players["alice"].hp, 35);

        let floor = server.floors.get_mut(&1).unwrap();
        for &(gate, pos) in &floor.map.gates {
            floor.tiles_override.insert(pos, Tile::Gate { id: gate, open: true });
            floor.unlocked_gates.insert(gate);
        }
        floor.players.get_mut("alice").unwrap().pos = floor.map.stairs_pos;
        server.handle_packet("alice", ClientPacket::AscendStairs);
        assert_eq!(server.client_floors["alice"], 2);

        let floor = server.floors.get_mut(&2).unwrap();
        let author = floor.players.get_mut("alice").unwrap();
        author.pos = floor.map.shrine_pos.unwrap();
        let logout_pos = author.pos;
        author.shards = 7;
        server.handle_packet("alice", ClientPacket::DepositShards { amount: 7 });
        server.unregister_client("alice");

        let mut builder = server.floors.get_mut(&1).unwrap().remove_player("bob").unwrap();
        let floor = server.get_or_create_floor(10);
        let (origin, direction) = (0..floor.map.height)
            .flat_map(|y| (0..floor.map.width).map(move |x| Position::new(x, y)))
            .filter(|&pos| floor.get_effective_tile(pos) == Some(Tile::Floor))
            .find_map(|pos| Direction::ALL.into_iter().find(|&dir| {
                floor.get_effective_tile(pos.step(dir)) == Some(Tile::Chasm)
            }).map(|dir| (pos, dir)))
            .unwrap();
        builder.floor = 10;
        builder.pos = origin;
        builder.shards = 5;
        floor.add_player(builder);
        server.client_floors.insert("bob".into(), 10);
        server.handle_packet("bob", ClientPacket::BuildBridge(direction));
        drop(server);

        let mut restored = ServerState::load(7, path).unwrap();
        assert_eq!(restored.world_seed, 42);
        assert!(matches!(restored.hall_of_fame_packet(), ServerPacket::HallOfFame {
            highest_unlocked_floor: 2, hall_of_fame,
        } if hall_of_fame.get(&2).map(String::as_str) == Some("Alice")));
        assert_eq!(restored.tower_shards_bank, 7);
        assert_eq!(restored.get_or_create_floor(2).shrine_shards, 7);
        for key in [None, Some(ResumeKey("b".repeat(64)))] {
            restored.handle_packet("other", ClientPacket::Join {
                username: "ALICE".into(),
                class_idx: 0,
                resume_key: key,
            });
            assert_eq!(restored.client_floors["other"], 1);
            restored.unregister_client("other");
        }
        restored.handle_packet("returning", ClientPacket::Join { username: "ALICE".into(), class_idx: 0, resume_key: Some(ResumeKey("a".repeat(64))) });
        assert_eq!(restored.client_floors["returning"], 2);
        assert_eq!(restored.floors[&2].players["returning"].pos, logout_pos);
        assert_eq!(restored.get_or_create_floor(10).get_effective_tile(origin.step(direction)), Some(Tile::Bridge));

        restored.handle_packet("reader", ClientPacket::Join { username: "BOB".into(), class_idx: 0, resume_key: None });
        let floor = restored.floors.get_mut(&1).unwrap();
        assert_eq!(floor.get_effective_tile(fire), Some(Tile::Campfire));
        assert_eq!(floor.get_effective_tile(second_fire), Some(Tile::Campfire));
        assert_eq!(floor.player_campfires.get("alice"), Some(&second_fire));
        assert!(floor.map.gates.iter().all(|&(id, pos)| floor.get_effective_tile(pos) == Some(Tile::Gate { id, open: true })));
        floor.players.get_mut("reader").unwrap().pos = note;
        restored.handle_packet("reader", ClientPacket::UpvoteCarving { pos: note });
        let carving = restored.floors[&1].carvings.iter().find(|c| c.pos == note).unwrap();
        assert_eq!((carving.message.as_str(), carving.upvotes), ("Mind the gate.", 1));
    }

    #[test]
    fn reconnect_uses_the_last_checkpoint_or_the_floor_spawn() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("hyaku_save.json");
        let mut server = ServerState::load(42, path.clone()).unwrap();
        let join = ClientPacket::Join { username: "Alice".into(), class_idx: 0, resume_key: Some(ResumeKey("a".repeat(64))) };
        server.handle_packet("first", join.clone());
        server.handle_packet("duplicate", join.clone());
        assert!(!server.client_floors.contains_key("duplicate"));

        let floor = server.floors.get_mut(&1).unwrap();
        let spawn = floor.map.spawn_pos;
        let checkpoint = spawn.cardinal_neighbors().into_iter()
            .find(|&pos| floor.is_walkable(pos))
            .unwrap();
        floor.players.get_mut("first").unwrap().pos = checkpoint;
        server.next_location_checkpoint = Instant::now();
        server.sync_progress();
        drop(server);

        let mut restored = ServerState::load(42, path.clone()).unwrap();
        restored.handle_packet("returning", join.clone());
        assert_eq!(restored.floors[&1].players["returning"].pos, checkpoint);
        drop(restored);

        let mut restored = ServerState::load(42, path).unwrap();
        restored.floors.get_mut(&1).unwrap().tiles_override.insert(checkpoint, Tile::Wall);
        restored.handle_packet("returning", join);
        assert_eq!(restored.floors[&1].players["returning"].pos, spawn);
    }

    #[test]
    fn tempering_costs_shards_at_the_fire_and_survives_reconnect() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("hyaku_save.json");
        let mut server = ServerState::load(42, path.clone()).unwrap();
        let join = ClientPacket::Join {
            username: "Alice".into(),
            class_idx: 1,
            resume_key: Some(ResumeKey("a".repeat(64))),
        };
        server.handle_packet("alice", join.clone());
        let floor = server.floors.get_mut(&1).unwrap();
        let player = floor.players.get_mut("alice").unwrap();
        let base = player.attack_power();
        player.pos = floor.map.stairs_pos;
        player.shards = 8;
        server.handle_packet("alice", ClientPacket::TemperGear);
        assert_eq!(server.floors[&1].players["alice"].temper, 0);

        let floor = server.floors.get_mut(&1).unwrap();
        floor.players.get_mut("alice").unwrap().pos = floor.map.spawn_pos;
        floor.tiles_override.insert(floor.map.spawn_pos, Tile::Campfire);
        server.handle_packet("alice", ClientPacket::TemperGear);
        let player = &server.floors[&1].players["alice"];
        assert_eq!((player.temper, player.shards, player.attack_power()), (1, 0, base + 9));
        drop(server);

        let mut restored = ServerState::load(42, path).unwrap();
        restored.handle_packet("returning", join);
        assert_eq!(restored.floors[&1].players["returning"].attack_power(), base + 9);
    }

    #[test]
    fn summit_requires_a_win_and_respawn_is_confirmed_by_the_server() {
        let mut server = ServerState::new(42);
        let (tx, mut rx) = mpsc::unbounded_channel();
        server.register_client("alice".into(), tx);
        let floor = server.get_or_create_floor(100);
        let stairs = floor.map.stairs_pos;
        floor.add_player(PlayerState::new("alice".into(), "Alice".into(), 0, stairs, 100));
        server.client_floors.insert("alice".into(), 100);

        server.handle_packet("alice", ClientPacket::AscendStairs);
        assert!(matches!(rx.try_recv().unwrap(), ServerPacket::Announcement { .. }));

        let floor = server.floors.get_mut(&100).unwrap();
        floor.enemies.iter_mut().find(|e| e.kind == EnemyKind::FloorBoss).unwrap().hp = 0;
        floor.step_tick(0);
        server.handle_packet("alice", ClientPacket::AscendStairs);
        assert!(matches!(rx.try_recv().unwrap(), ServerPacket::Announcement { .. }));

        let floor = server.floors.get_mut(&100).unwrap();
        let left = floor.map.plates[0].1;
        let right = floor.map.plates[1].1;
        floor.decoys.push(DecoyState::new(1, "Alice".into(), left, 30));
        floor.players.get_mut("alice").unwrap().pos = right;
        assert_eq!(floor.check_pressure_plates().len(), 1);
        floor.players.get_mut("alice").unwrap().pos = stairs;
        server.handle_packet("alice", ClientPacket::AscendStairs);
        assert_eq!(rx.try_recv().unwrap(), ServerPacket::Victory);
        assert_eq!(server.floors[&100].players["alice"].pos, stairs);
        assert!(rx.try_recv().is_err());

        server.floors.get_mut(&100).unwrap().players.get_mut("alice").unwrap().hp = 0;
        server.handle_packet("alice", ClientPacket::Respawn);
        assert_eq!(rx.try_recv().unwrap(), ServerPacket::Respawned);
        let floor = &server.floors[&100];
        assert!(floor.players["alice"].is_alive());
        assert_eq!(floor.players["alice"].pos, floor.map.spawn_pos);
    }

    #[test]
    fn solo_raid_practice_starts_at_each_gatekeeper_and_accepts_a_decoy_tether() {
        use hyaku_protocol::BossPhase;

        for number in (10..=100).step_by(10) {
            let mut server = ServerState::raid_practice(42, number);
            server.handle_packet("tester", ClientPacket::Join {
                username: "Tester".into(),
                class_idx: 1,
                resume_key: None,
            });
            assert_eq!(server.client_floors["tester"], number);
            let floor = server.floors.get_mut(&number).unwrap();
            let boss = floor.enemies.iter_mut().find(|enemy| enemy.kind == EnemyKind::FloorBoss).unwrap();
            boss.boss_phase = Some(BossPhase::Shielded);
            let center = boss.pos;
            let (half_width, _) = boss.raid_boss.unwrap().half_size();

            floor.players.get_mut("tester").unwrap().pos = center.add_offset(-half_width - 1, 0);
            server.handle_packet("tester", ClientPacket::DeployDecoy);
            let floor = server.floors.get_mut(&number).unwrap();
            let decoy_id = floor.decoys[0].id;
            floor.players.get_mut("tester").unwrap().pos = center.add_offset(half_width + 1, 0);
            server.handle_packet("tester", ClientPacket::ToggleTether {
                target_id: Some(format!("decoy-{decoy_id}")),
            });
            let tick = server.floors.get_mut(&number).unwrap().step_tick(0);
            let boss = tick.enemies.iter().find(|enemy| enemy.kind == EnemyKind::FloorBoss).unwrap();
            assert_eq!(boss.boss_phase, Some(BossPhase::Exposed));
        }
    }

    #[test]
    fn gatekeeper_shield_requires_a_live_ally_and_defeat_opens_stairs() {
        use hyaku_protocol::BossPhase;

        let mut floor = FloorInstance::new(10, 42);
        floor.enemies.retain(|e| e.kind == EnemyKind::FloorBoss);
        let center = floor.enemies[0].pos;
        let west = Position::new(center.x - 4, center.y);
        let east = Position::new(center.x + 4, center.y);
        let plate = floor.map.plates[0].1;
        let gate = floor.map.gates[0].1;
        floor.add_player(PlayerState::new("alice".into(), "Alice".into(), 0, west, 10));
        floor.add_player(PlayerState::new("bob".into(), "Bob".into(), 0, plate, 10));
        floor.check_pressure_plates();
        assert_eq!(floor.get_effective_tile(gate), Some(Tile::Gate { id: 1, open: false }));
        assert!(!floor.stairs_unlocked);
        assert!(!floor.move_player("alice", Direction::Right));

        let other_plate = floor.map.plates[1].1;
        floor.players.get_mut("alice").unwrap().pos = other_plate;
        assert_eq!(floor.check_pressure_plates().len(), 1);
        assert_eq!(floor.get_effective_tile(gate), Some(Tile::Gate { id: 1, open: true }));
        assert!(!floor.stairs_unlocked);
        floor.players.get_mut("alice").unwrap().pos = west;

        floor.enemies[0].boss_phase = Some(BossPhase::Shielded);
        let hp = floor.enemies[0].hp;
        let edge = west.step(Direction::Right);
        assert!(floor.hit_enemy_at(edge, 50, 0, None, "Alice", &mut false).is_none());
        floor.decoys.push(DecoyState::new(1, "Alice".into(), east, 30));
        floor.players.get_mut("alice").unwrap().tether_target = Some("decoy-1".into());
        assert_eq!(floor.step_tick(0).enemies[0].boss_phase, Some(BossPhase::Shielded));

        let bob = floor.players.get_mut("bob").unwrap();
        bob.pos = east;
        bob.tether_target = Some("alice".into());
        floor.players.get_mut("alice").unwrap().tether_target = Some("bob".into());
        let tick = floor.step_tick(0);
        assert_eq!(tick.enemies[0].boss_phase, Some(BossPhase::Exposed));
        assert_eq!(tick.enemies[0].hp, hp);
        assert_eq!(floor.step_tick(0).enemies[0].hp, hp - crate::combat::TETHER_DAMAGE_PER_TICK);

        let mut hit_gatekeeper = false;
        assert!(floor.hit_enemy_at(edge, 25, 0, None, "Alice", &mut hit_gatekeeper).is_some());
        assert!(floor.hit_enemy_at(center, 25, 0, None, "Alice", &mut hit_gatekeeper).is_none());

        floor.enemies[0].hp = 1;
        let slash = crate::combat::execute_basic_attack(
            &floor.players["alice"], Direction::Right,
            |pos| FloorInstance::resolve_tile(&floor.map, &floor.tiles_override, pos),
            &mut floor.enemies, floor.tick_counter,
        );
        assert_eq!(slash.hit_count, 1);
        assert_eq!(slash.shards_dropped, 10);
        let tick = floor.step_tick(0);
        assert!(tick.enemies.is_empty());
        assert!(tick.stairs_unlocked);
        assert_eq!(floor.get_effective_tile(gate), Some(Tile::Gate { id: 1, open: true }));
    }

    #[test]
    fn test_floor_instance_movement_collision() {
        let mut floor = FloorInstance::new(1, 12345);
        let spawn = floor.map.spawn_pos;

        let player = PlayerState::new(
            "p1".to_string(),
            "Hero".to_string(),
            0,
            spawn,
            1,
        );
        floor.add_player(player);

        // Find an adjacent wall and floor
        for dir in Direction::ALL {
            let next_pos = spawn.step(dir);
            let is_walkable = floor.is_walkable(next_pos);
            let moved = floor.move_player("p1", dir);
            assert_eq!(moved, is_walkable);
            if moved {
                // Move back
                floor.move_player("p1", dir.opposite());
                break;
            }
        }
    }

    #[test]
    fn test_server_state_client_lifecycle() {
        let mut server = ServerState::new(999);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let client_id = "test-client-1".to_string();

        server.register_client(client_id.clone(), tx);
        server.handle_packet(
            &client_id,
            ClientPacket::Join {
                username: "Tester".to_string(),
                class_idx: 1,
                resume_key: None,
            },
        );

        // Client should receive Welcome packet
        let welcome = rx.try_recv().expect("welcome packet received");
        match welcome {
            ServerPacket::Welcome { player_id, floor, .. } => {
                assert_eq!(player_id, client_id);
                assert_eq!(floor, 1);
            }
            other => panic!("expected Welcome packet, got {:?}", other),
        }

        server.unregister_client(&client_id);
        assert!(!server.clients.contains_key(&client_id));
        assert!(!server.client_floors.contains_key(&client_id));
    }

    #[test]
    fn test_combat_knockback_and_abyss() {
        let mut floor = FloorInstance::new(1, 42);
        let pos = Position::new(10, 10);
        let chasm_pos = Position::new(10, 9);
        floor.tiles_override.insert(pos, Tile::Floor);
        floor.tiles_override.insert(chasm_pos, Tile::Chasm);

        let enemy = EnemyState::new(101, EnemyKind::Crawler, pos, 1.0);
        floor.enemies.push(enemy);

        // Knock enemy North into Chasm
        let result = floor.hit_enemy_at(pos, 10, 0, Some(Direction::Up), "ZeHero", &mut false);
        assert!(result.is_some());
        let (_dmg, _crit, shards, events) = result.unwrap();
        assert_eq!(floor.enemies.iter().find(|e| e.id == 101).unwrap().hp, 0);
        assert!(shards > 0);
        assert!(events.iter().any(|ev| ev.contains("ABYSS EXECUTION")));
    }

    #[test]
    fn test_ascension_gate_seals_stairs_until_opened() {
        let mut server = ServerState::new(42);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let client_id = "test-climber".to_string();

        server.register_client(client_id.clone(), tx);
        server.handle_packet(
            &client_id,
            ClientPacket::Join {
                username: "Climber".to_string(),
                class_idx: 0,
                resume_key: None,
            },
        );

        // Receive Welcome packet
        let _ = rx.try_recv().expect("welcome packet");

        // Teleport player directly to stairs position while gate is still locked
        {
            let floor = server.floors.get_mut(&1).unwrap();
            let p = floor.players.get_mut(&client_id).unwrap();
            p.pos = floor.map.stairs_pos;
        }

        // Attempt ascension while gate is locked -> should be rejected!
        server.handle_packet(&client_id, ClientPacket::AscendStairs);
        assert_eq!(server.client_floors.get(&client_id).copied(), Some(1));

        // Should receive announcement about sealed gate
        let mut found_sealed_announcement = false;
        while let Ok(packet) = rx.try_recv() {
            if let ServerPacket::Announcement { text } = packet {
                if text.contains("The gate is sealed.") {
                    found_sealed_announcement = true;
                }
            }
        }
        assert!(found_sealed_announcement);

        // Now unlock all gates (e.g. pressure plate or guardian defeated)
        {
            let floor = server.floors.get_mut(&1).unwrap();
            for &(gate_id, gate_pos) in &floor.map.gates {
                floor
                    .tiles_override
                    .insert(gate_pos, Tile::Gate { id: gate_id, open: true });
            }
        }

        // Attempt ascension again -> should succeed to Floor 2!
        server.handle_packet(&client_id, ClientPacket::AscendStairs);
        assert_eq!(server.client_floors.get(&client_id).copied(), Some(2));
    }

    #[test]
    fn test_multi_plate_lock_coop_and_decoy() {
        let mut floor = FloorInstance::new(1, 42);
        let gate_pos = Position::new(20, 20);
        let plate1_pos = Position::new(21, 20);
        let plate2_pos = Position::new(22, 20);

        floor.map.gates = vec![(1, gate_pos)];
        floor.map.plates = vec![(1, plate1_pos), (1, plate2_pos)];
        floor.tiles_override.insert(gate_pos, Tile::Gate { id: 1, open: false });
        floor.tiles_override.insert(plate1_pos, Tile::PressurePlate { id: 1, active: false });
        floor.tiles_override.insert(plate2_pos, Tile::PressurePlate { id: 1, active: false });

        let p1 = PlayerState::new("p1".to_string(), "Alice".to_string(), 0, plate1_pos, 1);
        floor.add_player(p1);

        // Only plate 1 is occupied -> Gate 1 MUST remain locked!
        let unseal_events = floor.check_pressure_plates();
        assert!(unseal_events.is_empty());
        assert_eq!(floor.tiles_override.get(&gate_pos), Some(&Tile::Gate { id: 1, open: false }));

        // Now spawn Holo-Decoy on plate 1, and move Alice to plate 2
        floor.decoys.push(DecoyState::new(1, "Alice".to_string(), plate1_pos, 300));
        floor.players.get_mut("p1").unwrap().pos = plate2_pos;

        // Both plates are now held simultaneously (Plate 1 by Decoy, Plate 2 by Alice)!
        let unseal_events = floor.check_pressure_plates();
        assert!(!unseal_events.is_empty());
        assert!(unseal_events[0].contains("All 2 Pressure Plates"));
        assert_eq!(floor.tiles_override.get(&gate_pos), Some(&Tile::Gate { id: 1, open: true }));
        assert!(floor.unlocked_gates.contains(&1));
    }

    #[test]
    fn test_holo_decoy_mana_and_tick_expiration() {
        let mut server = ServerState::new(42);
        let (tx, _rx) = mpsc::unbounded_channel();
        let client_id = "decoy-wizard".to_string();

        server.register_client(client_id.clone(), tx);
        server.handle_packet(
            &client_id,
            ClientPacket::Join {
                username: "Wizard".to_string(),
                class_idx: 1, // Arcanist (120 Max Mana)
                resume_key: None,
            },
        );

        let floor = server.floors.get_mut(&1).unwrap();
        let p = floor.players.get_mut(&client_id).unwrap();
        p.mana = 30; // Set mana to 30

        // Deploy Holo-Decoy (costs 20 Mana)
        server.handle_packet(&client_id, ClientPacket::DeployDecoy);

        let floor = server.floors.get_mut(&1).unwrap();
        let p = floor.players.get(&client_id).unwrap();
        assert_eq!(p.mana, 10, "Deploying Holo-Decoy must deduct exactly 20 Mana");
        assert_eq!(floor.decoys.len(), 1, "Decoy must be spawned on the floor");
        assert_eq!(floor.decoys[0].remaining_ticks, 300, "Decoy lasts 300 ticks");

        // Step floor 299 ticks -> Decoy should still be alive with 1 tick left
        for _ in 0..299 {
            floor.step_tick(0);
        }
        assert_eq!(floor.decoys.len(), 1);
        assert_eq!(floor.decoys[0].remaining_ticks, 1);

        // Step 1 more tick -> Decoy expires and is removed!
        floor.step_tick(0);
        assert_eq!(floor.decoys.len(), 0, "Decoy must expire after 300 ticks");
    }

    #[test]
    fn test_bridge_building_spends_5_shards() {
        let mut server = ServerState::new(42);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let client_id = "bridge-builder".to_string();

        server.register_client(client_id.clone(), tx);
        server.handle_packet(
            &client_id,
            ClientPacket::Join {
                username: "Bob".to_string(),
                class_idx: 0,
                resume_key: None,
            },
        );
        let _ = rx.try_recv().expect("welcome");

        let pos = Position::new(10, 10);
        let chasm_pos = Position::new(11, 10); // East of pos

        {
            let floor = server.floors.get_mut(&1).unwrap();
            let p = floor.players.get_mut(&client_id).unwrap();
            p.pos = pos;
            p.shards = 8; // 8 shards
            floor.tiles_override.insert(chasm_pos, Tile::Chasm);
        }

        // Build Bridge East into Chasm
        server.handle_packet(&client_id, ClientPacket::BuildBridge(Direction::Right));

        {
            let floor = server.floors.get(&1).unwrap();
            let p = floor.players.get(&client_id).unwrap();
            assert_eq!(p.shards, 3, "Building a bridge must spend exactly 5 shards");
            assert_eq!(
                floor.get_effective_tile(chasm_pos),
                Some(Tile::Bridge),
                "Chasm must become a permanent Bridge tile"
            );
        }

        // Try building again with 3 shards (insufficient)
        let chasm_pos2 = Position::new(9, 10); // West of pos
        {
            let floor = server.floors.get_mut(&1).unwrap();
            floor.tiles_override.insert(chasm_pos2, Tile::Chasm);
        }

        server.handle_packet(&client_id, ClientPacket::BuildBridge(Direction::Left));

        {
            let floor = server.floors.get(&1).unwrap();
            let p = floor.players.get(&client_id).unwrap();
            assert_eq!(p.shards, 3, "Shards should not be deducted on failure");
            assert_eq!(floor.get_effective_tile(chasm_pos2), Some(Tile::Chasm));
        }

        // Check announcement about needing 5 shards
        let mut found_cost_msg = false;
        while let Ok(packet) = rx.try_recv() {
            if let ServerPacket::Announcement { text } = packet {
                if text.contains("Need 5 shards") {
                    found_cost_msg = true;
                }
            }
        }
        assert!(found_cost_msg);
    }

    #[test]
    fn test_shrine_unlock_and_server_banner() {
        let mut server = ServerState::new(42);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let client_id = "shrine-hero".to_string();

        server.register_client(client_id.clone(), tx);
        server.handle_packet(
            &client_id,
            ClientPacket::Join {
                username: "Hero".to_string(),
                class_idx: 0,
                resume_key: None,
            },
        );
        let _ = rx.try_recv().expect("welcome");

        // Move player to Floor 2
        server.client_floors.insert(client_id.clone(), 2);
        let floor2 = server.get_or_create_floor(2);
        let shrine_pos = floor2.map.shrine_pos.unwrap();
        let stairs_pos = floor2.map.stairs_pos;

        let player = PlayerState::new(client_id.clone(), "Hero".to_string(), 0, shrine_pos, 2);
        floor2.add_player(player);
        floor2.shrine_goal = 10;
        floor2.shrine_shards = 0;
        floor2.stairs_unlocked = false;

        // Player has 12 shards
        floor2.players.get_mut(&client_id).unwrap().shards = 12;

        // Unlock gates on floor 2 so only shrine locks the stairs
        for &(gid, gpos) in &floor2.map.gates.clone() {
            floor2.tiles_override.insert(gpos, Tile::Gate { id: gid, open: true });
        }

        // Try ascending while stairs are shrine-sealed
        floor2.players.get_mut(&client_id).unwrap().pos = stairs_pos;
        server.handle_packet(&client_id, ClientPacket::AscendStairs);
        assert_eq!(server.client_floors.get(&client_id).copied(), Some(2), "Ascent must be sealed");

        // Move back to shrine and deposit shards
        {
            let f2 = server.floors.get_mut(&2).unwrap();
            f2.players.get_mut(&client_id).unwrap().pos = shrine_pos;
        }

        // Deposit 10 shards to meet the goal
        server.handle_packet(&client_id, ClientPacket::DepositShards { amount: 10 });

        {
            let f2 = server.floors.get(&2).unwrap();
            assert_eq!(f2.shrine_shards, 10);
            assert!(f2.stairs_unlocked, "StairsUp must unlock when shrine goal is met");
        }

        // Verify ServerBanner popup packet was broadcast to client
        let mut banner_received = false;
        while let Ok(packet) = rx.try_recv() {
            if let ServerPacket::ServerBanner { title, message } = packet {
                if title.contains("AWAKENS") && message.contains("unsealed") {
                    banner_received = true;
                }
            }
        }
        assert!(banner_received, "ServerBanner must be received across terminals on shrine fulfillment");

        // Now move to stairs and ascend -> should succeed to Floor 3!
        {
            let f2 = server.floors.get_mut(&2).unwrap();
            f2.players.get_mut(&client_id).unwrap().pos = stairs_pos;
        }

        server.handle_packet(&client_id, ClientPacket::AscendStairs);
        assert_eq!(server.client_floors.get(&client_id).copied(), Some(3), "Ascent must succeed to Floor 3");
    }

    #[test]
    fn test_text_sanitization_security() {
        use super::sanitize_text;

        // Strip ANSI escape codes
        let malicious_ansi = "\x1b[2J\x1b[H\x1b[31mHacker\x1b[0m";
        let clean = sanitize_text(malicious_ansi, 24);
        assert_eq!(clean, "[2J[H[31mHacker[0m");
        assert!(!clean.contains('\x1b'));

        // Strip control characters (tabs, newlines, nulls)
        let control_chars = "Hello\x00\r\n\tWorld!";
        let clean_ctrl = sanitize_text(control_chars, 30);
        assert_eq!(clean_ctrl, "HelloWorld!");

        // Enforce max length truncation
        let long_string = "a".repeat(200);
        let truncated = sanitize_text(&long_string, 20);
        assert_eq!(truncated.len(), 20);
    }

    #[test]
    fn test_lightning_tether_damage_and_shards() {
        let mut server = ServerState::new(123);
        let f = server.get_or_create_floor(1);

        let mut p1 = PlayerState::new("p1".to_string(), "Alice".to_string(), 0, Position::new(10, 10), 1);
        p1.tether_target = Some("p2".to_string());
        let p2 = PlayerState::new("p2".to_string(), "Bob".to_string(), 1, Position::new(14, 10), 1);

        f.add_player(p1);
        f.add_player(p2);

        // Place an enemy right between Alice and Bob at (12, 10)
        let enemy = EnemyState::new(99, EnemyKind::Crawler, Position::new(12, 10), 1.0);
        f.enemies.push(enemy);

        // Step ticks to trigger tether cycle (even tick)
        f.step_tick(0);
        f.step_tick(0);

        // Crawler has 30 HP, takes 8 DMG -> HP = 22
        let enemy = f.enemies.iter().find(|e| e.id == 99).unwrap();
        assert_eq!(enemy.hp, 18, "Enemy intersected by tether must take damage");

        // Verify spark projectile exists on the line
        assert!(f.projectiles.iter().any(|(p, _)| p.pos == Position::new(12, 10) && matches!(p.glyph, '↯' | '⠶' | '*')));
    }

    #[test]
    fn test_coop_ally_revival() {
        let mut server = ServerState::new(123);
        let (tx1, _rx1) = mpsc::unbounded_channel();
        let (tx2, _rx2) = mpsc::unbounded_channel();

        server.register_client("hero-1".to_string(), tx1);
        server.register_client("hero-2".to_string(), tx2);

        server.client_floors.insert("hero-1".to_string(), 1);
        server.client_floors.insert("hero-2".to_string(), 1);

        let p1 = PlayerState::new("hero-1".to_string(), "Alice".to_string(), 0, Position::new(10, 10), 1);
        let mut p2 = PlayerState::new("hero-2".to_string(), "Bob".to_string(), 1, Position::new(11, 10), 1);
        p2.hp = 0; // Fallen hero

        {
            let f = server.get_or_create_floor(1);
            f.add_player(p1);
            f.add_player(p2);
            assert!(!f.players.get("hero-2").unwrap().is_alive());
        }

        // Alice revives Bob
        server.handle_packet("hero-1", ClientPacket::ReviveAlly { target_id: "hero-2".to_string() });

        let f = server.get_or_create_floor(1);
        let revived_bob = f.players.get("hero-2").unwrap();
        assert!(revived_bob.is_alive());
        assert_eq!(revived_bob.hp, revived_bob.max_hp / 2);
    }
}
