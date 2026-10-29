use crossterm::event::KeyCode;
use hyaku_protocol::{
    get_class_preset, ClientPacket, Direction, FloorMap, PlayerState, Position, ServerPacket,
    TickState, PLAYER_CLASSES,
};
use ratatui::style::Color;

// TODO(ZE): Customize HYAKU (百) multi-line ASCII title logo, class preview ASCII art, and title screen theme colors
pub const TITLE_BANNER: &str = r#"
  ██╗  ██╗██╗   ██╗ █████╗ ██╗  ██╗██╗   ██╗    (百)
  ██║  ██║╚██╗ ██╔╝██╔══██╗██║ ██╔╝██║   ██║
  ███████║ ╚████╔╝ ███████║█████═╝ ██║   ██║    100-FLOOR CO-OP
  ██╔══██║  ╚██╔╝  ██╔══██║██╔═██╗ ██║   ██║    TOWER ASCENSION
  ██║  ██║   ██║   ██║  ██║██║ ╚██╗╚██████╔╝    TERMINAL CLIENT
  ╚═╝  ╚═╝   ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝
"#;

pub const CLASS_PREVIEWS: &[&str] = &[
    // Blade
    r#"
       /| ________________
 O|===|* >________________>
       \|
    "#,
    // Arcanist
    r#"
       / \
      ( * )   < ether focus >
       \ /
        |
        |
    "#,
    // Ranger
    r#"
      |\
      | \
      |  >======>>  [long range]
      | /
      |/
    "#,
    // Architect
    r#"
      [====]
        ||
      ======  [chasm builder]
      | ## |
    "#,
];

pub const TITLE_COLOR_PRIMARY: Color = Color::Cyan;
pub const TITLE_COLOR_ACCENT: Color = Color::Yellow;
pub const TITLE_COLOR_MUTED: Color = Color::DarkGray;

// TODO(ZE): Customize death screen ASCII banner and theme colors
pub const DEATH_BANNER: &str = r#"
  ╦ ╦╔═╗╦ ╦  ╔╦╗╦╔═╗╔╦╗
  ╚╦╝║ ║║ ║   ║║║║╣  ║║
   ╩ ╚═╝╚═╝  ═╩╝╩╚═╝═╩╝
  YOU HAVE FALLEN IN THE TOWER
"#;
pub const DEATH_COLOR_PRIMARY: Color = Color::Red;
pub const DEATH_COLOR_ACCENT: Color = Color::DarkGray;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    TitleScreen,
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Chatting,
    CarvingMessage,
}

#[derive(Debug, Clone)]
pub struct FloatingText {
    pub pos: Position,
    pub text: String,
    pub color: Color,
    pub ttl: u8,
}

pub struct AppState {
    pub screen: Screen,
    pub input_mode: InputMode,
    pub username: String,
    pub server_url: String,
    pub class_idx: usize,
    pub my_player_id: Option<String>,
    pub current_floor: u32,
    pub world_seed: u64,
    pub current_map: Option<FloorMap>,
    pub tick_state: Option<TickState>,
    pub chat_input: String,
    pub carving_input: String,
    pub chat_log: Vec<(String, String)>,
    pub announcement: Option<String>,
    pub combat_log: Vec<String>,
    pub floating_texts: Vec<FloatingText>,
    pub screen_flash: Option<(Color, u8)>,
    pub camera_offset: (i32, i32),
    pub outbound_queue: Vec<ClientPacket>,
    pub should_quit: bool,
    pub title_edit_name: bool,
    pub last_dir: Direction,
}

impl AppState {
    pub fn new(username: String, server_url: String, class_idx: usize) -> Self {
        let valid_class = class_idx.min(PLAYER_CLASSES.len().saturating_sub(1));
        Self {
            screen: Screen::TitleScreen,
            input_mode: InputMode::Normal,
            username,
            server_url,
            class_idx: valid_class,
            my_player_id: None,
            current_floor: 1,
            world_seed: 0,
            current_map: None,
            tick_state: None,
            chat_input: String::new(),
            carving_input: String::new(),
            chat_log: Vec::new(),
            announcement: None,
            combat_log: Vec::new(),
            floating_texts: Vec::new(),
            screen_flash: None,
            camera_offset: (0, 0),
            outbound_queue: Vec::new(),
            should_quit: false,
            title_edit_name: false,
            last_dir: Direction::Down,
        }
    }

    pub fn pop_outbound_packet(&mut self) -> Option<ClientPacket> {
        if self.outbound_queue.is_empty() {
            None
        } else {
            Some(self.outbound_queue.remove(0))
        }
    }

