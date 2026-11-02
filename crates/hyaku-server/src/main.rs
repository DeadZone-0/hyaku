pub mod combat;
mod state;
mod tick;

use crate::state::ServerState;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use hyaku_protocol::{ClientPacket, ServerPacket};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

static NEXT_CLIENT_ID: AtomicU64 = AtomicU64::new(1);
static ACTIVE_CONNECTIONS: AtomicU64 = AtomicU64::new(0);

const MAX_ACTIVE_CONNECTIONS: u64 = 500;
const MAX_PACKET_BYTES: usize = 64 * 1024; // 64 KB limit protects against OOM attacks
const MAX_PACKETS_PER_SEC: u32 = 50;       // Max 50 packets/sec per connection rate limit

// T
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

    let world_seed = 42424242;
    let server_state = Arc::new(RwLock::new(ServerState::new(world_seed)));

    // Spawn 20-TPS authoritative game loop
    tokio::spawn(tick::run_tick_loop(server_state.clone()));

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(server_state);

    let bind_addr = std::env::var("HYAKU_BIND_ADDR").unwrap_or_else(|_| "[::]:3000".to_string());
    println!("[HYAKU] Binding WebSocket listener on {}", bind_addr);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("failed to bind server port");

    println!("[HYAKU] Ready. Clients may connect to ws://{}/ws", bind_addr);
    axum::serve(listener, app).await.expect("server failed");
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<RwLock<ServerState>>>,
) -> impl IntoResponse {
    let current_connections = ACTIVE_CONNECTIONS.load(Ordering::Relaxed);
    if current_connections >= MAX_ACTIVE_CONNECTIONS {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Server at maximum climber capacity. Try again shortly.",
        )
            .into_response();
    }

    ws.max_message_size(MAX_PACKET_BYTES)
        .max_frame_size(MAX_PACKET_BYTES)
        .on_upgrade(move |socket| handle_socket(socket, state))
        .into_response()
}

async fn handle_socket(socket: WebSocket, state: Arc<RwLock<ServerState>>) {
    ACTIVE_CONNECTIONS.fetch_add(1, Ordering::Relaxed);
    let client_id = format!("client-{}", NEXT_CLIENT_ID.fetch_add(1, Ordering::Relaxed));
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerPacket>();

    {
        let mut server = state.write().await;
        server.register_client(client_id.clone(), tx);
    }

    println!("[HYAKU] Connected: {} (Active: {})", client_id, ACTIVE_CONNECTIONS.load(Ordering::Relaxed));

    // Outbound packet task (server -> client)
    let outbound_task = tokio::spawn(async move {
        while let Some(packet) = rx.recv().await {
            if let Ok(json) = packet.to_json() {
                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Inbound packet loop with per-connection rate limiting
    let mut rate_window_start = tokio::time::Instant::now();
    let mut packet_count = 0u32;

    while let Some(msg_result) = ws_receiver.next().await {
        let now = tokio::time::Instant::now();
        if now.duration_since(rate_window_start).as_secs() >= 1 {
            rate_window_start = now;
            packet_count = 0;
        }

        packet_count += 1;
        if packet_count > MAX_PACKETS_PER_SEC {
            // Drop excessive packets to prevent CPU starvation
            continue;
        }

        match msg_result {
            Ok(Message::Text(text)) => {
                if let Ok(packet) = ClientPacket::from_json(&text) {
                    let mut server = state.write().await;
                    server.handle_packet(&client_id, packet);
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // Cleanup on disconnect
    outbound_task.abort();
    {
        let mut server = state.write().await;
        server.unregister_client(&client_id);
    }
    ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);

    println!("[HYAKU] Disconnected: {} (Active: {})", client_id, ACTIVE_CONNECTIONS.load(Ordering::Relaxed));
}
