mod boss;
pub mod combat;
mod db;
mod state;
mod tick;

use crate::state::ServerState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, State,
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    serve::ListenerExt,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use hyaku_protocol::{ClientPacket, ServerPacket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Instant};

static NEXT_CLIENT_ID: AtomicU64 = AtomicU64::new(1);
static CONNECTIONS: Mutex<Vec<IpAddr>> = Mutex::new(Vec::new());

const MAX_CONNECTIONS: usize = 500;
const MAX_CONNECTIONS_PER_IP: usize = 8;
const MAX_PACKET_BYTES: usize = 64 * 1024;
const MAX_PACKETS_PER_SEC: u32 = 50;
const OUTBOX_SIZE: usize = 64;
const JOIN_TIMEOUT: Duration = Duration::from_secs(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

struct ConnectionSlot(IpAddr);

impl ConnectionSlot {
    fn claim(ip: IpAddr) -> Option<Self> {
        let mut connections = CONNECTIONS.lock().unwrap();
        if connections.len() >= MAX_CONNECTIONS
            || connections.iter().filter(|&&peer| peer == ip).count() >= MAX_CONNECTIONS_PER_IP
        {
            return None;
        }
        connections.push(ip);
        Some(Self(ip))
    }
}

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        let mut connections = CONNECTIONS.lock().unwrap();
        if let Some(index) = connections.iter().position(|&ip| ip == self.0) {
            connections.swap_remove(index);
        }
    }
}

#[derive(Clone)]
struct SocketState {
    world: Arc<RwLock<ServerState>>,
    trusted_proxy: Option<IpAddr>,
}

pub const STARTUP_BANNER: &str = r#"
  ██╗  ██╗██╗   ██╗ █████╗ ██╗  ██╗██╗   ██╗    (百)
  ██║  ██║╚██╗ ██╔╝██╔══██╗██║ ██╔╝██║   ██║
  ███████║ ╚████╔╝ ███████║█████═╝ ██║   ██║    100-FLOOR CO-OP
  ██╔══██║  ╚██╔╝  ██╔══██║██╔═██╗ ██║   ██║    TOWER ASCENSION
  ██║  ██║   ██║   ██║  ██║██║ ╚██╗╚██████╔╝    
  ╚═╝  ╚═╝   ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝
"#;

#[tokio::main]
async fn main() {
    println!("{}", STARTUP_BANNER);
    println!("[HYAKU] Initializing authoritative world state...");

    let bind_addr = std::env::var("HYAKU_BIND_ADDR").unwrap_or_else(|_| "[::]:3000".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("failed to bind server port");
    let address = listener.local_addr().expect("read server address");

    let world_seed = 42424242;
    let saved = if let Some(value) = std::env::var_os("HYAKU_RAID_TEST_FLOOR") {
        assert!(address.ip().is_loopback(), "raid practice only listens on loopback");
        let floor = value.to_str()
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|floor| (10..=100).contains(floor) && floor % 10 == 0)
            .expect("raid practice requires a floor from 10 through 100 in steps of 10");
        ServerState::raid_practice(world_seed, floor)
    } else if std::env::var_os("HYAKU_DEMO").is_some_and(|value| value == "1") {
        assert!(address.ip().is_loopback(), "the demo lobby only listens on loopback");
        let mut state = ServerState::new(world_seed);
        state.enable_demo_lobby();
        state
    } else {
        let save_path = std::env::var_os("HYAKU_SAVE_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "hyaku_save.json".into());
        ServerState::load(world_seed, save_path.clone())
            .unwrap_or_else(|error| panic!("load {}: {error}", save_path.display()))
    };
    let server_state = Arc::new(RwLock::new(saved));
    tokio::spawn(tick::run_tick_loop(server_state.clone()));

    let trusted_proxy = std::env::var("HYAKU_TRUSTED_PROXY").ok().map(|ip| {
        ip.parse::<IpAddr>().expect("HYAKU_TRUSTED_PROXY must be an IP address").to_canonical()
    });
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(SocketState { world: server_state, trusted_proxy });

    println!("[HYAKU] Ready. Clients may connect to ws://{address}/ws");

    let listener = listener.tap_io(|stream| {
        let _ = stream.set_nodelay(true);
    });
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .expect("server failed");
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    State(state): State<SocketState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let peer = peer.ip().to_canonical();
    let (ip, resume_allowed) = if Some(peer) == state.trusted_proxy {
        let forwarded = headers.get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.rsplit(',').next())
            .and_then(|value| value.trim().parse::<IpAddr>().ok());
        let secure = headers.get("x-forwarded-proto").is_some_and(|value| value == "https");
        let Some(ip) = forwarded.filter(|_| secure) else {
            return (StatusCode::FORBIDDEN, "A secure proxy connection is required.").into_response();
        };
        (ip.to_canonical(), true)
    } else {
        (peer, peer.is_loopback())
    };

    let Some(slot) = ConnectionSlot::claim(ip) else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Too many connections. Try again shortly.")
            .into_response();
    };

    ws.max_message_size(MAX_PACKET_BYTES)
        .max_frame_size(MAX_PACKET_BYTES)
        .on_upgrade(move |socket| handle_socket(socket, state.world, slot, resume_allowed))
        .into_response()
}

