use crate::app::{
    AppState, InputMode, Screen, CLASS_PREVIEWS, DEATH_BANNER, DEATH_COLOR_ACCENT,
    DEATH_COLOR_PRIMARY, TITLE_BANNER, TITLE_COLOR_ACCENT, TITLE_COLOR_MUTED, TITLE_COLOR_PRIMARY,
};
use hyaku_protocol::{Position, Tile, PLAYER_CLASSES};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, Paragraph, Wrap},
    Frame,
};

pub fn render(frame: &mut Frame, app: &mut AppState) {
    match app.screen {
        Screen::TitleScreen => render_title_screen(frame, app),
        Screen::Connecting => render_connecting_screen(frame, app),
        Screen::Connected => render_game_screen(frame, app),
        Screen::Disconnected => render_disconnected_screen(frame, app),
    }
}

fn render_title_screen(frame: &mut Frame, app: &AppState) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // Logo Banner
            Constraint::Length(3),  // Username
            Constraint::Min(12),    // Class Selector & Preview
            Constraint::Length(3),  // Keybindings footer
        ])
        .margin(1)
        .split(area);

    // 1. Title Banner
    let banner = Paragraph::new(TITLE_BANNER)
        .alignment(Alignment::Center)
        .style(
            Style::default()
                .fg(TITLE_COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(banner, chunks[0]);

    // 2. Username input
    let name_style = if app.title_edit_name {
        Style::default()
            .fg(Color::Black)
            .bg(TITLE_COLOR_ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let cursor = if app.title_edit_name { "_" } else { "" };
    let username_line = Line::from(vec![
        Span::raw("  Adventurer Name: [ "),
        Span::styled(format!("{}{}", app.username, cursor), name_style),
        Span::raw(" ]  "),
        Span::styled(
            if app.title_edit_name {
                "(Press Enter to confirm)"
            } else {
                "(Press TAB to edit)"
            },
            Style::default().fg(TITLE_COLOR_MUTED),
        ),
    ]);
    let username_p = Paragraph::new(username_line)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::NONE));
    frame.render_widget(username_p, chunks[1]);

    // 3. Class selector & preview
    let class_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[2]);

    let preset = app.current_class_preset();
    let class_info = vec![
        Line::from(vec![
            Span::raw("  <  "),
            Span::styled(
                format!("[ {} ]", preset.name),
                Style::default()
                    .fg(TITLE_COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  >  "),
            Span::styled(
                format!("({} of {})", app.class_idx + 1, PLAYER_CLASSES.len()),
                Style::default().fg(TITLE_COLOR_MUTED),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}", preset.description),
            Style::default().fg(Color::Gray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Base HP:       "),
            Span::styled(
                format!("{}", preset.max_hp),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Base Mana:     "),
            Span::styled(
                format!("{}", preset.max_mana),
                Style::default().fg(Color::Blue),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Attack Power:  "),
            Span::styled(
                format!("{}", preset.attack_power),
                Style::default().fg(Color::Red),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Map Glyph:     "),
            Span::styled(
                format!("'{}'", preset.glyph),
                Style::default().fg(Color::Rgb(
                    preset.color_rgb.0,
                    preset.color_rgb.1,
                    preset.color_rgb.2,
                )),
            ),
        ]),
    ];

    let class_block = Block::default()
        .title(" Select Class ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(TITLE_COLOR_PRIMARY));
    let class_p = Paragraph::new(class_info).block(class_block);
    frame.render_widget(class_p, class_chunks[0]);

    // Preview ASCII box
    let preview_art = if app.class_idx < CLASS_PREVIEWS.len() {
        CLASS_PREVIEWS[app.class_idx]
    } else {
        ""
    };
    let preview_p = Paragraph::new(preview_art)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Rgb(
            preset.color_rgb.0,
            preset.color_rgb.1,
            preset.color_rgb.2,
        )))
        .block(
            Block::default()
                .title(" Crest & Weapon ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(TITLE_COLOR_MUTED)),
        );
    frame.render_widget(preview_p, class_chunks[1]);

    // 4. Footer controls
    let footer_text = Line::from(vec![
        Span::styled("[Enter]", Style::default().fg(TITLE_COLOR_ACCENT)),
        Span::raw(" Ascend Tower   "),
        Span::styled("[A/D or Arrows]", Style::default().fg(TITLE_COLOR_ACCENT)),
        Span::raw(" Switch Class   "),
        Span::styled("[Tab]", Style::default().fg(TITLE_COLOR_ACCENT)),
        Span::raw(" Edit Name   "),
        Span::styled("[Esc/Q]", Style::default().fg(TITLE_COLOR_ACCENT)),
        Span::raw(" Quit"),
    ]);
    let footer_p = Paragraph::new(footer_text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::TOP));
    frame.render_widget(footer_p, chunks[3]);
}

fn render_connecting_screen(frame: &mut Frame, app: &AppState) {
    let area = frame.area();
    let popup = centered_rect(60, 20, area);
    frame.render_widget(Clear, popup);

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Connecting to the Tower...",
            Style::default()
                .fg(TITLE_COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Target: {}", app.server_url),
            Style::default().fg(Color::Gray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "[Esc] Cancel and return to title",
            Style::default().fg(TITLE_COLOR_MUTED),
        )),
    ];

    let p = Paragraph::new(text).alignment(Alignment::Center).block(
        Block::default()
            .title(" Connection ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(TITLE_COLOR_PRIMARY)),
    );
    frame.render_widget(p, popup);
}

fn render_disconnected_screen(frame: &mut Frame, app: &AppState) {
    let area = frame.area();
    let popup = centered_rect(60, 25, area);
    frame.render_widget(Clear, popup);

    let reason = app
        .announcement
        .as_deref()
        .unwrap_or("Connection closed by server.");

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Disconnected from the Tower",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(reason, Style::default().fg(Color::Gray))),
        Line::from(""),
        Line::from(Span::styled(
            "Press [Enter] or [Esc] to return to title screen",
            Style::default().fg(TITLE_COLOR_ACCENT),
        )),
    ];

    let p = Paragraph::new(text).alignment(Alignment::Center).block(
        Block::default()
            .title(" Status ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red)),
    );
    frame.render_widget(p, popup);
}

fn render_game_screen(frame: &mut Frame, app: &mut AppState) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Top header
            Constraint::Min(10),   // Main gameplay (map + side panel)
            Constraint::Length(5), // Bottom chat & input
        ])
        .split(area);

    render_header(frame, app, chunks[0]);

    let middle_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(20), Constraint::Length(30)])
        .split(chunks[1]);

    render_viewport(frame, app, middle_chunks[0]);
    render_sidebar(frame, app, middle_chunks[1]);
    render_chat_bar(frame, app, chunks[2]);

    // If player is slain, overlay dramatic Death Screen
    if app.my_player().map_or(false, |p| !p.is_alive()) {
        render_death_screen(frame, area);
    }
}

