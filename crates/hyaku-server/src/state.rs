use hyaku_protocol::{
    ClientPacket, DecoyState, Direction, EnemyKind, EnemyState, FloorCarving, FloorMap, PlayerState,
    ProjectileState, ServerPacket, TickState, Tile, Position,
};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
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

pub struct FloorInstance {
    pub floor: u32,
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
        } else {
            (10 + (floor - 2) * 5, false)
        };

        Self {
            floor,
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

    fn check_walkable(
        map: &FloorMap,
        tiles_override: &HashMap<Position, Tile>,
        pos: Position,
    ) -> bool {
        Self::resolve_tile(map, tiles_override, pos).map_or(false, |t| t.is_walkable())
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

        if !self.is_walkable(target_pos) {
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
    ) -> Option<(i32, bool, u32, Vec<String>)> {
        let enemy_idx = self
            .enemies
            .iter()
            .position(|e| e.is_alive() && e.pos == target_pos)?;

        let enemy_id = self.enemies[enemy_idx].id;
        let enemy_name = self.enemies[enemy_idx].kind.name();
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

        self.enemies[enemy_idx].take_damage(actual_dmg);

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
                            .position(|e| e.is_alive() && e.pos == pushed_pos);
                        if let Some(other_idx) = occupied {
                            extra_dmg += 5;
                            self.enemies[enemy_idx].take_damage(5);
                            self.enemies[other_idx].take_damage(5);
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

        // Campfire & Shrine regeneration every 20 ticks (1 sec at 20-TPS)
        if self.tick_counter % 20 == 0 {
            for player in self.players.values_mut() {
                if !player.is_alive() {
                    continue;
                }
                match Self::resolve_tile(&self.map, &self.tiles_override, player.pos) {
                    Some(Tile::Campfire) => {
                        player.heal(6);
                        player.restore_mana(6);
                    }
                    Some(Tile::Shrine) => {
                        player.heal(player.max_hp);
                        player.restore_mana(player.max_mana);
                    }
                    _ => {}
                }
            }
        }

        // Enemy AI every 10 ticks (0.5 sec)
        if self.tick_counter % 10 == 0 && !self.players.is_empty() {
            let mut attacks = Vec::new();

            for enemy in self.enemies.iter_mut() {
                if !enemy.is_alive() {
                    continue;
                }

                // Check active Holo-Decoys first (distraction target)
                let mut closest_target: Option<Position> = None;
                let mut min_dist = u32::MAX;

                for decoy in &self.decoys {
                    let d = enemy.pos.manhattan_distance(decoy.pos);
                    if d < min_dist && d <= 12 {
                        min_dist = d;
                        closest_target = Some(decoy.pos);
                    }
                }

                // If no nearby decoy, target nearest alive player
                if closest_target.is_none() {
                    for player in self.players.values() {
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

                if let Some(target) = closest_target {
                    if min_dist == 1 {
                        let dmg = match enemy.kind {
                            EnemyKind::Crawler => 8,
                            EnemyKind::Archer => 12,
                            EnemyKind::Brute => 22,
                            EnemyKind::Phantom => 16,
                            EnemyKind::FloorBoss => 30,
                        };
                        attacks.push((target, dmg));
                    } else if enemy.kind == EnemyKind::Archer && min_dist >= 2 && min_dist <= 5 {
                        attacks.push((target, 12));
                        self.projectiles.push((
                            ProjectileState::new(target, '*', (140, 200, 80)),
                            2,
                        ));
                    } else if min_dist <= 12 {
                        // Move one step toward target
                        if let Some(step_dir) = enemy.pos.direction_to(target) {
                            let next_pos = enemy.pos.step(step_dir);
                            if Self::check_walkable(&self.map, &self.tiles_override, next_pos) {
                                enemy.pos = next_pos;
                            }
                        }
                    }
                }
            }

            // Apply queued enemy attack damage
            for (target_pos, dmg) in attacks {
                self.projectiles.push((
                    ProjectileState::new(target_pos, '!', (255, 60, 60)),
                    2,
                ));
                for player in self.players.values_mut() {
                    if player.is_alive() && player.pos == target_pos {
                        player.take_damage(dmg);
                        println!(
                            "[HYAKU COMBAT] Enemy struck '{}' at ({}, {}) for {} DMG! HP: {}/{}",
                            player.username, target_pos.x, target_pos.y, dmg, player.hp, player.max_hp
                        );
                        break;
                    }
                }
            }
        }

        // Lightning Tether electrical arc damage and energetic spark particles
        if self.tick_counter % 2 == 0 {
            let mut processed_pairs: HashSet<(String, String)> = HashSet::new();
            let mut tether_events = Vec::new();

            let player_tethers: Vec<(String, String, Position, String)> = self
                .players
                .values()
                .filter(|p| p.is_alive() && p.tether_target.is_some())
                .map(|p| (p.id.clone(), p.tether_target.clone().unwrap(), p.pos, p.username.clone()))
                .collect();

            for (p1_id, target_id, p1_pos, p1_name) in player_tethers {
                let pair_key = if p1_id < target_id {
                    (p1_id.clone(), target_id.clone())
                } else {
                    (target_id.clone(), p1_id.clone())
                };

                if processed_pairs.contains(&pair_key) {
                    continue;
                }
                processed_pairs.insert(pair_key);

                if let Some(target_player) = self.players.get(&target_id) {
                    if target_player.is_alive() && p1_pos.manhattan_distance(target_player.pos) <= 14 {
                        let target_pos = target_player.pos;
                        let target_name = target_player.username.clone();
                        let line_points = p1_pos.line_to(target_pos);

                        for pt in line_points {
                            if pt == p1_pos || pt == target_pos {
                                continue;
                            }
                            // Spawn energetic spark particle along tether beam
                            self.projectiles.push((
                                ProjectileState::new(pt, '⚡', (120, 240, 255)),
                                1,
                            ));

                            // Electrocute enemies intersecting the beam
                            for enemy in self.enemies.iter_mut() {
                                if enemy.is_alive() && enemy.pos == pt {
                                    enemy.take_damage(8);
                                    let dead = !enemy.is_alive();
                                    if dead {
                                        let enemy_name = enemy.kind.name();
                                        tether_events.push((
                                            p1_id.clone(),
                                            target_id.clone(),
                                            format!(":: TETHER DISCHARGE :: @{} and @{} electrocuted {}!", p1_name, target_name, enemy_name),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            for (p1_id, p2_id, event) in tether_events {
                self.pending_announcements.push(event);
                if let Some(p1) = self.players.get_mut(&p1_id) {
                    p1.shards += 2;
                }
                if let Some(p2) = self.players.get_mut(&p2_id) {
                    p2.shards += 2;
                }
            }
        }

        // Projectile decay (lasts specified TTL ticks)
        self.projectiles.retain_mut(|(proj, ttl)| {
            if *ttl > 0 && proj.pos.in_bounds(self.map.width, self.map.height) {
                *ttl -= 1;
                true
            } else {
                false
            }
        });

        // Remove dead non-boss enemies and reward floor players
        let prev_boss_alive = self.enemies.iter().any(|e| e.is_boss && e.is_alive());
        self.enemies.retain(|e| e.is_alive());
        let boss_still_alive = self.enemies.iter().any(|e| e.is_boss && e.is_alive());

        // If boss was defeated, open all gates
        if prev_boss_alive && !boss_still_alive {
            println!("[HYAKU BOSS] Floor {} boss has fallen! All barrier gates unlocked.", self.floor);
            for &(gate_id, gate_pos) in &self.map.gates {
                self.unlocked_gates.insert(gate_id);
                self.tiles_override
                    .insert(gate_pos, Tile::Gate { id: gate_id, open: true });
            }
        }

        TickState {
            tick: self.tick_counter,
            floor: self.floor,
            players: self.players.values().cloned().collect(),
            enemies: self.enemies.clone(),
            projectiles: self.projectiles.iter().map(|(p, _)| p.clone()).collect(),
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
}

impl ServerState {
    pub fn new(seed: u64) -> Self {
        let mut state = Self {
            clients: HashMap::new(),
            client_floors: HashMap::new(),
            floors: HashMap::new(),
            world_seed: seed,
            tower_shards_bank: 0,
        };
        // Pre-create floor 1
        state.floors.insert(1, FloorInstance::new(1, seed));
        state
    }

    pub fn get_or_create_floor(&mut self, floor: u32) -> &mut FloorInstance {
        self.floors
            .entry(floor)
            .or_insert_with(|| FloorInstance::new(floor, self.world_seed))
    }

    pub fn register_client(&mut self, player_id: String, sender: UnboundedSender<ServerPacket>) {
        self.clients.insert(player_id, sender);
    }

    pub fn unregister_client(&mut self, player_id: &str) {
        self.clients.remove(player_id);
        if let Some(floor_num) = self.client_floors.remove(player_id) {
            if let Some(floor) = self.floors.get_mut(&floor_num) {
                if let Some(player) = floor.remove_player(player_id) {
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

    pub fn handle_packet(&mut self, player_id: &str, packet: ClientPacket) {
        match packet {
            ClientPacket::Join { username, class_idx } => {
                let clean_name = sanitize_text(&username, 24);
                let username = if clean_name.is_empty() {
                    "Climber".to_string()
                } else {
                    clean_name
                };

                let start_floor = 1;
                let world_seed = self.world_seed;
                let floor_inst = self.get_or_create_floor(start_floor);
                let spawn_pos = floor_inst.map.spawn_pos;

                let player = PlayerState::new(
                    player_id.to_string(),
                    username.clone(),
                    class_idx,
                    spawn_pos,
                    start_floor,
                );

                floor_inst.add_player(player);
                self.client_floors.insert(player_id.to_string(), start_floor);

                if let Some(tx) = self.clients.get(player_id) {
                    let _ = tx.send(ServerPacket::Welcome {
                        player_id: player_id.to_string(),
                        floor: start_floor,
                        map_seed: world_seed,
                    });
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
                        let player_info = floor.players.get(player_id).and_then(|p| {
                            if p.is_alive() {
                                Some((p.class_idx, p.username.clone(), p.pos))
                            } else {
                                None
                            }
                        });

                        if let Some((class_idx, player_name, start_pos)) = player_info {
                            let class_preset = hyaku_protocol::get_class_preset(class_idx);
                            let base_dmg = class_preset.attack_power;

                            let (max_reach, crit_chance, slash_glyph) = match class_idx {
                                1 => (3, 25, '*'),
                                2 => (4, 35, dir.arrow_glyph()),
                                _ => (
                                    1,
                                    20,
                                    match dir {
                                        Direction::Up | Direction::Down => '|',
                                        Direction::Left | Direction::Right => '-',
                                    },
                                ),
                            };

                            let mut target_positions = Vec::new();
                            let mut current = start_pos;
                            let mut shards_earned = 0;

                            for _ in 1..=max_reach {
                                current = current.step(dir);
                                if !current.in_bounds(floor.map.width, floor.map.height) {
                                    break;
                                }
                                target_positions.push(current);

                                if let Some((_dmg, _crit, shards, events)) = floor.hit_enemy_at(
                                    current,
                                    base_dmg,
                                    crit_chance,
                                    Some(dir),
                                    &player_name,
                                ) {
                                    shards_earned += shards;
                                    announcements.extend(events);
                                    break;
                                }

                                if !floor.is_walkable(current) {
                                    break;
                                }
                            }

                            for &pos in &target_positions {
                                floor.projectiles.push((
                                    ProjectileState::new(pos, slash_glyph, class_preset.color_rgb),
                                    3,
                                ));
                            }

                            if shards_earned > 0 {
                                if let Some(p) = floor.players.get_mut(player_id) {
                                    p.shards += shards_earned;
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
                                Some((p.class_idx, p.username.clone(), p.pos, p.mana))
                            } else {
                                None
                            }
                        });

                        if let Some((class_idx, player_name, start_pos, cur_mana)) = player_info {
                            let class_preset = hyaku_protocol::get_class_preset(class_idx);
                            let base_dmg = class_preset.attack_power;

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
                                    let target_name = floor.players.get(tid).map(|p| p.username.clone()).unwrap_or_else(|| "Ally".to_string());
                                    tether_msg = Some(format!("* [CO-OP] @{} forged an Aetheric Lightning Tether with @{}! Electric arcing connects you! *", my_name, target_name));
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

                                // Holo-Decoy: 15 seconds at 20-TPS = 300 ticks
                                let decoy = DecoyState::new(decoy_id, username.clone(), pos, 300);
                                floor.decoys.push(decoy);

                                floor.projectiles.push((
                                    ProjectileState::new(pos, '§', (120, 220, 255)),
                                    6,
                                ));

                                unseal_events = floor.check_pressure_plates();

                                println!(
                                    "[HYAKU ACTION] Player '{}' deployed Holo-Decoy at ({}, {}) (-20 MP, 15s)",
                                    username, pos.x, pos.y
                                );

                                deploy_announcement = Some(format!(
                                    "* [HOLO-DECOY] @{} projected a Holo-Decoy at ({}, {})! (Lasts 15s, holds pressure plates) *",
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

                            if !near_shrine {
                                direct_msg = Some("You must stand near the Floor Shrine [百] to deposit shards!".to_string());
                            } else {
                                let deposit = amount.min(shards);
                                if deposit > 0 {
                                    if let Some(p) = floor.players.get_mut(player_id) {
                                        p.shards -= deposit;
                                    }
                                    floor.shrine_shards += deposit;
                                    self.tower_shards_bank += deposit;

                                    println!(
                                        "[HYAKU SHRINE] Player '{}' deposited {} shards into Floor {} Shrine ({}/{} Shards). Bank: {}",
                                        username, deposit, floor_num, floor.shrine_shards, floor.shrine_goal, self.tower_shards_bank
                                    );

                                    deposit_announcement = Some(format!(
                                        "* [SHRINE] @{} offered {} Shards to Floor Shrine [百]! Progress: {}/{} Shards *",
                                        username, deposit, floor.shrine_shards, floor.shrine_goal
                                    ));

                                    if floor.shrine_shards >= floor.shrine_goal && !floor.stairs_unlocked {
                                        floor.stairs_unlocked = true;
                                        unsealed_stairs = true;
                                        banner_payload = Some((
                                            format!("★ [百] TOWER FLOOR {} SHRINE FULFILLED! ★", floor_num),
                                            format!(
                                                "The shard offering ({}/{}) is complete! The Ascension Stairs have been UNSEALED!",
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

            ClientPacket::LightCampfire => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        let player_info = floor.players.get(player_id).map(|p| {
                            (p.pos, p.mana, p.shards, p.username.clone())
                        });

                        if let Some((pos, mana, shards, username)) = player_info {
                            if let Some(Tile::Campfire) = floor.get_effective_tile(pos) {
                                if let Some(tx) = self.clients.get(player_id) {
                                    let _ = tx.send(ServerPacket::Announcement {
                                        text: "You are already resting at a campfire!".to_string(),
                                    });
                                }
                                return;
                            }

                            // Cost: 15 Mana or 2 Shards
                            let mut paid = false;
                            if let Some(p) = floor.players.get_mut(player_id) {
                                if p.spend_mana(15) {
                                    paid = true;
                                } else if p.shards >= 2 {
                                    p.shards -= 2;
                                    paid = true;
                                }
                            }

                            if paid {
                                // Limit: 1 active campfire per player (replace previous)
                                if let Some(old_pos) = floor.player_campfires.remove(player_id) {
                                    floor.tiles_override.remove(&old_pos);
                                }
                                floor.tiles_override.insert(pos, Tile::Campfire);
                                floor.player_campfires.insert(player_id.to_string(), pos);

                                println!(
                                    "[HYAKU ACTION] Player '{}' lit a resting campfire at ({}, {})",
                                    username, pos.x, pos.y
                                );
                                self.broadcast_to_floor(
                                    floor_num,
                                    ServerPacket::Announcement {
                                        text: format!(
                                            "* [CO-OP] @{} kindled a resting Campfire! Rest nearby to recover HP and Mana! *",
                                            username
                                        ),
                                    },
                                );
                            } else {
                                if let Some(tx) = self.clients.get(player_id) {
                                    let _ = tx.send(ServerPacket::Announcement {
                                        text: format!("Need 15 Mana or 2 Shards to light a Campfire! (MP: {}, Shards: {})", mana, shards),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            ClientPacket::CarveFloor { message } => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        if let Some(player) = floor.players.get(player_id) {
                            let clean_msg = sanitize_text(&message, 80);
                            if !clean_msg.is_empty() {
                                let author = player.username.clone();
                                let carving = FloorCarving::new(player.pos, author.clone(), clean_msg.clone());
                                floor.carvings.push(carving);
                                println!(
                                    "[HYAKU CARVING] Player '{}' carved message at ({}, {}): '{}'",
                                    author, player.pos.x, player.pos.y, clean_msg
                                );
                                if let Some(tx) = self.clients.get(player_id) {
                                    let _ = tx.send(ServerPacket::Announcement {
                                        text: "Etched a message into the ancient stone floor.".to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            ClientPacket::UpvoteCarving { pos } => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        for carving in floor.carvings.iter_mut() {
                            if carving.pos == pos {
                                carving.upvotes = carving.upvotes.saturating_add(1);
                                if let Some(tx) = self.clients.get(player_id) {
                                    let _ = tx.send(ServerPacket::Announcement {
                                        text: format!("Upvoted message by {} (now {} upvotes)", carving.author, carving.upvotes),
                                    });
                                }
                                break;
                            }
                        }
                    }
                }
            }

            ClientPacket::AscendStairs => {
                if let Some(&current_floor) = self.client_floors.get(player_id) {
                    let mut player_opt = None;
                    let mut can_ascend = false;
                    let mut rejection_msg = None;

                    if let Some(floor) = self.floors.get_mut(&current_floor) {
                        if let Some(player) = floor.players.get(player_id) {
                            let tile = floor.get_effective_tile(player.pos);
                            if tile == Some(Tile::StairsUp) || player.pos == floor.map.stairs_pos {
                                // Check if any Ascension Gate on this floor is still locked
                                let is_gate_locked = floor.map.gates.iter().any(|&(_id, gate_pos)| {
                                    matches!(
                                        floor.get_effective_tile(gate_pos),
                                        Some(Tile::Gate { open: false, .. })
                                    )
                                });

                                if is_gate_locked {
                                    rejection_msg = Some(":: ASCENSION SEALED :: The gate to the stairs is locked! Unlock it via the Pressure Plate or by slaying the Floor Guardian!".to_string());
                                } else if !floor.stairs_unlocked {
                                    rejection_msg = Some(format!(
                                        ":: SHRINE SEALED :: The Stairs are sealed until the Shrine [百] offering is fulfilled ({}/{} Shards)!",
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
                        }
                    }
                }
            }

            ClientPacket::Respawn => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        let respawn_pos = floor
                            .player_campfires
                            .get(player_id)
                            .copied()
                            .unwrap_or(floor.map.spawn_pos);

                        let mut username = String::new();
                        if let Some(player) = floor.players.get_mut(player_id) {
                            if !player.is_alive() {
                                player.pos = respawn_pos;
                                player.hp = player.max_hp;
                                player.mana = player.max_mana;
                                username = player.username.clone();
                            }
                        }

                        if !username.is_empty() {
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
        let result = floor.hit_enemy_at(pos, 10, 0, Some(Direction::Up), "ZeHero");
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
                if text.contains("ASCENSION SEALED") {
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
        assert_eq!(floor.decoys[0].remaining_ticks, 300, "Decoy must have 15s (300 ticks) duration");

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
                if title.contains("SHRINE FULFILLED") && message.contains("UNSEALED") {
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
        assert_eq!(enemy.hp, 22, "Enemy intersected by tether must take damage");

        // Verify spark projectile exists on the line
        assert!(f.projectiles.iter().any(|(p, _)| p.pos == Position::new(12, 10) && p.glyph == '⚡'));
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
