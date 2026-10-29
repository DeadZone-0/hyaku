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

    let bind_addr = "0.0.0.0:3000";
    println!("[HYAKU] Binding WebSocket listener on {}", bind_addr);

    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .expect("failed to bind server port");

    println!("[HYAKU] Ready. Clients may connect to ws://{}/ws", bind_addr);
    axum::serve(listener, app).await.expect("server failed");
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<RwLock<ServerState>>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<RwLock<ServerState>>) {
    let client_id = format!("client-{}", NEXT_CLIENT_ID.fetch_add(1, Ordering::Relaxed));
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerPacket>();

    {
        let mut server = state.write().await;
        server.register_client(client_id.clone(), tx);
    }

    println!("[HYAKU] Connected: {}", client_id);

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

    // Inbound packet loop (client -> server)
    while let Some(msg_result) = ws_receiver.next().await {
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

    println!("[HYAKU] Disconnected: {}", client_id);
}