    pub fn queue_packet(&mut self, packet: ClientPacket) {
        self.outbound_queue.push(packet);
    }

    pub fn tick_effects(&mut self) {
        if let Some((color, ttl)) = self.screen_flash {
            if ttl > 1 {
                self.screen_flash = Some((color, ttl - 1));
            } else {
                self.screen_flash = None;
            }
        }

        self.floating_texts.retain_mut(|ft| {
            if ft.ttl > 1 {
                ft.ttl -= 1;
                true
            } else {
                false
            }
        });
    }

    pub fn spawn_floating_text(&mut self, pos: Position, text: String, color: Color) {
        self.floating_texts.push(FloatingText {
            pos,
            text,
            color,
            ttl: 20,
        });
    }

    pub fn my_player(&self) -> Option<&PlayerState> {
        let id = self.my_player_id.as_ref()?;
        let tick = self.tick_state.as_ref()?;
        tick.players.iter().find(|p| &p.id == id)
    }

    pub fn update_camera(&mut self, viewport_w: i32, viewport_h: i32) {
        if let Some(player) = self.my_player() {
            let px = player.pos.x;
            let py = player.pos.y;

            let (map_w, map_h) = self
                .current_map
                .as_ref()
                .map(|m| (m.width, m.height))
                .unwrap_or((80, 30));

            let raw_x = px - viewport_w / 2;
            let raw_y = py - viewport_h / 2;

            let clamped_x = raw_x.clamp(0, (map_w - viewport_w).max(0));
            let clamped_y = raw_y.clamp(0, (map_h - viewport_h).max(0));

            self.camera_offset = (clamped_x, clamped_y);
        }
    }

    pub fn world_to_screen(&self, pos: Position) -> (i32, i32) {
        (pos.x - self.camera_offset.0, pos.y - self.camera_offset.1)
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        match self.screen {
            Screen::TitleScreen => self.handle_title_key(key),
            Screen::Connecting => {
                if key == KeyCode::Esc {
                    self.screen = Screen::TitleScreen;
                }
            }
            Screen::Connected => self.handle_connected_key(key),
            Screen::Disconnected => {
                if key == KeyCode::Esc || key == KeyCode::Enter {
                    self.screen = Screen::TitleScreen;
                }
            }
        }
    }

    fn handle_title_key(&mut self, key: KeyCode) {
        if self.title_edit_name {
            match key {
                KeyCode::Enter => {
                    self.title_edit_name = false;
                }
                KeyCode::Esc => {
                    self.title_edit_name = false;
                }
                KeyCode::Backspace => {
                    self.username.pop();
                }
                KeyCode::Char(c) => {
                    if self.username.len() < 16 && !c.is_control() {
                        self.username.push(c);
                    }
                }
                _ => {}
            }
            return;
        }

        match key {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Tab => {
                self.title_edit_name = true;
            }
            KeyCode::Left | KeyCode::Char('a') => {
                if self.class_idx == 0 {
                    self.class_idx = PLAYER_CLASSES.len().saturating_sub(1);
                } else {
                    self.class_idx -= 1;
                }
            }
            KeyCode::Right | KeyCode::Char('d') => {
                self.class_idx = (self.class_idx + 1) % PLAYER_CLASSES.len();
            }
            KeyCode::Enter => {
                if self.username.trim().is_empty() {
                    self.username = "Adventurer".to_string();
                }
                self.screen = Screen::Connecting;
            }
            _ => {}
        }
    }

