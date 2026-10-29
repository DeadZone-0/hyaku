use hyaku_protocol::{
    ClientPacket, Direction, EnemyKind, EnemyState, FloorCarving, FloorMap, PlayerState,
    ProjectileState, ServerPacket, TickState, Tile, Position,
};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedSender;

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

        Self {
            floor,
            map,
            players: HashMap::new(),
            enemies,
            projectiles: Vec::new(),
            tiles_override,
            carvings: Vec::new(),
            tick_counter: 0,
            player_campfires: HashMap::new(),
        }
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

    pub fn move_player(&mut self, player_id: &str, dir: Direction) -> bool {
        let (current_pos, is_alive) = match self.players.get(player_id) {
            Some(p) => (p.pos, p.is_alive()),
            None => return false,
        };

        if !is_alive {
            return false;
        }

        let target_pos = current_pos.step(dir);
        if !target_pos.in_bounds(self.map.width, self.map.height) {
            return false;
        }

        if !self.is_walkable(target_pos) {
            return false;
        }

        if let Some(player) = self.players.get_mut(player_id) {
            player.pos = target_pos;
        }

        // Check if stepped on a pressure plate
        if let Some(Tile::PressurePlate { id, active }) = self.get_effective_tile(target_pos) {
            if !active {
                self.tiles_override
                    .insert(target_pos, Tile::PressurePlate { id, active: true });

                // Open all corresponding gates
                for &(gate_id, gate_pos) in &self.map.gates {
                    if gate_id == id {
                        self.tiles_override
                            .insert(gate_pos, Tile::Gate { id: gate_id, open: true });
                    }
                }
            }
        }

        true
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

                for (proj, _) in &self.projectiles {
                    if proj.glyph == '?' {
                        let d = enemy.pos.manhattan_distance(proj.pos);
                        if d < min_dist && d <= 12 {
                            min_dist = d;
                            closest_target = Some(proj.pos);
                        }
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
                        floor.move_player(player_id, dir);
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
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        if let Some(player) = floor.players.get_mut(player_id) {
                            player.tether_target = target_id;
                        }
                    }
                }
            }

            ClientPacket::DeployDecoy => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        if let Some(player) = floor.players.get_mut(player_id) {
                            if player.spend_mana(25) {
                                floor.projectiles.push((
                                    ProjectileState::new(
                                        player.pos,
                                        '?',
                                        (120, 200, 255),
                                    ),
                                    200, // 200 ticks = 10 seconds decoy
                                ));
                                println!(
                                    "[HYAKU ACTION] Player '{}' deployed Holo-Decoy at ({}, {}) (-25 MP)",
                                    player.username, player.pos.x, player.pos.y
                                );
                                if let Some(tx) = self.clients.get(player_id) {
                                    let _ = tx.send(ServerPacket::Announcement {
                                        text: "Holo-Decoy deployed! (-25 MP, lasts 10s)".to_string(),
                                    });
                                }
                            } else {
                                if let Some(tx) = self.clients.get(player_id) {
                                    let _ = tx.send(ServerPacket::Announcement {
                                        text: format!("Need 25 Mana for Holo-Decoy! (You have {})", player.mana),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            ClientPacket::DepositShards { amount } => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        if let Some(player) = floor.players.get_mut(player_id) {
                            let deposit = amount.min(player.shards);
                            if deposit > 0 {
                                player.shards -= deposit;
                                self.tower_shards_bank += deposit;
                                println!(
                                    "[HYAKU BANK] Player '{}' deposited {} shards. Total Tower Bank: {}",
                                    player.username, deposit, self.tower_shards_bank
                                );
                                if let Some(tx) = self.clients.get(player_id) {
                                    let _ = tx.send(ServerPacket::Announcement {
                                        text: format!("Deposited {} shards! Total Tower Bank: {}", deposit, self.tower_shards_bank),
                                    });
                                }
                            } else {
                                if let Some(tx) = self.clients.get(player_id) {
                                    let _ = tx.send(ServerPacket::Announcement {
                                        text: "You have 0 shards to deposit!".to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            ClientPacket::BuildBridge(dir) => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        let player_info = floor.players.get(player_id).map(|p| {
                            (p.pos, p.class_idx, p.shards, p.username.clone())
                        });

                        if let Some((pos, class_idx, shards, username)) = player_info {
                            let target_pos = pos.step(dir);
                            let tile = floor.get_effective_tile(target_pos);
                            if tile.map_or(true, |t| !t.is_chasm()) {
                                if let Some(tx) = self.clients.get(player_id) {
                                    let _ = tx.send(ServerPacket::Announcement {
                                        text: "No chasm in front of you to bridge!".to_string(),
                                    });
                                }
                                return;
                            }

                            // Architect class gets discount: 1 shard vs 3 shards
                            let cost = if class_idx == 3 { 1 } else { 3 };
                            if shards >= cost {
                                if let Some(p) = floor.players.get_mut(player_id) {
                                    p.shards -= cost;
                                }
                                floor.tiles_override.insert(target_pos, Tile::Bridge);
                                println!(
                                    "[HYAKU ACTION] Player '{}' built bridge at ({}, {}) (-{} shards)",
                                    username, target_pos.x, target_pos.y, cost
                                );
                                if let Some(tx) = self.clients.get(player_id) {
                                    let _ = tx.send(ServerPacket::Announcement {
                                        text: format!("Bridge constructed! (-{} shards)", cost),
                                    });
                                }
                            } else {
                                if let Some(tx) = self.clients.get(player_id) {
                                    let _ = tx.send(ServerPacket::Announcement {
                                        text: format!("Need {} shards to build a bridge! (You have {})", cost, shards),
                                    });
                                }
                            }
                        }
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
                                if let Some(tx) = self.clients.get(player_id) {
                                    let _ = tx.send(ServerPacket::Announcement {
                                        text: "Lit resting Campfire! (Regenerates HP/MP, max 1 active)".to_string(),
                                    });
                                }
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
                            let clean_msg = message.trim();
                            if !clean_msg.is_empty() {
                                let author = player.username.clone();
                                let carving = FloorCarving::new(player.pos, author.clone(), clean_msg.to_string());
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
                                    rejection_msg = Some(":: ASCENSION SEALED :: The gate to the stairs is locked! Unlock it via the Pressure Plate or by slaying the Floor Guardian!");
                                } else {
                                    can_ascend = true;
                                }
                            } else {
                                rejection_msg = Some("You must stand on the Stairs to ascend!");
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
                            p.last_chat = Some((text.clone(), now));
                            p.username.clone()
                        })
                        .unwrap_or_else(|| "Adventurer".to_string());

                    self.broadcast_to_floor(
                        floor_num,
                        ServerPacket::ChatMessage {
                            sender: sender_name,
                            text,
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
}
