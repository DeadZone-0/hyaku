mod app;
mod network;
mod ui;

use app::{AppState, Screen};
use crossterm::{
    cursor,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use hyaku_protocol::ClientPacket;
use network::NetworkHandle;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Parse CLI arguments
    let mut args = std::env::args().skip(1);
    let mut username = "Adventurer".to_string();
    let mut server_url = "ws://127.0.0.1:3000/ws".to_string();
    let mut class_idx = 0;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--user" | "-u" => {
                if let Some(val) = args.next() {
                    username = val;
                }
            }
            "--server" | "-s" => {
                if let Some(val) = args.next() {
                    server_url = val;
                }
            }
            "--class" | "-c" => {
                if let Some(val) = args.next() {
                    if let Ok(idx) = val.parse::<usize>() {
                        class_idx = idx;
                    }
                }
            }
            other => {
                if other.starts_with("ws://") || other.starts_with("wss://") {
                    server_url = other.to_string();
                } else if let Ok(idx) = other.parse::<usize>() {
                    class_idx = idx;
                } else {
                    username = other.to_string();
                }
            }
        }
    }

    // 2. Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app_result = run_app(&mut terminal, username, server_url, class_idx).await;

    // 3. Terminal restoration
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        cursor::Show
    )?;
    terminal.show_cursor()?;

    if let Err(err) = app_result {
        eprintln!("[HYAKU CLIENT ERROR] {}", err);
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    username: String,
    server_url: String,
    class_idx: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = AppState::new(username, server_url, class_idx);
    let mut network_opt: Option<NetworkHandle> = None;
    let target_frame_duration = Duration::from_millis(16); // ~60 FPS

    while !app.should_quit {
        let frame_start = Instant::now();

        // 1. Handle network lifecycle
        match app.screen {
            Screen::Connecting => {
                if network_opt.is_none() {
                    let net = network::spawn_network(app.server_url.clone());
                    network_opt = Some(net);
                    // Queue join packet immediately
                    app.queue_packet(ClientPacket::Join {
                        username: app.username.clone(),
                        class_idx: app.class_idx,
                    });
                }
            }
            Screen::TitleScreen => {
                // Drop connection if returning to title
                network_opt = None;
            }
            _ => {}
        }

        // 2. Pump network messages
        if let Some(net) = &mut network_opt {
            // Receive inbound server packets
            while let Some(server_packet) = net.try_recv() {
                app.handle_server_packet(server_packet);
            }

            // Flush outbound client packets
            while let Some(client_packet) = app.pop_outbound_packet() {
                net.send(client_packet);
            }

            // Detect disconnects during gameplay
            if app.screen == Screen::Connected && !net.is_connected() {
                app.screen = Screen::Disconnected;
                network_opt = None;
            }
        }

        // 3. Update active visual effects & render frame
        app.tick_effects();
        terminal.draw(|f| ui::render(f, &mut app))?;

        // 4. Poll keyboard events (crossterm)
        let elapsed = frame_start.elapsed();
        let timeout = target_frame_duration.saturating_sub(elapsed);

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key.code);
                }
            }
        }

        // 5. Yield remaining frame time to CPU
        let frame_time = frame_start.elapsed();
        if frame_time < target_frame_duration {
            tokio::time::sleep(target_frame_duration - frame_time).await;
        }
    }

    Ok(())
}