#[derive(Clone, Copy)]
pub struct BiomeTheme {
    pub name: &'static str,
    pub desc: &'static str,
    pub accent_color: Color,
    pub wall_color: Color,
    pub floor_primary: Color,
    pub floor_secondary: Color,
    pub floor_tertiary: Color,
    pub chasm_wind_color: Color,
    pub chasm_depth_bg: Color,
}

pub fn get_biome_theme(floor: u32) -> BiomeTheme {
    match floor {
        1..=10 => BiomeTheme {
            name: "Subterranean Vaults",
            desc: "Ancient mossy stone halls lit by dying embers",
            accent_color: Color::Rgb(120, 190, 255),
            wall_color: Color::Rgb(115, 130, 155),
            floor_primary: Color::Rgb(85, 95, 115),
            floor_secondary: Color::Rgb(70, 105, 90),
            floor_tertiary: Color::Rgb(105, 115, 130),
            chasm_wind_color: Color::Rgb(65, 90, 120),
            chasm_depth_bg: Color::Rgb(8, 10, 16),
        },
        11..=20 => BiomeTheme {
            name: "Sunless Catacombs",
            desc: "Echoing burial ruins haunted by restless phantoms",
            accent_color: Color::Rgb(175, 135, 255),
            wall_color: Color::Rgb(135, 120, 160),
            floor_primary: Color::Rgb(95, 85, 120),
            floor_secondary: Color::Rgb(120, 95, 140),
            floor_tertiary: Color::Rgb(80, 75, 105),
            chasm_wind_color: Color::Rgb(120, 75, 175),
            chasm_depth_bg: Color::Rgb(12, 8, 18),
        },
        21..=30 => BiomeTheme {
            name: "Abyssal Precipice",
            desc: "Bottomless chasms swept by freezing void winds",
            accent_color: Color::Rgb(255, 130, 90),
            wall_color: Color::Rgb(105, 115, 145),
            floor_primary: Color::Rgb(85, 95, 125),
            floor_secondary: Color::Rgb(75, 85, 115),
            floor_tertiary: Color::Rgb(105, 115, 140),
            chasm_wind_color: Color::Rgb(85, 125, 210),
            chasm_depth_bg: Color::Rgb(6, 8, 16),
        },
        31..=50 => BiomeTheme {
            name: "Clockwork Bastion",
            desc: "Sealed iron barriers and grinding mechanical gears",
            accent_color: Color::Rgb(255, 205, 80),
            wall_color: Color::Rgb(165, 140, 95),
            floor_primary: Color::Rgb(120, 105, 80),
            floor_secondary: Color::Rgb(150, 120, 70),
            floor_tertiary: Color::Rgb(105, 90, 65),
            chasm_wind_color: Color::Rgb(180, 135, 55),
            chasm_depth_bg: Color::Rgb(14, 10, 6),
        },
        51..=80 => BiomeTheme {
            name: "Astral Sanctum",
            desc: "Crystalline ether halls humming with celestial power",
            accent_color: Color::Rgb(90, 245, 230),
            wall_color: Color::Rgb(95, 180, 230),
            floor_primary: Color::Rgb(75, 135, 165),
            floor_secondary: Color::Rgb(115, 215, 235),
            floor_tertiary: Color::Rgb(95, 155, 185),
            chasm_wind_color: Color::Rgb(100, 220, 255),
            chasm_depth_bg: Color::Rgb(6, 12, 22),
        },
        81..=99 => BiomeTheme {
            name: "Infernal Spire",
            desc: "Blazing trial chambers forged in ancient dragonfire",
            accent_color: Color::Rgb(255, 100, 70),
            wall_color: Color::Rgb(185, 90, 70),
            floor_primary: Color::Rgb(130, 80, 70),
            floor_secondary: Color::Rgb(170, 75, 55),
            floor_tertiary: Color::Rgb(110, 65, 60),
            chasm_wind_color: Color::Rgb(235, 105, 40),
            chasm_depth_bg: Color::Rgb(18, 6, 6),
        },
        _ => BiomeTheme {
            name: "The Celestial Apex",
            desc: "The legendary golden zenith of the hundred floors",
            accent_color: Color::Rgb(255, 230, 100),
            wall_color: Color::Rgb(240, 215, 125),
            floor_primary: Color::Rgb(175, 155, 95),
            floor_secondary: Color::Rgb(225, 205, 135),
            floor_tertiary: Color::Rgb(150, 135, 80),
            chasm_wind_color: Color::Rgb(255, 230, 120),
            chasm_depth_bg: Color::Rgb(16, 14, 8),
        },
    }
}

