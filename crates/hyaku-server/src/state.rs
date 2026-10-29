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
    pub projectiles: Vec<ProjectileState>,
    pub tiles_override: HashMap<Position, Tile>,
    pub carvings: Vec<FloorCarving>,
    pub tick_counter: u64,
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

    pub fn step_tick(&mut self, tower_shards: u32) -> TickState {
        self.tick_counter = self.tick_counter.wrapping_add(1);

        // Campfire regeneration every 20 ticks (1 sec at 20-TPS)
        if self.tick_counter % 20 == 0 {
            for player in self.players.values_mut() {
                if !player.is_alive() {
                    continue;
                }
                if let Some(Tile::Campfire) =
                    Self::resolve_tile(&self.map, &self.tiles_override, player.pos)
                {
                    player.heal(5);
                    player.restore_mana(5);
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

                // Find nearest player
                let mut closest_player: Option<Position> = None;
                let mut min_dist = u32::MAX;

                for player in self.players.values() {
                    if !player.is_alive() {
                        continue;
                    }
                    let dist = enemy.pos.manhattan_distance(player.pos);
                    if dist < min_dist {
                        min_dist = dist;
                        closest_player = Some(player.pos);
                    }
                }

                if let Some(target) = closest_player {
                    if min_dist == 1 {
                        let dmg = match enemy.kind {
                            EnemyKind::Crawler => 8,
                            EnemyKind::Archer => 12,
                            EnemyKind::Brute => 22,
                            EnemyKind::Phantom => 16,
                            EnemyKind::FloorBoss => 30,
                        };
                        attacks.push((target, dmg));
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
                for player in self.players.values_mut() {
                    if player.is_alive() && player.pos == target_pos {
                        player.take_damage(dmg);
                        break;
                    }
                }
            }
        }

        // Projectile updates
        self.projectiles.retain_mut(|proj| {
            // Expire after moving or hit
            proj.pos.in_bounds(self.map.width, self.map.height)
        });

        // Remove dead non-boss enemies and reward floor players
        let prev_boss_alive = self.enemies.iter().any(|e| e.is_boss && e.is_alive());
        self.enemies.retain(|e| e.is_alive());
        let boss_still_alive = self.enemies.iter().any(|e| e.is_boss && e.is_alive());

        // If boss was defeated, open all gates
        if prev_boss_alive && !boss_still_alive {
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
            projectiles: self.projectiles.clone(),
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
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        if let Some(player) = floor.players.get_mut(player_id) {
                            if !player.is_alive() {
                                return;
                            }
                            let target_pos = player.pos.step(dir);
                            let mut shard_reward = 0;

                            for enemy in floor.enemies.iter_mut() {
                                if enemy.is_alive() && enemy.pos == target_pos {
                                    let dmg = 25;
                                    enemy.take_damage(dmg);
                                    if !enemy.is_alive() {
                                        shard_reward += if enemy.is_boss { 10 } else { 2 };
                                    }
                                }
                            }

                            if shard_reward > 0 {
                                player.shards += shard_reward;
                            }
                        }
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
                            if player.spend_mana(20) {
                                floor.projectiles.push(ProjectileState::new(
                                    player.pos,
                                    '?',
                                    (120, 200, 255),
                                ));
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
                            player.shards -= deposit;
                            self.tower_shards_bank += deposit;
                        }
                    }
                }
            }

            ClientPacket::BuildBridge(dir) => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        if let Some(player) = floor.players.get_mut(player_id) {
                            let target_pos = player.pos.step(dir);
                            if let Some(tile) = floor.get_effective_tile(target_pos) {
                                if tile.is_chasm() {
                                    floor.tiles_override.insert(target_pos, Tile::Bridge);
                                }
                            }
                        }
                    }
                }
            }

            ClientPacket::LightCampfire => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        if let Some(player) = floor.players.get_mut(player_id) {
                            floor.tiles_override.insert(player.pos, Tile::Campfire);
                        }
                    }
                }
            }

            ClientPacket::CarveFloor { message } => {
                if let Some(&floor_num) = self.client_floors.get(player_id) {
                    if let Some(floor) = self.floors.get_mut(&floor_num) {
                        if let Some(player) = floor.players.get(player_id) {
                            let carving =
                                FloorCarving::new(player.pos, player.username.clone(), message);
                            floor.carvings.push(carving);
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

                    if let Some(floor) = self.floors.get_mut(&current_floor) {
                        if let Some(player) = floor.players.get(player_id) {
                            let tile = floor.get_effective_tile(player.pos);
                            if tile == Some(Tile::StairsUp) || player.pos == floor.map.stairs_pos {
                                can_ascend = true;
                            }
                        }
                        if can_ascend {
                            player_opt = floor.remove_player(player_id);
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
}