async fn handle_socket(
    socket: WebSocket,
    state: Arc<RwLock<ServerState>>,
    _slot: ConnectionSlot,
    resume_allowed: bool,
) {
    let client_id = format!("client-{}", NEXT_CLIENT_ID.fetch_add(1, Ordering::Relaxed));
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel::<ServerPacket>(OUTBOX_SIZE);
    let overloaded = state.write().await.register_client(client_id.clone(), tx);

    println!("[HYAKU] Connected: {client_id}");

    let mut outbound_task = tokio::spawn(async move {
        while let Some(packet) = rx.recv().await {
            let Ok(json) = packet.to_json() else { continue };
            if !matches!(
                tokio::time::timeout(WRITE_TIMEOUT, ws_sender.send(Message::Text(json.into()))).await,
                Ok(Ok(()))
            ) {
                break;
            }
        }
    });

    let join_deadline = sleep(JOIN_TIMEOUT);
    tokio::pin!(join_deadline);
    let mut joined = false;
    let mut rate_window_start = Instant::now();
    let mut packet_count = 0;

    loop {
        let message = tokio::select! {
            biased;
            _ = overloaded.notified() => break,
            _ = &mut outbound_task => break,
            _ = &mut join_deadline, if !joined => break,
            message = ws_receiver.next() => message,
        };
        let now = Instant::now();
        if now.duration_since(rate_window_start).as_secs() >= 1 {
            rate_window_start = now;
            packet_count = 0;
        }
        packet_count += 1;
        if packet_count > MAX_PACKETS_PER_SEC {
            break;
        }

        match message {
            Some(Ok(Message::Text(text))) => {
                if let Ok(mut packet) = ClientPacket::from_json(&text) {
                    if !resume_allowed {
                        if let ClientPacket::Join { resume_key, .. } = &mut packet {
                            *resume_key = None;
                        }
                    }
                    let mut server = state.write().await;
                    server.handle_packet(&client_id, packet);
                    joined = server.client_floors.contains_key(&client_id);
                }
            }
            None | Some(Ok(Message::Close(_)) | Err(_)) => break,
            _ => {}
        }
    }

    outbound_task.abort();
    state.write().await.unregister_client(&client_id);
    println!("[HYAKU] Disconnected: {client_id}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_slots_are_limited_and_reusable() {
        let ip = "192.0.2.1".parse().unwrap();
        let mut slots: Vec<_> = (0..MAX_CONNECTIONS_PER_IP)
            .map(|_| ConnectionSlot::claim(ip).unwrap())
            .collect();
        assert!(ConnectionSlot::claim(ip).is_none());
        assert!(ConnectionSlot::claim("192.0.2.2".parse().unwrap()).is_some());
        slots.pop();
        assert!(ConnectionSlot::claim(ip).is_some());
    }
}
