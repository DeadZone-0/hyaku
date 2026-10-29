use crate::state::ServerState;
use hyaku_protocol::ServerPacket;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{interval, MissedTickBehavior};

pub async fn run_tick_loop(state: Arc<RwLock<ServerState>>) {
    let mut ticker = interval(Duration::from_millis(50));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        ticker.tick().await;

        let mut server = state.write().await;
        let tower_shards = server.tower_shards_bank;

        // Determine all active floors that currently have players
        let mut active_floors: HashSet<u32> = HashSet::new();
        for &floor_num in server.client_floors.values() {
            active_floors.insert(floor_num);
        }

        // Always step floor 1 even if empty to keep world clock ticking
        active_floors.insert(1);

        let mut floor_ticks = Vec::new();
        for &floor_num in &active_floors {
            let floor_inst = server.get_or_create_floor(floor_num);
            let tick_state = floor_inst.step_tick(tower_shards);
            floor_ticks.push((floor_num, tick_state));
        }

        // Broadcast Tick packets to respective floors
        for (floor_num, tick_state) in floor_ticks {
            server.broadcast_to_floor(floor_num, ServerPacket::Tick(tick_state));
        }
    }
}
