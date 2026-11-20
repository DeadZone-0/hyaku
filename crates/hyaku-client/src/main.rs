mod app;
mod network;
mod local_server;
mod resume;
mod sound;
mod ui;

use app::{AppState, Screen};
use crossterm::{
    cursor,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use hyaku_protocol::{ClientPacket, ResumeKey};
use network::NetworkHandle;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Parse CLI arguments
    let mut args = std::env::args().skip(1);
    let mut username = "Adventurer".to_string();
    let default_server = std::env::var("HYAKU_SERVER")
        .unwrap_or_else(|_| "ws://[2a01:4f9:3080:3192::2124]:3000/ws".to_string());
    let mut server_url = default_server;
    let mut class_idx = 0;
    let mut muted = false;
    let mut launch_local = false;
    let mut auto_connect = false;
    let mut raid_floor = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!(
                    "HYAKU (百) - 100-Floor Co-op Tower Ascension MMO TUI\n\
                     \n\
                     USAGE:\n\
                       hyaku [OPTIONS] [USERNAME] [SERVER_URL]\n\
                     \n\
                     OPTIONS:\n\
                       -u, --user <NAME>       Player username\n\
                       -s, --server <URL>      WebSocket server URL (default: Nest cloud server)\n\
                       -c, --class <0-3>       Class selection (0: Blade, 1: Arcanist, 2: Ranger, 3: Architect)\n\
                       -l, --local             Connect to local development server (ws://127.0.0.1:3000/ws)\n\
                           --local-server      Launch and join an isolated local server\n\
                           --raid-floor <N>    Start a local solo raid on floor 10, 20, ... 100\n\
                           --connect           Join immediately with the selected name and class\n\
                           --mute              Turn off sound and ambience\n\
                       -v, --version           Print version information\n\
                       -h, --help              Print this help message\n\
                     \n\
                     CONTROLS:\n\
                       [WASD] / [Arrows]       Move climber\n\
                       [Space]                 Slash or shoot as Ranger\n\
                       [R]                     Deploy Holo-Decoy (20 Mana, holds pressure plates)\n\
                       [B] + [WASD]            Build Shard Bridge over Chasm (5 Shards)\n\
                       [E]                     Interact with shrines, stairs, or allies\n\
                       [Enter]                 Open Tower Comms\n\
                       [Esc]                   Cancel / Menu\n\
                       [Q]                     Link Lightning Tether\n\
                       [C]                     Kindle campfire (15 Shards)\n\
                       [M] / [L]               Carve / praise a message\n\
                       [Tab]                   Hall of Fame & Lore\n"
                );
                return Ok(());
            }
            "--version" | "-v" => {
                println!("hyaku v{}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--mute" => muted = true,
            "--local-server" => launch_local = true,
            "--raid-floor" => {
                raid_floor = Some(args.next()
                    .and_then(|floor| floor.parse::<u32>().ok())
                    .filter(|floor| (10..=100).contains(floor) && floor % 10 == 0)
                    .ok_or_else(|| io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--raid-floor needs a floor from 10 through 100 in steps of 10",
                    ))?);
                launch_local = true;
            }
            "--connect" => auto_connect = true,
            "--local" | "-l" => {
                server_url = "ws://127.0.0.1:3000/ws".to_string();
            }
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
                } else if other == "local" || other == "localhost" {
                    server_url = "ws://127.0.0.1:3000/ws".to_string();
                } else if let Ok(idx) = other.parse::<usize>() {
                    class_idx = idx;
                } else {
                    username = other.to_string();
                }
            }
        }
    }

    let local = if launch_local {
        let server = local_server::LocalServer::start(raid_floor).await?;
        server_url = server.url.clone();
        auto_connect = true;
        Some(server)
    } else {
        None
    };

    let sound = if muted { None } else { sound::Sound::open() };

    let resume_key = if launch_local {
        None
    } else {
        match resume::for_server(&server_url) {
            Ok(key) => Some(key),
            Err(error) => {
                eprintln!("Could not load the resume key: {error}");
                None
            }
        }
    };

    // 2. Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app_result = run_app(&mut terminal, username, server_url, class_idx, sound, resume_key, auto_connect).await;

    // 3. Terminal restoration
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        cursor::Show
    )?;
    terminal.show_cursor()?;
    if let Some(server) = local {
        server.stop().await;
    }

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
    mut sound: Option<sound::Sound>,
    resume_key: Option<ResumeKey>,
    auto_connect: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = AppState::new(username, server_url, class_idx);
    if auto_connect {
        app.handle_key(event::KeyCode::Enter);
    }
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
                        resume_key: resume_key.clone(),
                    });
                }
            }
            Screen::TitleScreen => {
                // Drop connection if returning to title
                network_opt = None;
            }
            _ => {}
        }

        // 2. Pump inbound server packets
        if let Some(net) = &mut network_opt {
            while let Some(server_packet) = net.try_recv() {
                app.handle_server_packet(server_packet);
            }

            if app.screen == Screen::Connected && !net.is_connected() {
                app.screen = Screen::Disconnected;
                network_opt = None;
            }
        }

        // 3. Drain all pending keyboard events (handles Press & Repeat for smooth held-key walking)
        while event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                    app.handle_key(key.code);
                }
            }
        }

        // 4. Zero-frame outbound packet flush: send immediately upon keypress!
        if let Some(net) = &mut network_opt {
            while let Some(client_packet) = app.pop_outbound_packet() {
                net.send(client_packet);
            }
        }

        // 5. Update active visual effects, camera & render frame
        app.tick_effects();
        let cue = app.sound_cue.take();
        if let Some(sound) = &mut sound {
            sound.update(app.ambience_level(), app.raid_music_active(), cue, &app.milestone_cues);
        }
        app.milestone_cues.clear();
        terminal.draw(|f| ui::render(f, &mut app))?;

        // 6. Yield remaining frame time while listening for immediate input
        let elapsed = frame_start.elapsed();
        if elapsed < target_frame_duration {
            let remaining = target_frame_duration - elapsed;
            if event::poll(remaining)? {
                let mut quick_input = false;
                while event::poll(Duration::from_millis(0))? {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press || key.kind == KeyEventKind::Repeat {
                            app.handle_key(key.code);
                            quick_input = true;
                        }
                    }
                }
                if quick_input {
                    if let Some(net) = &mut network_opt {
                        while let Some(client_packet) = app.pop_outbound_packet() {
                            net.send(client_packet);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