    fn handle_connected_key(&mut self, key: KeyCode) {
        if self.input_mode == InputMode::CarvingMessage {
            match key {
                KeyCode::Enter => {
                    let text = self.carving_input.trim().to_string();
                    if !text.is_empty() {
                        self.queue_packet(ClientPacket::CarveFloor { message: text });
                        self.carving_input.clear();
                    }
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Esc => {
                    self.carving_input.clear();
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Backspace => {
                    self.carving_input.pop();
                }
                KeyCode::Char(c) => {
                    if self.carving_input.len() < 60 {
                        self.carving_input.push(c);
                    }
                }
                _ => {}
            }
            return;
        }

        if self.input_mode == InputMode::Chatting {
            match key {
                KeyCode::Enter => {
                    let text = self.chat_input.trim().to_string();
                    if !text.is_empty() {
                        self.queue_packet(ClientPacket::Chat { text });
                        self.chat_input.clear();
                    }
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Esc => {
                    self.chat_input.clear();
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Backspace => {
                    self.chat_input.pop();
                }
                KeyCode::Char(c) => {
                    if self.chat_input.len() < 80 {
                        self.chat_input.push(c);
                    }
                }
                _ => {}
            }
            return;
        }

        // Dead player control: only Respawn or Quit
        if self.my_player().map_or(false, |p| !p.is_alive()) {
            match key {
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.queue_packet(ClientPacket::Respawn);
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.should_quit = true;
                }
                _ => {}
            }
            return;
        }

        // InputMode::Normal
        match key {
            KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                let skill_name = match self.class_idx {
                    0 => "Iai: Eightfold Bloom",
                    1 => "Aether Rupture: Singularity",
                    2 => "Phantom Rain: Eclipse Volley",
                    _ => "Titan's Cataclysm: Faultline",
                };
                self.announcement = Some(format!("Unleashing Power: {}!", skill_name));
                self.screen_flash = Some((Color::Cyan, 4));
                if let Some(me) = self.my_player() {
                    self.spawn_floating_text(
                        Position::new(me.pos.x, me.pos.y.saturating_sub(1)),
                        skill_name.to_string(),
                        Color::LightMagenta,
                    );
                }
                self.queue_packet(ClientPacket::ClassSkill(self.last_dir));
            }
            KeyCode::Enter => {
                self.input_mode = InputMode::Chatting;
            }
            KeyCode::Char('f') => {
                self.input_mode = InputMode::CarvingMessage;
            }
            KeyCode::Char('u') => {
                if let (Some(me), Some(tick)) = (self.my_player(), &self.tick_state) {
                    if let Some(carving) = tick.carvings.iter().find(|c| c.pos == me.pos || me.pos.manhattan_distance(c.pos) <= 1) {
                        self.queue_packet(ClientPacket::UpvoteCarving { pos: carving.pos });
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') => {
                self.last_dir = Direction::Up;
                self.queue_packet(ClientPacket::Move(Direction::Up));
            }
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') => {
                self.last_dir = Direction::Down;
                self.queue_packet(ClientPacket::Move(Direction::Down));
            }
            KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('h') => {
                self.last_dir = Direction::Left;
                self.queue_packet(ClientPacket::Move(Direction::Left));
            }
            KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('l') => {
                self.last_dir = Direction::Right;
                self.queue_packet(ClientPacket::Move(Direction::Right));
            }
            KeyCode::Char(' ') => {
                let action_desc = match self.class_idx {
                    0 => "Crescent Slash",
                    1 => "Astral Lance",
                    2 => "Gale Piercer",
                    _ => "Adamantine Strike",
                };
                self.announcement = Some(format!("Striking with {} towards {}", action_desc, self.last_dir));
                self.queue_packet(ClientPacket::Attack(self.last_dir));
            }
            KeyCode::Char('e') => {
                let is_gate_locked = self.current_map.as_ref().map_or(false, |map| {
                    if map.gates.is_empty() {
                        return false;
                    }
                    map.gates.iter().any(|&(_id, gpos)| {
                        if let Some(tick) = &self.tick_state {
                            if let Some(&(_, hyaku_protocol::Tile::Gate { open, .. })) =
                                tick.tiles_override.iter().find(|(p, _)| *p == gpos)
                            {
                                return !open;
                            }
                        }
                        true
                    })
                });

                if is_gate_locked {
                    if let Some(me) = self.my_player() {
                        self.spawn_floating_text(
                            me.pos,
                            "* GATE SEALED! *".to_string(),
                            ratatui::style::Color::Red,
                        );
                    }
                    self.announcement = Some(
                        "The gate to the stairs is sealed! Step on the Pressure Plate or slay the Floor Guardian to unseal the ascent!".to_string(),
                    );
                } else {
                    self.queue_packet(ClientPacket::AscendStairs);
                }
            }
            KeyCode::Char('b') => {
                self.queue_packet(ClientPacket::BuildBridge(self.last_dir));
            }
            KeyCode::Char('c') => {
                self.queue_packet(ClientPacket::LightCampfire);
            }
            KeyCode::Char('x') => {
                self.queue_packet(ClientPacket::DeployDecoy);
            }
            KeyCode::Char('z') => {
                if let Some(player) = self.my_player() {
                    if player.shards > 0 {
                        self.queue_packet(ClientPacket::DepositShards { amount: 5 });
                    }
                }
            }
            _ => {}
        }
    }

    pub fn handle_server_packet(&mut self, packet: ServerPacket) {
        match packet {
            ServerPacket::Welcome {
                player_id,
                floor,
                map_seed,
            } => {
                self.my_player_id = Some(player_id);
                self.current_floor = floor;
                self.world_seed = map_seed;
                self.current_map = Some(FloorMap::generate(map_seed, floor));
                self.screen = Screen::Connected;
                self.announcement = Some(format!("Entered Floor {} of the Tower", floor));
            }
            ServerPacket::Tick(state) => {
                if self.current_floor != state.floor {
                    self.current_floor = state.floor;
                    self.current_map = Some(FloorMap::generate(self.world_seed, state.floor));
                }
                self.tick_state = Some(state);
            }
            ServerPacket::FloorTransition {
                floor,
                map_seed,
                message,
            } => {
                self.current_floor = floor;
                self.world_seed = map_seed;
                self.current_map = Some(FloorMap::generate(map_seed, floor));
                self.announcement = Some(message);
            }
            ServerPacket::ChatMessage {
                sender,
                text,
                ..
            } => {
                self.chat_log.push((sender, text));
                if self.chat_log.len() > 50 {
                    self.chat_log.remove(0);
                }
            }
            ServerPacket::Announcement { text } => {
                self.announcement = Some(text.clone());
                self.combat_log.push(text.clone());
                if self.combat_log.len() > 10 {
                    self.combat_log.remove(0);
                }

                if text.contains("ABYSS EXECUTION") {
                    self.screen_flash = Some((Color::Magenta, 5));
                    if let Some(me) = self.my_player() {
                        let front = me.pos.step(self.last_dir);
                        self.spawn_floating_text(front, "*VOID FALL!*".to_string(), Color::LightMagenta);
                    }
                } else if text.contains("WALL SLAM") {
                    self.screen_flash = Some((Color::Yellow, 4));
                    if let Some(me) = self.my_player() {
                        let front = me.pos.step(self.last_dir);
                        self.spawn_floating_text(front, "*SLAM +8*".to_string(), Color::Yellow);
                    }
                } else if text.contains("CRIT") {
                    self.screen_flash = Some((Color::Rgb(255, 230, 40), 3));
                    if let Some(me) = self.my_player() {
                        let front = me.pos.step(self.last_dir);
                        self.spawn_floating_text(front, "*CRIT!*".to_string(), Color::Rgb(255, 230, 40));
                    }
                } else if text.contains("Enemy struck") {
                    self.screen_flash = Some((Color::Red, 4));
                    if let Some(me) = self.my_player() {
                        self.spawn_floating_text(me.pos, "-HURT-".to_string(), Color::Red);
                    }
                }
            }
            ServerPacket::Error { message } => {
                self.announcement = Some(format!("Error: {}", message));
            }
            ServerPacket::Pong { .. } => {}
        }
    }

    pub fn current_class_preset(&self) -> &'static hyaku_protocol::PlayerClassPreset {
        get_class_preset(self.class_idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_title_navigation() {
        let mut app = AppState::new(
            "ZeHero".to_string(),
            "ws://127.0.0.1:3000/ws".to_string(),
            0,
        );
        assert_eq!(app.screen, Screen::TitleScreen);
        assert_eq!(app.class_idx, 0);

        // Move to next class
        app.handle_key(KeyCode::Right);
        assert_eq!(app.class_idx, 1);

        // Cycle through all
        app.handle_key(KeyCode::Left);
        assert_eq!(app.class_idx, 0);

        // Edit name
        app.handle_key(KeyCode::Tab);
        assert!(app.title_edit_name);
        app.handle_key(KeyCode::Backspace);
        assert_eq!(app.username, "ZeHer");
        app.handle_key(KeyCode::Char('o'));
        assert_eq!(app.username, "ZeHero");
        app.handle_key(KeyCode::Enter);
        assert!(!app.title_edit_name);

        // Connect
        app.handle_key(KeyCode::Enter);
        assert_eq!(app.screen, Screen::Connecting);
    }

    #[test]
    fn test_camera_math_and_screen_projection() {
        let mut app = AppState::new(
            "ZeHero".to_string(),
            "ws://127.0.0.1:3000/ws".to_string(),
            0,
        );
        app.current_map = Some(FloorMap::new_empty(1, 80, 30));
        app.my_player_id = Some("p-1".to_string());
        app.tick_state = Some(TickState {
            tick: 1,
            floor: 1,
            players: vec![PlayerState::new(
                "p-1".to_string(),
                "ZeHero".to_string(),
                0,
                Position::new(40, 15),
                1,
            )],
            enemies: Vec::new(),
            projectiles: Vec::new(),
            tiles_override: Vec::new(),
            carvings: Vec::new(),
            tower_shards_bank: 0,
        });

        // Center on (40, 15) with viewport 40x20
        app.update_camera(40, 20);
        assert_eq!(app.camera_offset, (20, 5));

        let screen_pos = app.world_to_screen(Position::new(40, 15));
        assert_eq!(screen_pos, (20, 10)); // Exactly in the middle of 40x20
    }
}
