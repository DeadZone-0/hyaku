use futures_util::{SinkExt, StreamExt};
use hyaku_protocol::{ClientPacket, ServerPacket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

pub struct NetworkHandle {
    outbound_tx: mpsc::UnboundedSender<ClientPacket>,
    inbound_rx: mpsc::UnboundedReceiver<ServerPacket>,
    connected: Arc<AtomicBool>,
}

impl NetworkHandle {
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn send(&self, packet: ClientPacket) -> bool {
        self.outbound_tx.send(packet).is_ok()
    }

    pub fn try_recv(&mut self) -> Option<ServerPacket> {
        self.inbound_rx.try_recv().ok()
    }
}

pub fn spawn_network(url: String) -> NetworkHandle {
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<ClientPacket>();
    let (inbound_tx, inbound_rx) = mpsc::unbounded_channel::<ServerPacket>();
    let connected = Arc::new(AtomicBool::new(false));
    let connected_clone = connected.clone();

    tokio::spawn(async move {
        match connect_async(&url).await {
            Ok((ws_stream, _)) => {
                connected_clone.store(true, Ordering::Relaxed);
                let (mut ws_write, mut ws_read) = ws_stream.split();

                loop {
                    tokio::select! {
                        // Outbound packet from client application
                        Some(client_pkt) = outbound_rx.recv() => {
                            if let Ok(json) = client_pkt.to_json() {
                                if ws_write.send(Message::Text(json.into())).await.is_err() {
                                    break;
                                }
                            }
                        }

                        // Inbound message from server
                        msg_opt = ws_read.next() => {
                            match msg_opt {
                                Some(Ok(Message::Text(text))) => {
                                    if let Ok(server_pkt) = ServerPacket::from_json(&text) {
                                        if inbound_tx.send(server_pkt).is_err() {
                                            break;
                                        }
                                    }
                                }
                                Some(Ok(Message::Close(_))) | None => break,
                                Some(Err(_)) => break,
                                _ => {}
                            }
                        }
                    }
                }

                connected_clone.store(false, Ordering::Relaxed);
            }
            Err(e) => {
                let _ = inbound_tx.send(ServerPacket::Error {
                    message: format!("Failed to connect to {}: {}", url, e),
                });
                connected_clone.store(false, Ordering::Relaxed);
            }
        }
    });

    NetworkHandle {
        outbound_tx,
        inbound_rx,
        connected,
    }
}