pub fn get_floor_biome(floor: u32) -> (&'static str, &'static str, Color) {
    let theme = get_biome_theme(floor);
    (theme.name, theme.desc, theme.accent_color)
}

fn render_header(frame: &mut Frame, app: &AppState, area: Rect) {
    let player_count = app
        .tick_state
        .as_ref()
        .map(|t| t.players.len())
        .unwrap_or(0);

    let shards_bank = app
        .tick_state
        .as_ref()
        .map(|t| t.tower_shards_bank)
        .unwrap_or(0);

    let my_shards = app.my_player().map(|p| p.shards).unwrap_or(0);
    let (biome_title, _, biome_col) = get_floor_biome(app.current_floor);

    let (stairs_compass, stairs_dist) = if let (Some(me), Some(map)) = (app.my_player(), &app.current_map) {
        let dx = map.stairs_pos.x - me.pos.x;
        let dy = map.stairs_pos.y - me.pos.y;
        let dist = me.pos.manhattan_distance(map.stairs_pos);
        let arrow = match (dx.signum(), dy.signum()) {
            (0, -1) => "↑",
            (0, 1) => "↓",
            (-1, 0) => "←",
            (1, 0) => "→",
            (1, -1) => "↗",
            (1, 1) => "↘",
            (-1, -1) => "↖",
            (-1, 1) => "↙",
            _ => "* HERE",
        };
        (arrow, dist)
    } else {
        ("?", 0)
    };

    let is_gate_locked = app.current_map.as_ref().map_or(false, |map| {
        if map.gates.is_empty() {
            return false;
        }
        map.gates.iter().any(|&(_id, gpos)| {
            if let Some(tick) = &app.tick_state {
                if let Some(&(_, Tile::Gate { open, .. })) =
                    tick.tiles_override.iter().find(|(p, _)| *p == gpos)
                {
                    return !open;
                }
            }
            true
        })
    });

    let header_line = Line::from(vec![
        Span::styled(
            " HYAKU (百) ",
            Style::default()
                .fg(TITLE_COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | Floor: "),
        Span::styled(
            format!("{}/100 ", app.current_floor),
            Style::default()
                .fg(TITLE_COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("[ {} ]", biome_title),
            Style::default().fg(biome_col).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | Gate: "),
        if is_gate_locked {
            Span::styled(
                "[ LOCKED ] ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                "[ UNLOCKED ] ",
                Style::default()
                    .fg(Color::Rgb(80, 255, 150))
                    .add_modifier(Modifier::BOLD),
            )
        },
        Span::raw("| Obj: "),
        if is_gate_locked {
            Span::styled(
                "[ Step Plate or Slay Guardian ] ",
                Style::default().fg(Color::Yellow),
            )
        } else {
            Span::styled(
                "[ ASCEND VIA STAIRS [E] ] ",
                Style::default()
                    .fg(Color::Rgb(90, 255, 180))
                    .add_modifier(Modifier::BOLD),
            )
        },
        Span::raw("| Stairs: "),
        Span::styled(
            format!("[ {} {}m ]", stairs_compass, stairs_dist),
            Style::default()
                .fg(Color::Rgb(80, 255, 160))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | Shards: "),
        Span::styled(
            format!("{}", my_shards),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(" | Bank: "),
        Span::styled(
            format!("{}", shards_bank),
            Style::default().fg(Color::LightYellow),
        ),
        Span::raw(" | Co-op: "),
        Span::styled(
            format!("{}", player_count),
            Style::default().fg(Color::Green),
        ),
    ]);

    let header_p = Paragraph::new(header_line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(TITLE_COLOR_PRIMARY)),
    );
    frame.render_widget(header_p, area);
}

fn render_viewport(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let inner_w = area.width.saturating_sub(2) as i32;
    let inner_h = area.height.saturating_sub(2) as i32;

    app.update_camera(inner_w, inner_h);

    let theme = get_biome_theme(app.current_floor);
    let border_color = if let Some((flash_color, _)) = app.screen_flash {
        flash_color
    } else {
        theme.accent_color
    };

    let title_badge = if let Some((flash_color, _)) = app.screen_flash {
        if flash_color == Color::Red {
            " [!! DAMAGE TAKEN !!] "
        } else if flash_color == Color::Yellow {
            " [** CRITICAL HIT **] "
        } else {
            " [<< POWER UNLEASHED >>] "
        }
    } else {
        ""
    };

    let map_block = Block::default()
        .title(format!(
            " Floor {}: {} {}(Floor Map) ",
            app.current_floor, theme.name, title_badge
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color).add_modifier(
            if app.screen_flash.is_some() {
                Modifier::BOLD
            } else {
                Modifier::empty()
            },
        ));

    let inner_area = map_block.inner(area);
    frame.render_widget(map_block, area);

    if inner_w <= 0 || inner_h <= 0 {
        return;
    }

    // Build 2D grid buffer for visible screen
    let mut grid: Vec<Vec<(char, Style)>> =
        vec![vec![(' ', Style::default()); inner_w as usize]; inner_h as usize];

    let tick_counter = app.tick_state.as_ref().map(|t| t.tick).unwrap_or(0);

    // Landmark positions for ambient atmosphere
    let campfire_positions: Vec<Position> = app
        .tick_state
        .as_ref()
        .map(|tick| {
            tick.tiles_override
                .iter()
                .filter(|(_, t)| matches!(t, Tile::Campfire))
                .map(|&(p, _)| p)
                .collect()
        })
        .unwrap_or_default();

    let shrine_pos = app.current_map.as_ref().and_then(|m| m.shrine_pos);
    let stairs_pos = app.current_map.as_ref().map(|m| m.stairs_pos);

    // 1. Render base map tiles with rich biome palettes, procedural texturing & living animation
    if let Some(map) = &app.current_map {
        for sy in 0..inner_h {
            for sx in 0..inner_w {
                let wx = sx + app.camera_offset.0;
                let wy = sy + app.camera_offset.1;
                let pos = Position::new(wx, wy);

                if let Some(tile) = map.get_tile(pos) {
                    let near_campfire = campfire_positions
                        .iter()
                        .any(|&cp| pos.chebyshev_distance(cp) <= 2);
                    let near_shrine = shrine_pos.map_or(false, |sp| pos.chebyshev_distance(sp) <= 2);
                    let near_stairs = stairs_pos.map_or(false, |stp| pos.chebyshev_distance(stp) <= 1);

                    let (ch, style) = tile_to_glyph_and_style(
                        tile,
                        tick_counter,
                        pos,
                        &theme,
                        near_campfire,
                        near_shrine,
                        near_stairs,
                    );
                    grid[sy as usize][sx as usize] = (ch, style);
                }
            }
        }
    }

    // 2. Render dynamic tile overrides (bridges, active plates, open gates, campfires)
    if let Some(tick) = &app.tick_state {
        for &(pos, tile) in &tick.tiles_override {
            let (sx, sy) = app.world_to_screen(pos);
            if sx >= 0 && sx < inner_w && sy >= 0 && sy < inner_h {
                let near_campfire = campfire_positions
                    .iter()
                    .any(|&cp| pos.chebyshev_distance(cp) <= 2);
                let near_shrine = shrine_pos.map_or(false, |sp| pos.chebyshev_distance(sp) <= 2);
                let near_stairs = stairs_pos.map_or(false, |stp| pos.chebyshev_distance(stp) <= 1);

                let (ch, style) = tile_to_glyph_and_style(
                    tile,
                    tick_counter,
                    pos,
                    &theme,
                    near_campfire,
                    near_shrine,
                    near_stairs,
                );
                grid[sy as usize][sx as usize] = (ch, style);
            }
        }

        // 3. Render carvings
        for carving in &tick.carvings {
            let (sx, sy) = app.world_to_screen(carving.pos);
            if sx >= 0 && sx < inner_w && sy >= 0 && sy < inner_h {
                grid[sy as usize][sx as usize] = (
                    '?',
                    Style::default()
                        .fg(Color::LightYellow)
                        .add_modifier(Modifier::BOLD),
                );
            }
        }

        // 4. Render projectiles (attack slashes, arrows, ether bolts)
        for proj in &tick.projectiles {
            let (sx, sy) = app.world_to_screen(proj.pos);
            if sx >= 0 && sx < inner_w && sy >= 0 && sy < inner_h {
                let bg_rgb = (
                    (proj.color_rgb.0 / 4).max(15),
                    (proj.color_rgb.1 / 4).max(15),
                    (proj.color_rgb.2 / 4).max(15),
                );
                let style = Style::default()
                    .fg(Color::Rgb(
                        proj.color_rgb.0,
                        proj.color_rgb.1,
                        proj.color_rgb.2,
                    ))
                    .bg(Color::Rgb(bg_rgb.0, bg_rgb.1, bg_rgb.2))
                    .add_modifier(Modifier::BOLD);
                grid[sy as usize][sx as usize] = (proj.glyph, style);
            }
        }

        // 5. Render enemies with health status colors
        for enemy in &tick.enemies {
            if !enemy.is_alive() {
                continue;
            }
            let (sx, sy) = app.world_to_screen(enemy.pos);
            if sx >= 0 && sx < inner_w && sy >= 0 && sy < inner_h {
                let hp_ratio = enemy.hp as f32 / enemy.max_hp.max(1) as f32;
                let rgb = enemy.kind.color_rgb();

                let style = if hp_ratio < 0.35 {
                    Style::default()
                        .fg(Color::Red)
                        .bg(Color::Rgb(80, 0, 0))
                        .add_modifier(Modifier::BOLD)
                } else if hp_ratio < 0.70 {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Rgb(rgb.0, rgb.1, rgb.2))
                        .add_modifier(if enemy.is_boss {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        })
                };
                grid[sy as usize][sx as usize] = (enemy.glyph, style);
            }
        }

        // 6. Render other players and local player
        for player in &tick.players {
            let (sx, sy) = app.world_to_screen(player.pos);
            if sx >= 0 && sx < inner_w && sy >= 0 && sy < inner_h {
                if !player.is_alive() {
                    grid[sy as usize][sx as usize] = (
                        '†',
                        Style::default()
                            .fg(Color::Rgb(220, 80, 80))
                            .bg(Color::Rgb(50, 15, 15))
                            .add_modifier(Modifier::BOLD),
                    );
                    continue;
                }
                let is_me = app.my_player_id.as_deref() == Some(&player.id);
                let color = Color::Rgb(player.color_rgb.0, player.color_rgb.1, player.color_rgb.2);

                let style = if is_me {
                    Style::default()
                        .fg(color)
                        .bg(Color::Rgb(30, 50, 75))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(color).add_modifier(Modifier::BOLD)
                };

                grid[sy as usize][sx as usize] = (player.glyph, style);
            }
        }

        // 7. Render Player Aiming / Facing Reticle
        if let Some(me) = app.my_player() {
            let target_pos = me.pos.step(app.last_dir);
            let (tx, ty) = app.world_to_screen(target_pos);
            if tx >= 0 && tx < inner_w && ty >= 0 && ty < inner_h {
                let (cur_ch, cur_style) = grid[ty as usize][tx as usize];
                let has_enemy = tick.enemies.iter().any(|e| e.is_alive() && e.pos == target_pos);

                if has_enemy {
                    grid[ty as usize][tx as usize] = (
                        cur_ch,
                        cur_style
                            .bg(Color::Red)
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    );
                } else if cur_ch != '#' {
                    let arrow = match app.last_dir {
                        hyaku_protocol::Direction::Up => '▲',
                        hyaku_protocol::Direction::Down => '▼',
                        hyaku_protocol::Direction::Left => '◄',
                        hyaku_protocol::Direction::Right => '►',
                    };
                    grid[ty as usize][tx as usize] = (
                        arrow,
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(255, 215, 0))
                            .add_modifier(Modifier::BOLD),
                    );
                }
            }
        }

        // 8. Render Floating Combat Damage and Effect Texts
        for ft in &app.floating_texts {
            let (fx, fy) = app.world_to_screen(ft.pos);
            if fy >= 0 && fy < inner_h {
                for (i, ch) in ft.text.chars().enumerate() {
                    let sx = fx + i as i32;
                    if sx >= 0 && sx < inner_w {
                        grid[fy as usize][sx as usize] = (
                            ch,
                            Style::default()
                                .fg(ft.color)
                                .bg(Color::Rgb(20, 20, 30))
                                .add_modifier(Modifier::BOLD),
                        );
                    }
                }
            }
        }
    }

    // Convert grid buffer into lines for Ratatui Paragraph
    let lines: Vec<Line> = grid
        .into_iter()
        .map(|row| {
            Line::from(
                row.into_iter()
                    .map(|(ch, style)| Span::styled(ch.to_string(), style))
                    .collect::<Vec<Span>>(),
            )
        })
        .collect();

    let map_p = Paragraph::new(lines);
    frame.render_widget(map_p, inner_area);
}

fn tile_to_glyph_and_style(
    tile: Tile,
    tick: u64,
    pos: Position,
    theme: &BiomeTheme,
    near_campfire: bool,
    near_shrine: bool,
    near_stairs: bool,
) -> (char, Style) {
    match tile {
        Tile::Wall => {
            ('#', Style::default().fg(theme.wall_color).bg(Color::Rgb(22, 26, 35)))
        }
        Tile::Floor => {
            if near_campfire {
                let ember_step = (tick / 2) as i32;
                let spark = ((pos.x * 11).wrapping_add(pos.y * 19).wrapping_add(ember_step)).rem_euclid(9);
                if spark == 0 {
                    (
                        '⠂',
                        Style::default()
                            .fg(Color::Rgb(255, 185, 50))
                            .bg(Color::Rgb(48, 22, 10))
                            .add_modifier(Modifier::BOLD),
                    )
                } else if spark == 1 {
                    (
                        '⠄',
                        Style::default()
                            .fg(Color::Rgb(255, 140, 30))
                            .bg(Color::Rgb(48, 22, 10)),
                    )
                } else {
                    (
                        '·',
                        Style::default()
                            .fg(Color::Rgb(220, 140, 70))
                            .bg(Color::Rgb(42, 20, 10)),
                    )
                }
            } else if near_shrine {
                let ether_step = (tick / 2) as i32;
                let mote = ((pos.x * 13).wrapping_add(pos.y * 7).wrapping_add(ether_step)).rem_euclid(8);
                if mote == 0 {
                    (
                        '⠁',
                        Style::default()
                            .fg(Color::Rgb(120, 245, 255))
                            .bg(Color::Rgb(15, 38, 52))
                            .add_modifier(Modifier::BOLD),
                    )
                } else if mote == 1 {
                    (
                        '⠐',
                        Style::default()
                            .fg(Color::Rgb(90, 220, 245))
                            .bg(Color::Rgb(15, 38, 52)),
                    )
                } else {
                    (
                        '·',
                        Style::default()
                            .fg(Color::Rgb(100, 210, 235))
                            .bg(Color::Rgb(12, 32, 45)),
                    )
                }
            } else if near_stairs {
                let aura_step = (tick / 3) as i32;
                let beam = ((pos.x * 5).wrapping_add(pos.y * 7).wrapping_add(aura_step)).rem_euclid(7);
                if beam == 0 {
                    (
                        '▲',
                        Style::default()
                            .fg(Color::Rgb(130, 255, 190))
                            .bg(Color::Rgb(18, 48, 32)),
                    )
                } else {
                    (
                        '·',
                        Style::default()
                            .fg(Color::Rgb(100, 230, 160))
                            .bg(Color::Rgb(15, 42, 28)),
                    )
                }
            } else {
                let hash = ((pos.x.wrapping_mul(37) ^ pos.y.wrapping_mul(17)).wrapping_abs()) as usize;
                let (ch, col) = match hash % 10 {
                    0 => (',', theme.floor_secondary),
                    1 => ('.', theme.floor_tertiary),
                    _ => ('·', theme.floor_primary),
                };
                (ch, Style::default().fg(col).bg(Color::Rgb(16, 20, 26)))
            }
        }
        Tile::Chasm => {
            let wind_step = (tick / 2) as i32;
            let wave = ((pos.x * 7).wrapping_add(pos.y * 13).wrapping_add(wind_step)).rem_euclid(18);
            if wave == 0 {
                ('⠄', Style::default().fg(theme.chasm_wind_color).bg(theme.chasm_depth_bg))
            } else if wave == 1 {
                ('⠂', Style::default().fg(theme.chasm_wind_color).bg(theme.chasm_depth_bg))
            } else if wave == 9 {
                ('⠐', Style::default().fg(theme.chasm_wind_color).bg(theme.chasm_depth_bg))
            } else {
                (' ', Style::default().bg(theme.chasm_depth_bg))
            }
        }
        Tile::PressurePlate { active: true, .. } => (
            '_',
            Style::default()
                .fg(Color::Rgb(80, 255, 220))
                .bg(Color::Rgb(15, 50, 45))
                .add_modifier(Modifier::BOLD),
        ),
        Tile::PressurePlate { active: false, .. } => {
            let plate_fg = if (tick / 4) % 2 == 0 {
                Color::Rgb(255, 220, 20)
            } else {
                Color::Rgb(220, 180, 50)
            };
            (
                'o',
                Style::default()
                    .fg(plate_fg)
                    .bg(Color::Rgb(45, 38, 12))
                    .add_modifier(Modifier::BOLD),
            )
        }
        Tile::Gate { open: true, .. } => (
            '/',
            Style::default()
                .fg(Color::Rgb(80, 255, 150))
                .bg(Color::Rgb(15, 55, 30))
                .add_modifier(Modifier::BOLD),
        ),
        Tile::Gate { open: false, .. } => {
            let pulse_col = if (tick / 3) % 2 == 0 {
                Color::Rgb(255, 75, 75)
            } else {
                Color::Rgb(220, 50, 50)
            };
            (
                '+',
                Style::default()
                    .fg(pulse_col)
                    .bg(Color::Rgb(65, 15, 15))
                    .add_modifier(Modifier::BOLD),
            )
        }
        Tile::Shrine => {
            const BRAILLE_PULSAR: &[char] = &['⠁', '⠃', '⠇', '⠧', '⠷', '⠿', '⠾', '⠼', '⠸', '⠈'];
            let glyph = if (tick / 4) % 2 == 0 {
                'S'
            } else {
                BRAILLE_PULSAR[(tick as usize / 2) % BRAILLE_PULSAR.len()]
            };
            let glow = if (tick / 2) % 2 == 0 {
                Color::Rgb(90, 245, 255)
            } else {
                Color::Rgb(180, 255, 255)
            };
            (
                glyph,
                Style::default()
                    .fg(glow)
                    .bg(Color::Rgb(18, 55, 75))
                    .add_modifier(Modifier::BOLD),
            )
        }
        Tile::StairsUp => {
            let glow = if (tick / 3) % 2 == 0 {
                Color::Rgb(90, 255, 170)
            } else {
                Color::Rgb(180, 255, 230)
            };
            (
                '▲',
                Style::default()
                    .fg(glow)
                    .bg(Color::Rgb(20, 70, 42))
                    .add_modifier(Modifier::BOLD),
            )
        }
        Tile::Bridge => (
            '=',
            Style::default()
                .fg(Color::Rgb(220, 160, 85))
                .bg(Color::Rgb(55, 35, 20))
                .add_modifier(Modifier::BOLD),
        ),
        Tile::Campfire => {
            const BRAILLE_FLAME: &[char] = &['⠂', '⠢', '⠶', '⣶', '⠿', '⠦', '⠔', '⠒'];
            let glyph = BRAILLE_FLAME[(tick as usize / 2) % BRAILLE_FLAME.len()];
            let fg_col = match (tick / 2) % 3 {
                0 => Color::Rgb(255, 110, 20),
                1 => Color::Rgb(255, 180, 40),
                _ => Color::Rgb(255, 230, 70),
            };
            (
                glyph,
                Style::default()
                    .fg(fg_col)
                    .bg(Color::Rgb(65, 22, 5))
                    .add_modifier(Modifier::BOLD),
            )
        }
    }
}

fn render_sidebar(frame: &mut Frame, app: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Health, Mana, Facing, Stairs Radar & Gate status
            Constraint::Length(7),  // Nearby Foes with HP bars
            Constraint::Min(8),     // Controls guide
        ])
        .split(area);

    let my_player = app.my_player();

    // 1. Vitals block
    let (hp, max_hp) = my_player.map(|p| (p.hp, p.max_hp)).unwrap_or((0, 100));
    let (mana, max_mana) = my_player
        .map(|p| (p.mana, p.max_mana))
        .unwrap_or((0, 100));

    let hp_ratio = (hp as f64 / max_hp.max(1) as f64).clamp(0.0, 1.0);
    let mana_ratio = (mana as f64 / max_mana.max(1) as f64).clamp(0.0, 1.0);

    let vitals_block = Block::default()
        .title(" Adventurer Vitals ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(TITLE_COLOR_PRIMARY));
    let vitals_inner = vitals_block.inner(chunks[0]);
    frame.render_widget(vitals_block, chunks[0]);

    let gauge_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // HP
            Constraint::Length(2), // Mana
            Constraint::Length(1), // Facing direction
            Constraint::Length(1), // Stairs radar
            Constraint::Length(1), // Gate & Ascent status
        ])
        .split(vitals_inner);

    let hp_gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Red).bg(Color::DarkGray))
        .ratio(hp_ratio)
        .label(format!("HP: {}/{}", hp, max_hp));
    frame.render_widget(hp_gauge, gauge_chunks[0]);

    let mana_gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Blue).bg(Color::DarkGray))
        .ratio(mana_ratio)
        .label(format!("MP: {}/{}", mana, max_mana));
    frame.render_widget(mana_gauge, gauge_chunks[1]);

    let reach = match app.class_idx {
        1 => "Reach 3 (Spell)",
        2 => "Reach 4 (Ranged)",
        _ => "Reach 1 (Melee)",
    };
    let facing_line = Line::from(vec![
        Span::raw("Facing: "),
        Span::styled(
            format!("[ {} {} ]", app.last_dir.arrow_glyph(), app.last_dir.as_str()),
            Style::default()
                .fg(TITLE_COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  {}", reach), Style::default().fg(Color::Gray)),
    ]);
    frame.render_widget(Paragraph::new(facing_line), gauge_chunks[2]);

    let (stairs_compass, stairs_dist) = if let (Some(me), Some(map)) = (app.my_player(), &app.current_map) {
        let dx = map.stairs_pos.x - me.pos.x;
        let dy = map.stairs_pos.y - me.pos.y;
        let dist = me.pos.manhattan_distance(map.stairs_pos);
        let arrow = match (dx.signum(), dy.signum()) {
            (0, -1) => "↑",
            (0, 1) => "↓",
            (-1, 0) => "←",
            (1, 0) => "→",
            (1, -1) => "↗",
            (1, 1) => "↘",
            (-1, -1) => "↖",
            (-1, 1) => "↙",
            _ => "* HERE",
        };
        (arrow, dist)
    } else {
        ("?", 0)
    };

    let stairs_line = Line::from(vec![
        Span::raw("Stairs: "),
        Span::styled(
            format!("[ {} {}m Exit ]", stairs_compass, stairs_dist),
            Style::default()
                .fg(Color::Rgb(80, 255, 160))
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(stairs_line), gauge_chunks[3]);

    let is_gate_locked = app.current_map.as_ref().map_or(false, |map| {
        if map.gates.is_empty() {
            return false;
        }
        map.gates.iter().any(|&(_id, gpos)| {
            if let Some(tick) = &app.tick_state {
                if let Some(&(_, Tile::Gate { open, .. })) =
                    tick.tiles_override.iter().find(|(p, _)| *p == gpos)
                {
                    return !open;
                }
            }
            true
        })
    });

    let ascent_line = Line::from(vec![
        Span::raw("Gate: "),
        if is_gate_locked {
            Span::styled(
                "[ SEALED - Slay Guardian / Plate ]",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                "[ UNSEALED - Press E ]",
                Style::default()
                    .fg(Color::Rgb(80, 255, 160))
                    .add_modifier(Modifier::BOLD),
            )
        },
    ]);
    frame.render_widget(Paragraph::new(ascent_line), gauge_chunks[4]);

    // 2. Nearby Foes block with live Health Bars
    let mut foe_lines = Vec::new();
    if let (Some(me), Some(tick)) = (my_player, &app.tick_state) {
        let mut nearby: Vec<(u32, &hyaku_protocol::EnemyState)> = tick
            .enemies
            .iter()
            .filter(|e| e.is_alive())
            .map(|e| (me.pos.manhattan_distance(e.pos), e))
            .filter(|&(d, _)| d <= 12)
            .collect();
        nearby.sort_by_key(|&(d, _)| d);

        if nearby.is_empty() {
            foe_lines.push(Line::from(Span::styled(
                "  No foes in sight",
                Style::default().fg(TITLE_COLOR_MUTED),
            )));
        } else {
            for (dist, enemy) in nearby.iter().take(3) {
                let (bar_str, bar_col) = format_mini_bar(enemy.hp, enemy.max_hp, 8);
                let label = if enemy.is_boss { "BOSS" } else { enemy.kind.name() };
                foe_lines.push(Line::from(vec![
                    Span::styled(
                        format!("{} ", enemy.glyph),
                        Style::default()
                            .fg(Color::Rgb(
                                enemy.kind.color_rgb().0,
                                enemy.kind.color_rgb().1,
                                enemy.kind.color_rgb().2,
                            ))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!("{:<6} ", label.chars().take(6).collect::<String>())),
                    Span::styled(bar_str, Style::default().fg(bar_col)),
                    Span::styled(
                        format!(" {}/{}", enemy.hp, enemy.max_hp),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(
                        format!(" ({}m)", dist),
                        Style::default().fg(TITLE_COLOR_MUTED),
                    ),
                ]));
            }
        }
    } else {
        foe_lines.push(Line::from(Span::styled(
            "  No foes in sight",
            Style::default().fg(TITLE_COLOR_MUTED),
        )));
    }

    let foes_block = Block::default()
        .title(" Nearby Foes (HP) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    frame.render_widget(Paragraph::new(foe_lines).block(foes_block), chunks[1]);

    // 3. Controls block
    let (biome_title, biome_desc, biome_col) = get_floor_biome(app.current_floor);

    let (skill_name, attack_name) = match app.class_idx {
        0 => ("Iai: Eightfold Bloom (15 MP)", "Crescent Slash"),
        1 => ("Aether Rupture: Singularity (20 MP)", "Astral Lance"),
        2 => ("Phantom Rain: Eclipse Volley (15 MP)", "Gale Piercer"),
        _ => ("Titan's Cataclysm: Faultline (15 MP)", "Adamantine Strike"),
    };

    let controls = vec![
        Line::from(vec![
            Span::styled(format!("Realm: {}", biome_title), Style::default().fg(biome_col).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(format!("\"{}\"", biome_desc), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("[WASD/Arrows]", Style::default().fg(TITLE_COLOR_ACCENT)),
            Span::raw(" Move & Face"),
        ]),
        Line::from(vec![
            Span::styled("[Space]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" Strike ({})", attack_name), Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("[Q]", Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" Power: {}", skill_name), Style::default().fg(Color::LightMagenta).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("[B]", Style::default().fg(TITLE_COLOR_ACCENT)),
            Span::raw(" Bridge (3 Shards)"),
        ]),
        Line::from(vec![
            Span::styled("[C]", Style::default().fg(TITLE_COLOR_ACCENT)),
            Span::raw(" Campfire (15 MP/2 Shards)"),
        ]),
        Line::from(vec![
            Span::styled("[X]", Style::default().fg(TITLE_COLOR_ACCENT)),
            Span::raw(" Holo-Decoy (25 MP)"),
        ]),
        Line::from(vec![
            Span::styled("[Z]", Style::default().fg(TITLE_COLOR_ACCENT)),
            Span::raw(" Bank 5 Shards"),
        ]),
        Line::from(vec![
            Span::styled("[E]", Style::default().fg(TITLE_COLOR_ACCENT)),
            Span::raw(" Ascend Stairs"),
        ]),
        Line::from(vec![
            Span::styled("[F / U]", Style::default().fg(TITLE_COLOR_ACCENT)),
            Span::raw(" Inscribe / Upvote"),
        ]),
        Line::from(vec![
            Span::styled("[Enter]", Style::default().fg(TITLE_COLOR_ACCENT)),
            Span::raw(" Floor Chat"),
        ]),
        Line::from(vec![
            Span::styled("[Esc]", Style::default().fg(TITLE_COLOR_ACCENT)),
            Span::raw(" Quit Game"),
        ]),
    ];

    let controls_p = Paragraph::new(controls).block(
        Block::default()
            .title(" Actions ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(TITLE_COLOR_MUTED)),
    );
    frame.render_widget(controls_p, chunks[2]);
}

fn format_mini_bar(cur: i32, max: i32, width: usize) -> (String, Color) {
    let ratio = (cur as f32 / max.max(1) as f32).clamp(0.0, 1.0);
    let filled = (ratio * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    let color = if ratio < 0.35 {
        Color::Red
    } else if ratio < 0.70 {
        Color::Yellow
    } else {
        Color::Green
    };
    (format!("[{}{}]", "█".repeat(filled), "-".repeat(empty)), color)
}

fn render_chat_bar(frame: &mut Frame, app: &AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(2), Constraint::Length(3)])
        .split(area);

    // Event Announcement + Chat history
    let mut history_lines = Vec::new();

    if !app.combat_log.is_empty() {
        for event in app.combat_log.iter().rev().take(2).rev() {
            let style = if event.contains("ABYSS") {
                Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)
            } else if event.contains("WALL") {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else if event.contains("CRIT") || event.contains("SINGULARITY") || event.contains("AETHER") {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if event.contains("BLOOM") || event.contains("EIGHTFOLD") {
                Style::default().fg(Color::Rgb(255, 90, 70)).add_modifier(Modifier::BOLD)
            } else if event.contains("ECLIPSE") || event.contains("VOLLEY") {
                Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)
            } else if event.contains("CATACLYSM") || event.contains("FAULTLINE") {
                Style::default().fg(Color::Rgb(255, 190, 60)).add_modifier(Modifier::BOLD)
            } else if event.contains("SLAIN") {
                Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD)
            } else if event.contains("Enemy struck") {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            history_lines.push(Line::from(vec![
                Span::styled(
                    ">> EVENT: ",
                    Style::default()
                        .fg(TITLE_COLOR_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(event, style),
            ]));
        }
    } else if let Some(ann) = &app.announcement {
        history_lines.push(Line::from(vec![
            Span::styled(
                ">> EVENT: ",
                Style::default()
                    .fg(TITLE_COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                ann,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    let chat_lines: Vec<Line> = app
        .chat_log
        .iter()
        .rev()
        .take(2)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|(sender, msg)| {
            Line::from(vec![
                Span::styled(
                    format!("{}: ", sender),
                    Style::default().fg(TITLE_COLOR_ACCENT),
                ),
                Span::raw(msg),
            ])
        })
        .collect();
    history_lines.extend(chat_lines);

    let chat_block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(TITLE_COLOR_MUTED));
    let history_p = Paragraph::new(history_lines)
        .block(chat_block)
        .wrap(Wrap { trim: true });
    frame.render_widget(history_p, chunks[0]);

    // Active input line
    let (input_text, input_style) = match app.input_mode {
        InputMode::Chatting => (
            Line::from(vec![
                Span::styled(
                    "Say: ",
                    Style::default()
                        .fg(TITLE_COLOR_ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&app.chat_input),
                Span::styled("_", Style::default().fg(TITLE_COLOR_ACCENT)),
            ]),
            Style::default().fg(Color::White),
        ),
        InputMode::CarvingMessage => (
            Line::from(vec![
                Span::styled(
                    ":: Inscribe Floor Message: ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&app.carving_input),
                Span::styled("_", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "  [Enter: Inscribe  |  Esc: Cancel]",
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Style::default().fg(Color::Yellow),
        ),
        InputMode::Normal => {
            // Check if standing on or near a carving
            let standing_carving = if let (Some(me), Some(tick)) = (app.my_player(), &app.tick_state) {
                tick.carvings
                    .iter()
                    .find(|c| c.pos == me.pos || me.pos.manhattan_distance(c.pos) <= 1)
            } else {
                None
            };

            if let Some(carving) = standing_carving {
                (
                    Line::from(vec![
                        Span::styled(":: Ancient Carving by ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            &carving.author,
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(": \""),
                        Span::styled(
                            &carving.message,
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("\" [+{}]  ", carving.upvotes),
                            Style::default().fg(Color::Yellow),
                        ),
                        Span::styled(
                            "[Press U to Upvote!]",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Style::default().fg(Color::Yellow),
                )
            } else {
                (
                    Line::from(Span::styled(
                        "Press [Enter] to Chat  |  [F] to Inscribe Floor Message  |  [Space] to Attack",
                        Style::default().fg(TITLE_COLOR_MUTED),
                    )),
                    Style::default(),
                )
            }
        }
    };

    let input_p = Paragraph::new(input_text).style(input_style).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(match app.input_mode {
                InputMode::Chatting => Style::default().fg(TITLE_COLOR_ACCENT),
                InputMode::CarvingMessage => Style::default().fg(Color::Yellow),
                InputMode::Normal => Style::default().fg(TITLE_COLOR_MUTED),
            }),
    );
    frame.render_widget(input_p, chunks[1]);
}

fn render_death_screen(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(65, 45, area);
    frame.render_widget(Clear, popup);

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            DEATH_BANNER,
            Style::default()
                .fg(DEATH_COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [R] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(
                "Respawn at Campfire / Spawn Point",
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [Esc / Q] ", Style::default().fg(DEATH_COLOR_ACCENT)),
            Span::styled("Quit to Terminal", Style::default().fg(DEATH_COLOR_ACCENT)),
        ]),
    ];

    let p = Paragraph::new(text).alignment(Alignment::Center).block(
        Block::default()
            .title(" Slain in the Tower ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DEATH_COLOR_PRIMARY)),
    );
    frame.render_widget(p, popup);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
