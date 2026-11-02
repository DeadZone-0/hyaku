use crate::app::{
    AppState, InputMode, Screen, ServerBannerData, CLASS_PREVIEWS, DEATH_BANNER, DEATH_COLOR_ACCENT,
    DEATH_COLOR_PRIMARY, TITLE_BANNER, TITLE_COLOR_ACCENT, TITLE_COLOR_MUTED, TITLE_COLOR_PRIMARY,
};
use hyaku_protocol::{Direction, Position, Tile, PLAYER_CLASSES};
use ratatui::{
    layout::{Alignment, Constraint, Direction as LayoutDirection, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, Paragraph, Wrap},
    Frame,
};

// TODO(ZE): Customize the 10 Stratum biome color palettes (Walls, Floors, Borders, Accents) and Shrine ASCII pedestal art
#[allow(dead_code)]
pub const SHRINE_ASCII_PEDESTAL: &[&str] = &[
    "   ╔═════╗   ",
    " ╔═╣ [百] ╠═╗ ",
    " ║ ╚═════╝ ║ ",
    " ╚═════════╝ ",
];

#[allow(dead_code)]
pub const SHRINE_DECORATION_EMBLEM: &str = "[百]";

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct StratumPalette {
    pub stratum_idx: u8,
    pub name: &'static str,
    pub desc: &'static str,
    pub wall_color: Color,
    pub floor_primary: Color,
    pub floor_secondary: Color,
    pub floor_tertiary: Color,
    pub border_color: Color,
    pub accent_color: Color,
    pub chasm_wind_color: Color,
    pub chasm_depth_bg: Color,
}

pub fn get_stratum_palette(floor: u8) -> StratumPalette {
    match floor {
        1..=10 => StratumPalette {
            stratum_idx: 1,
            name: "Stratum I: Subterranean Vaults",
            desc: "Ancient mossy stone halls lit by dying embers",
            wall_color: Color::Rgb(115, 130, 155),
            floor_primary: Color::Rgb(85, 95, 115),
            floor_secondary: Color::Rgb(70, 105, 90),
            floor_tertiary: Color::Rgb(105, 115, 130),
            border_color: Color::Rgb(120, 190, 255),
            accent_color: Color::Rgb(120, 190, 255),
            chasm_wind_color: Color::Rgb(65, 90, 120),
            chasm_depth_bg: Color::Rgb(8, 10, 16),
        },
        11..=20 => StratumPalette {
            stratum_idx: 2,
            name: "Stratum II: Sunless Catacombs",
            desc: "Echoing burial ruins haunted by restless phantoms",
            wall_color: Color::Rgb(135, 120, 160),
            floor_primary: Color::Rgb(95, 85, 120),
            floor_secondary: Color::Rgb(120, 95, 140),
            floor_tertiary: Color::Rgb(80, 75, 105),
            border_color: Color::Rgb(175, 135, 255),
            accent_color: Color::Rgb(175, 135, 255),
            chasm_wind_color: Color::Rgb(120, 75, 175),
            chasm_depth_bg: Color::Rgb(12, 8, 18),
        },
        21..=30 => StratumPalette {
            stratum_idx: 3,
            name: "Stratum III: Abyssal Precipice",
            desc: "Bottomless chasms swept by freezing void winds",
            wall_color: Color::Rgb(105, 115, 145),
            floor_primary: Color::Rgb(85, 95, 125),
            floor_secondary: Color::Rgb(75, 85, 115),
            floor_tertiary: Color::Rgb(105, 115, 140),
            border_color: Color::Rgb(255, 130, 90),
            accent_color: Color::Rgb(255, 130, 90),
            chasm_wind_color: Color::Rgb(85, 125, 210),
            chasm_depth_bg: Color::Rgb(6, 8, 16),
        },
        31..=40 => StratumPalette {
            stratum_idx: 4,
            name: "Stratum IV: Clockwork Bastion",
            desc: "Sealed iron barriers and grinding mechanical gears",
            wall_color: Color::Rgb(165, 140, 95),
            floor_primary: Color::Rgb(120, 105, 80),
            floor_secondary: Color::Rgb(150, 120, 70),
            floor_tertiary: Color::Rgb(105, 90, 65),
            border_color: Color::Rgb(255, 205, 80),
            accent_color: Color::Rgb(255, 205, 80),
            chasm_wind_color: Color::Rgb(180, 135, 55),
            chasm_depth_bg: Color::Rgb(14, 10, 6),
        },
        41..=50 => StratumPalette {
            stratum_idx: 5,
            name: "Stratum V: Iron Foundry",
            desc: "Molten slag sluices and thunderous steam hammers",
            wall_color: Color::Rgb(175, 95, 75),
            floor_primary: Color::Rgb(115, 75, 65),
            floor_secondary: Color::Rgb(145, 85, 60),
            floor_tertiary: Color::Rgb(100, 65, 55),
            border_color: Color::Rgb(255, 145, 60),
            accent_color: Color::Rgb(255, 145, 60),
            chasm_wind_color: Color::Rgb(210, 105, 45),
            chasm_depth_bg: Color::Rgb(20, 8, 6),
        },
        51..=60 => StratumPalette {
            stratum_idx: 6,
            name: "Stratum VI: Verdant Overgrowth",
            desc: "Bioluminescent roots breaking through ancient masonry",
            wall_color: Color::Rgb(90, 145, 110),
            floor_primary: Color::Rgb(65, 110, 80),
            floor_secondary: Color::Rgb(85, 135, 95),
            floor_tertiary: Color::Rgb(55, 95, 70),
            border_color: Color::Rgb(100, 245, 140),
            accent_color: Color::Rgb(100, 245, 140),
            chasm_wind_color: Color::Rgb(80, 195, 120),
            chasm_depth_bg: Color::Rgb(6, 16, 10),
        },
        61..=70 => StratumPalette {
            stratum_idx: 7,
            name: "Stratum VII: Astral Sanctum",
            desc: "Crystalline ether halls humming with celestial power",
            wall_color: Color::Rgb(95, 180, 230),
            floor_primary: Color::Rgb(75, 135, 165),
            floor_secondary: Color::Rgb(115, 215, 235),
            floor_tertiary: Color::Rgb(95, 155, 185),
            border_color: Color::Rgb(90, 245, 230),
            accent_color: Color::Rgb(90, 245, 230),
            chasm_wind_color: Color::Rgb(100, 220, 255),
            chasm_depth_bg: Color::Rgb(6, 12, 22),
        },
        71..=80 => StratumPalette {
            stratum_idx: 8,
            name: "Stratum VIII: Frostpeak Citadel",
            desc: "Glacial spires swept by eternal blizzards",
            wall_color: Color::Rgb(150, 190, 225),
            floor_primary: Color::Rgb(110, 145, 185),
            floor_secondary: Color::Rgb(165, 210, 245),
            floor_tertiary: Color::Rgb(95, 130, 165),
            border_color: Color::Rgb(180, 235, 255),
            accent_color: Color::Rgb(180, 235, 255),
            chasm_wind_color: Color::Rgb(140, 215, 255),
            chasm_depth_bg: Color::Rgb(10, 16, 26),
        },
        81..=90 => StratumPalette {
            stratum_idx: 9,
            name: "Stratum IX: Infernal Spire",
            desc: "Blazing trial chambers forged in ancient dragonfire",
            wall_color: Color::Rgb(185, 90, 70),
            floor_primary: Color::Rgb(130, 80, 70),
            floor_secondary: Color::Rgb(170, 75, 55),
            floor_tertiary: Color::Rgb(110, 65, 60),
            border_color: Color::Rgb(255, 100, 70),
            accent_color: Color::Rgb(255, 100, 70),
            chasm_wind_color: Color::Rgb(235, 105, 40),
            chasm_depth_bg: Color::Rgb(18, 6, 6),
        },
        _ => StratumPalette {
            stratum_idx: 10,
            name: "Stratum X: The Celestial Apex",
            desc: "The legendary golden zenith of the hundred floors",
            wall_color: Color::Rgb(240, 215, 125),
            floor_primary: Color::Rgb(175, 155, 95),
            floor_secondary: Color::Rgb(225, 205, 135),
            floor_tertiary: Color::Rgb(150, 135, 80),
            border_color: Color::Rgb(255, 230, 100),
            accent_color: Color::Rgb(255, 230, 100),
            chasm_wind_color: Color::Rgb(255, 230, 120),
            chasm_depth_bg: Color::Rgb(16, 14, 8),
        },
    }
}

#[allow(dead_code)]
pub fn get_floor_biome(floor: u32) -> (&'static str, &'static str, Color) {
    let p = get_stratum_palette(floor.clamp(1, 100) as u8);
    (p.name, p.desc, p.accent_color)
}

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
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(8), // Logo Banner
            Constraint::Length(3), // Username
            Constraint::Min(12),   // Class Selector & Preview
            Constraint::Length(3), // Keybindings footer
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
        .direction(LayoutDirection::Horizontal)
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

    let preview_block = Block::default()
        .title(" Weapon / Emblem ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(TITLE_COLOR_ACCENT));
    let preview_p = Paragraph::new(preview_art)
        .alignment(Alignment::Center)
        .block(preview_block);
    frame.render_widget(preview_p, class_chunks[1]);

    // 4. Footer controls
    let footer_line = Line::from(vec![
        Span::styled(
            " [Enter] ",
            Style::default()
                .fg(TITLE_COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Ascend the Tower  | "),
        Span::styled(
            " [A/D / Left/Right] ",
            Style::default().fg(TITLE_COLOR_PRIMARY),
        ),
        Span::raw("Switch Class  | "),
        Span::styled(" [TAB] ", Style::default().fg(TITLE_COLOR_PRIMARY)),
        Span::raw("Edit Name  | "),
        Span::styled(" [Q / Esc] ", Style::default().fg(Color::Red)),
        Span::raw("Quit"),
    ]);
    let footer_p = Paragraph::new(footer_line).alignment(Alignment::Center);
    frame.render_widget(footer_p, chunks[3]);
}

fn render_connecting_screen(frame: &mut Frame, app: &AppState) {
    let area = frame.area();
    let popup = centered_rect(60, 25, area);
    frame.render_widget(Clear, popup);

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Establishing Link to the Hundred-Floor Tower...",
            Style::default()
                .fg(TITLE_COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Target Gateway: {}", app.server_url),
            Style::default().fg(Color::Gray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press [Esc] to cancel connection attempt",
            Style::default().fg(TITLE_COLOR_MUTED),
        )),
    ];

    let p = Paragraph::new(text).alignment(Alignment::Center).block(
        Block::default()
            .title(" Connecting ")
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

// =========================================================================
// STAGE 4: 4-PANE TUI COMMAND DECK
// =========================================================================

fn render_game_screen(frame: &mut Frame, app: &mut AppState) {
    let area = frame.area();

    // 4-Pane TUI Command Deck:
    // 1. Top Status Bar: Height 3
    // 2. Main Middle Deck: Left 72% Camera Viewport, Right 28% Sidebar (3 stacked panels)
    // 3. Bottom HUD: Height 4
    let deck_layout = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(3), // 1. Top Status Bar
            Constraint::Min(10),   // Middle Deck (72% Camera + 28% Sidebar)
            Constraint::Length(4), // 4. Bottom HUD
        ])
        .split(area);

    render_top_status_bar(frame, app, deck_layout[0]);

    let middle_layout = Layout::default()
        .direction(LayoutDirection::Horizontal)
        .constraints([
            Constraint::Percentage(72), // 2. Main Camera Viewport (Left 72%)
            Constraint::Percentage(28), // 3. Sidebar (Right 28%)
        ])
        .split(deck_layout[1]);

    render_camera_viewport(frame, app, middle_layout[0]);
    render_sidebar(frame, app, middle_layout[1]);
    render_bottom_hud(frame, app, deck_layout[2]);

    // Slain adventurer overlay
    if app.my_player().map_or(false, |p| !p.is_alive()) {
        render_death_screen(frame, area);
    }

    // Full-width ServerBanner popup overlay across all connected terminals
    if let Some(ref banner) = app.server_banner {
        render_server_banner(frame, banner, area);
    }
}

// -------------------------------------------------------------------------
// 1. Top Status Bar: Floor & Stratum, Shrine [百] progress, online climbers
// -------------------------------------------------------------------------
fn render_top_status_bar(frame: &mut Frame, app: &AppState, area: Rect) {
    let palette = get_stratum_palette(app.current_floor.clamp(1, 100) as u8);
    let tower_bank = app
        .tick_state
        .as_ref()
        .map(|t| t.tower_shards_bank)
        .unwrap_or(0);
    let climbers_count = app
        .tick_state
        .as_ref()
        .map(|t| t.players.len())
        .unwrap_or(0);

    let (shrine_cur, shrine_goal, stairs_unlocked) = if let Some(tick) = &app.tick_state {
        (tick.shrine_shards, tick.shrine_goal, tick.stairs_unlocked)
    } else {
        (0, 0, true)
    };

    let shard_ratio = if shrine_goal == 0 {
        1.0
    } else {
        (shrine_cur as f64 / shrine_goal as f64).clamp(0.0, 1.0)
    };

    let (stairs_compass, stairs_dist) =
        if let (Some(me), Some(map)) = (app.my_player(), &app.current_map) {
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

    let (shrine_compass, shrine_dist) =
        if let (Some(me), Some(map)) = (app.my_player(), &app.current_map) {
            if let Some(spos) = map.shrine_pos {
                let dx = spos.x - me.pos.x;
                let dy = spos.y - me.pos.y;
                let dist = me.pos.manhattan_distance(spos);
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
            }
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

    let top_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette.border_color));
    let inner = top_block.inner(area);
    frame.render_widget(top_block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let cols = Layout::default()
        .direction(LayoutDirection::Horizontal)
        .constraints([
            Constraint::Percentage(36), // Floor & Stratum name
            Constraint::Percentage(38), // Shrine [百] Shard Progress Gauge
            Constraint::Percentage(26), // Online Climbers & Gate/Stairs
        ])
        .split(inner);

    // Left: Floor & Stratum Name
    let floor_line = Line::from(vec![
        Span::styled(
            " HYAKU (百) ",
            Style::default()
                .fg(palette.accent_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("F"),
        Span::styled(
            format!("{}/100 ", app.current_floor),
            Style::default()
                .fg(TITLE_COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("[ {} ]", palette.name),
            Style::default()
                .fg(palette.accent_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(floor_line), cols[0]);

    // Middle: Shrine [百] Shard Progress Gauge
    let gauge_label = if shrine_goal == 0 || stairs_unlocked {
        format!("Shrine [百]: UNLOCKED ({}/{} Shards)", shrine_cur, shrine_goal)
    } else {
        format!("Shrine [百]: {}/{} [{} {}m] (Bank: {})", shrine_cur, shrine_goal, shrine_compass, shrine_dist, tower_bank)
    };
    let shrine_gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(if stairs_unlocked { Color::Rgb(80, 255, 160) } else { palette.accent_color })
                .bg(Color::Rgb(25, 30, 45))
                .add_modifier(Modifier::BOLD),
        )
        .ratio(shard_ratio)
        .label(gauge_label);
    frame.render_widget(shrine_gauge, cols[1]);

    // Right: Online Climber Count & Gate/Stairs status
    let status_line = Line::from(vec![
        Span::raw("Climbers: "),
        Span::styled(
            format!("{}", climbers_count),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | Gate: "),
        if is_gate_locked {
            Span::styled(
                "[SEALED] ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                "[OPEN] ",
                Style::default()
                    .fg(Color::Rgb(80, 255, 160))
                    .add_modifier(Modifier::BOLD),
            )
        },
        Span::raw("Stairs: "),
        if !stairs_unlocked {
            Span::styled(
                "[SEALED] ",
                Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                "[OPEN] ",
                Style::default()
                    .fg(Color::Rgb(80, 255, 160))
                    .add_modifier(Modifier::BOLD),
            )
        },
        Span::styled(
            if !stairs_unlocked && !is_gate_locked {
                // Gate is opened, guide player directly to the Shrine to unseal the stairs!
                format!("Shrine:[{} {}m]", shrine_compass, shrine_dist)
            } else {
                format!("Stairs:[{} {}m]", stairs_compass, stairs_dist)
            },
            Style::default().fg(Color::Rgb(80, 255, 180)),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(status_line).alignment(Alignment::Right),
        cols[2],
    );
}

/// Bresenham's integer line algorithm to draw the Co-op Lightning Tether between climbers
pub fn bresenham_line(start: Position, end: Position) -> Vec<Position> {
    let mut points = Vec::new();
    let dx = (end.x - start.x).abs();
    let dy = (end.y - start.y).abs();
    let sx = if start.x < end.x { 1 } else { -1 };
    let sy = if start.y < end.y { 1 } else { -1 };
    let mut err = dx - dy;

    let mut cur = start;
    while cur != end {
        points.push(cur);
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            cur.x += sx;
        }
        if e2 < dx {
            err += dx;
            cur.y += sy;
        }
    }
    points.push(end);
    points
}

// -------------------------------------------------------------------------
// 2. Main Camera Viewport (Left 72%): 2D Map, @tags, speech bubbles, Shrine [百]
// -------------------------------------------------------------------------
fn render_camera_viewport(frame: &mut Frame, app: &mut AppState, area: Rect) {
    let inner_w = area.width.saturating_sub(2) as i32;
    let inner_h = area.height.saturating_sub(2) as i32;

    app.update_camera(inner_w, inner_h);

    let palette = get_stratum_palette(app.current_floor.clamp(1, 100) as u8);
    let border_color = if let Some((flash_color, _)) = app.screen_flash {
        flash_color
    } else {
        palette.border_color
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
            " Floor {}: {} {}(Main Camera Viewport 72%) ",
            app.current_floor, palette.name, title_badge
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

    // Dynamic landmarks for ambient atmosphere
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

    // 1. Render base map tiles
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
                    let near_shrine =
                        shrine_pos.map_or(false, |sp| pos.chebyshev_distance(sp) <= 2);
                    let near_stairs =
                        stairs_pos.map_or(false, |stp| pos.chebyshev_distance(stp) <= 1);

                    let (ch, style) = tile_to_glyph_and_style(
                        tile,
                        tick_counter,
                        pos,
                        &palette,
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
                let near_shrine =
                    shrine_pos.map_or(false, |sp| pos.chebyshev_distance(sp) <= 2);
                let near_stairs =
                    stairs_pos.map_or(false, |stp| pos.chebyshev_distance(stp) <= 1);

                let (ch, style) = tile_to_glyph_and_style(
                    tile,
                    tick_counter,
                    pos,
                    &palette,
                    near_campfire,
                    near_shrine,
                    near_stairs,
                );
                grid[sy as usize][sx as usize] = (ch, style);
            }
        }

        // 3. Render carvings with runic pulse and overhead preview if near
        for carving in &tick.carvings {
            let (sx, sy) = app.world_to_screen(carving.pos);
            if sx >= 0 && sx < inner_w && sy >= 0 && sy < inner_h {
                // Ensure carvings never cover mechanical interactive tiles (plates, campfires, shrines, gates, stairs)
                let is_interactive = tick.tiles_override.iter().any(|(p, t)| {
                    *p == carving.pos
                        && matches!(
                            t,
                            Tile::PressurePlate { .. }
                                | Tile::Gate { .. }
                                | Tile::Shrine
                                | Tile::Campfire
                                | Tile::StairsUp
                        )
                }) || app.current_map.as_ref().map_or(false, |m| {
                    m.get_tile(carving.pos).map_or(false, |t| {
                        matches!(
                            t,
                            Tile::PressurePlate { .. }
                                | Tile::Gate { .. }
                                | Tile::Shrine
                                | Tile::Campfire
                                | Tile::StairsUp
                        )
                    })
                });

                if !is_interactive {
                    let pulse_fg = if (tick_counter / 3) % 2 == 0 {
                        Color::Rgb(255, 230, 80)
                    } else {
                        Color::Rgb(255, 180, 40)
                    };
                    grid[sy as usize][sx as usize] = (
                        '?',
                        Style::default()
                            .fg(pulse_fg)
                            .bg(Color::Rgb(45, 35, 10))
                            .add_modifier(Modifier::BOLD),
                    );
                }

                let is_near = app
                    .my_player()
                    .map_or(false, |me| me.pos.manhattan_distance(carving.pos) <= 2);
                if is_near && sy > 0 {
                    let preview = format!(
                        "[? @{}: \"{}\" (+{}U)]",
                        carving.author, carving.message, carving.upvotes
                    );
                    let preview_len = preview.chars().count() as i32;
                    let p_start_x = (sx - preview_len / 2).max(0);
                    let p_y = sy - 1;
                    if p_y >= 0 && p_y < inner_h {
                        for (idx, ch) in preview.chars().enumerate() {
                            let cur_x = p_start_x + idx as i32;
                            if cur_x >= 0 && cur_x < inner_w {
                                let target_world_pos = Position::new(
                                    cur_x + app.camera_offset.0,
                                    p_y + app.camera_offset.1,
                                );
                                let tile_is_interactive = app.current_map.as_ref().map_or(false, |m| {
                                    m.get_tile(target_world_pos).map_or(false, |t| {
                                        matches!(
                                            t,
                                            Tile::PressurePlate { .. }
                                                | Tile::Gate { .. }
                                                | Tile::Shrine
                                                | Tile::Campfire
                                                | Tile::StairsUp
                                        )
                                    })
                                }) || tick.tiles_override.iter().any(|(p, t)| {
                                    *p == target_world_pos
                                        && matches!(
                                            t,
                                            Tile::PressurePlate { .. }
                                                | Tile::Gate { .. }
                                                | Tile::Shrine
                                                | Tile::Campfire
                                                | Tile::StairsUp
                                        )
                                });

                                if !tile_is_interactive {
                                    grid[p_y as usize][cur_x as usize] = (
                                        ch,
                                        Style::default()
                                            .fg(Color::Rgb(255, 240, 150))
                                            .bg(Color::Rgb(40, 30, 10))
                                            .add_modifier(Modifier::BOLD),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // 4. Render projectiles (directional melee slash arcs, arrows, aether shards, ether bolts)
        // Sort so slash arcs ('╱', '─', '│', '╲') render with priority over ground shard particles
        let mut sorted_projs: Vec<_> = tick.projectiles.iter().collect();
        sorted_projs.sort_by_key(|p| match p.glyph {
            '╱' | '│' | '╲' | '─' => 1,
            _ => 0,
        });

        for proj in sorted_projs {
            let (sx, sy) = app.world_to_screen(proj.pos);
            if sx >= 0 && sx < inner_w && sy >= 0 && sy < inner_h {
                let cell_bg = grid[sy as usize][sx as usize].1.bg.unwrap_or(Color::Rgb(20, 24, 32));
                let (fg_color, bg_color) = match proj.glyph {
                    '╱' | '│' | '╲' | '─' => (
                        Color::Rgb(
                            proj.color_rgb.0.saturating_add(60),
                            proj.color_rgb.1.saturating_add(60),
                            proj.color_rgb.2.saturating_add(60),
                        ),
                        cell_bg, // Clean native floor tile background! No muddy brown box
                    ),
                    '*' => (
                        Color::Rgb(255, 220, 40), // Dropped Aether Shards
                        cell_bg,
                    ),
                    '↯' | '⚡' => (
                        Color::Rgb(120, 240, 255), // Lightning sparks
                        cell_bg,
                    ),
                    _ => (
                        Color::Rgb(proj.color_rgb.0, proj.color_rgb.1, proj.color_rgb.2),
                        cell_bg,
                    ),
                };

                let style = Style::default()
                    .fg(fg_color)
                    .bg(bg_color)
                    .add_modifier(Modifier::BOLD);
                grid[sy as usize][sx as usize] = (proj.glyph, style);
            }
        }

        // 4.5. Render active Holo-Decoys (holding pressure plates, distracting enemies)
        for decoy in &tick.decoys {
            let (sx, sy) = app.world_to_screen(decoy.pos);
            if sx >= 0 && sx < inner_w && sy >= 0 && sy < inner_h {
                let remaining_secs = (decoy.remaining_ticks + 19) / 20;
                let pulse_fg = if (tick_counter / 2) % 2 == 0 {
                    Color::Rgb(100, 240, 255)
                } else {
                    Color::Rgb(60, 180, 255)
                };
                let style = Style::default()
                    .fg(pulse_fg)
                    .bg(Color::Rgb(15, 35, 60))
                    .add_modifier(Modifier::BOLD);
                grid[sy as usize][sx as usize] = ('§', style);

                // Overhead Holo label
                let tag = format!("~HOLO {}s~", remaining_secs);
                let tag_len = tag.chars().count() as i32;
                let tag_start_x = (sx - tag_len / 2).max(0);
                let tag_y = sy - 1;

                if tag_y >= 0 && tag_y < inner_h {
                    for (idx, ch) in tag.chars().enumerate() {
                        let cur_x = tag_start_x + idx as i32;
                        if cur_x >= 0 && cur_x < inner_w {
                            grid[tag_y as usize][cur_x as usize] = (
                                ch,
                                Style::default()
                                    .fg(Color::Rgb(120, 230, 255))
                                    .bg(Color::Rgb(10, 25, 45))
                                    .add_modifier(Modifier::BOLD),
                            );
                        }
                    }
                }
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

        // 5b. Render Lightning Tethers connecting tethered teammates
        let mut rendered_tethers: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
        for p in &tick.players {
            if let Some(ref target_id) = p.tether_target {
                let pair_key = if p.id < *target_id {
                    (p.id.clone(), target_id.clone())
                } else {
                    (target_id.clone(), p.id.clone())
                };

                if rendered_tethers.contains(&pair_key) {
                    continue;
                }
                rendered_tethers.insert(pair_key);

                let target_pos_opt = if let Some(target) = tick.players.iter().find(|t| t.id == *target_id) {
                    if target.is_alive() { Some(target.pos) } else { None }
                } else if target_id.starts_with("decoy-") {
                    let decoy_id = target_id.trim_start_matches("decoy-").parse::<u64>().ok();
                    tick.decoys.iter().find(|d| Some(d.id) == decoy_id).map(|d| d.pos)
                } else {
                    None
                };

                if let Some(target_pos) = target_pos_opt {
                    if p.is_alive() && p.pos.manhattan_distance(target_pos) <= 8 {
                        let line_points = bresenham_line(p.pos, target_pos);
                        for (idx, pt) in line_points.iter().enumerate() {
                            if *pt == p.pos || *pt == target_pos {
                                continue;
                            }
                            let (sx, sy) = app.world_to_screen(*pt);
                            if sx >= 0 && sx < inner_w && sy >= 0 && sy < inner_h {
                                let spark_glyph = match (tick_counter + idx as u64) % 3 {
                                    0 => '↯',
                                    1 => '⠶',
                                    _ => '~',
                                };
                                grid[sy as usize][sx as usize] = (
                                    spark_glyph,
                                    Style::default()
                                        .fg(Color::Rgb(120, 240, 255))
                                        .bg(Color::Rgb(15, 35, 55))
                                        .add_modifier(Modifier::BOLD),
                                );
                            }
                        }
                    }
                }
            }
        }

        // 6. Render other players & local player with @username tags and temporary speech bubbles
        for player in &tick.players {
            let (sx, sy) = app.world_to_screen(player.pos);
            let is_me = app.my_player_id.as_deref() == Some(&player.id);
            if sx >= 0 && sx < inner_w && sy >= 0 && sy < inner_h {
                if !player.is_alive() {
                    grid[sy as usize][sx as usize] = (
                        '†',
                        Style::default()
                            .fg(Color::Rgb(220, 80, 80))
                            .bg(Color::Rgb(50, 15, 15))
                            .add_modifier(Modifier::BOLD),
                    );
                    if !is_me && sy > 0 {
                        let revive_tag = format!("[E: Revive @{}]", player.username);
                        let r_len = revive_tag.chars().count() as i32;
                        let r_start_x = (sx - r_len / 2).max(0);
                        let r_y = sy - 1;
                        if r_y >= 0 && r_y < inner_h {
                            for (idx, ch) in revive_tag.chars().enumerate() {
                                let cur_x = r_start_x + idx as i32;
                                if cur_x >= 0 && cur_x < inner_w {
                                    grid[r_y as usize][cur_x as usize] = (
                                        ch,
                                        Style::default()
                                            .fg(Color::Rgb(120, 255, 160))
                                            .bg(Color::Rgb(20, 40, 20))
                                            .add_modifier(Modifier::BOLD),
                                    );
                                }
                            }
                        }
                    }
                    continue;
                }

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

                // Render @username tag above player's head
                let tag = format!("@{}", player.username);
                let tag_len = tag.chars().count() as i32;
                let tag_start_x = (sx - tag_len / 2).max(0);
                let tag_y = sy - 1;

                if tag_y >= 0 && tag_y < inner_h {
                    for (idx, ch) in tag.chars().enumerate() {
                        let cur_x = tag_start_x + idx as i32;
                        if cur_x >= 0 && cur_x < inner_w {
                            grid[tag_y as usize][cur_x as usize] = (
                                ch,
                                Style::default()
                                    .fg(if is_me { Color::Yellow } else { Color::Cyan })
                                    .bg(Color::Rgb(15, 20, 30))
                                    .add_modifier(Modifier::BOLD),
                            );
                        }
                    }
                }

                // Render temporary speech bubble above player's head if they have an active bubble
                if let Some(bubble) = app.speech_bubbles.get(&player.username) {
                    let clean_text = bubble.text.trim();
                    if !clean_text.is_empty() {
                        let bubble_str = format!("[\"{}\"]", clean_text);
                        let bubble_len = bubble_str.chars().count() as i32;
                        let bubble_start_x = (sx - bubble_len / 2).max(0);
                        let bubble_y = if tag_y > 0 { tag_y - 1 } else { sy + 1 };

                        if bubble_y >= 0 && bubble_y < inner_h {
                            for (idx, ch) in bubble_str.chars().enumerate() {
                                let cur_x = bubble_start_x + idx as i32;
                                if cur_x >= 0 && cur_x < inner_w {
                                    grid[bubble_y as usize][cur_x as usize] = (
                                        ch,
                                        Style::default()
                                            .fg(Color::Rgb(255, 235, 120))
                                            .bg(Color::Rgb(25, 25, 45))
                                            .add_modifier(Modifier::BOLD),
                                    );
                                }
                            }
                        }
                    }
                }
            } else if !is_me && player.is_alive() {
                // Off-screen teammate compass: show direction arrow along the edge of the viewport
                let edge_x = sx.clamp(0, inner_w - 1) as usize;
                let edge_y = sy.clamp(0, inner_h - 1) as usize;
                let arrow_ch = if sx < 0 {
                    '◄'
                } else if sx >= inner_w {
                    '►'
                } else if sy < 0 {
                    '▲'
                } else {
                    '▼'
                };
                let arrow_style = Style::default()
                    .fg(if (tick_counter / 3) % 2 == 0 {
                        Color::Rgb(80, 240, 255)
                    } else {
                        Color::Cyan
                    })
                    .bg(Color::Rgb(15, 30, 45))
                    .add_modifier(Modifier::BOLD);
                grid[edge_y][edge_x] = (arrow_ch, arrow_style);
            }
        }

        // 7. Render local player aiming reticle
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
                        Direction::Up => '▲',
                        Direction::Down => '▼',
                        Direction::Left => '◄',
                        Direction::Right => '►',
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

        // 8. Render floating combat damage and effect texts
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

    // Overlay Runestone Carving Card if standing near a carved message
    if let Some(carving) = app.nearby_carving() {
        if inner_h >= 4 {
            let row1 = inner_h - 2;
            let row2 = inner_h - 1;

            let header_str = format!(
                " [ RUNESTONE CARVING by @{} | Upvotes: {} ] ",
                carving.author, carving.upvotes
            );
            let msg_str = format!(" \"{}\"  --> Press [U] to Upvote ", carving.message);

            for (i, ch) in header_str.chars().enumerate() {
                let x = 2 + i as i32;
                if x < inner_w {
                    grid[row1 as usize][x as usize] = (
                        ch,
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(255, 215, 60))
                            .add_modifier(Modifier::BOLD),
                    );
                }
            }

            for (i, ch) in msg_str.chars().enumerate() {
                let x = 2 + i as i32;
                if x < inner_w {
                    grid[row2 as usize][x as usize] = (
                        ch,
                        Style::default()
                            .fg(Color::Rgb(255, 245, 180))
                            .bg(Color::Rgb(50, 35, 10))
                            .add_modifier(Modifier::BOLD),
                    );
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
    palette: &StratumPalette,
    near_campfire: bool,
    near_shrine: bool,
    near_stairs: bool,
) -> (char, Style) {
    match tile {
        Tile::Wall => ('#', Style::default().fg(palette.wall_color).bg(Color::Rgb(22, 26, 35))),
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
                    0 => (',', palette.floor_secondary),
                    1 => ('.', palette.floor_tertiary),
                    _ => ('·', palette.floor_primary),
                };
                (ch, Style::default().fg(col).bg(Color::Rgb(16, 20, 26)))
            }
        }
        Tile::Chasm => {
            let wind_step = (tick / 2) as i32;
            let wave = ((pos.x * 7).wrapping_add(pos.y * 13).wrapping_add(wind_step)).rem_euclid(18);
            if wave == 0 {
                ('⠄', Style::default().fg(palette.chasm_wind_color).bg(palette.chasm_depth_bg))
            } else if wave == 1 {
                ('⠂', Style::default().fg(palette.chasm_wind_color).bg(palette.chasm_depth_bg))
            } else if wave == 9 {
                ('⠐', Style::default().fg(palette.chasm_wind_color).bg(palette.chasm_depth_bg))
            } else {
                (' ', Style::default().bg(palette.chasm_depth_bg))
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
            // Central Shrine [百]
            let glow = if (tick / 2) % 2 == 0 {
                Color::Rgb(90, 245, 255)
            } else {
                Color::Rgb(180, 255, 255)
            };
            (
                '☼',
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
            '═',
            Style::default()
                .fg(Color::Rgb(240, 185, 95))
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

// -------------------------------------------------------------------------
// 3. Sidebar (Right 28%): 3 stacked panels
//    - Panel 1: Live Radar Minimap (player dots •, enemies, shrine 百)
//    - Panel 2: Nearby Climbers list with HP bars
//    - Panel 3: Tower Comms chat log + active input box
// -------------------------------------------------------------------------
fn render_sidebar(frame: &mut Frame, app: &AppState, area: Rect) {
    let panels = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(10), // Panel 1: Live Radar Minimap
            Constraint::Length(8),  // Panel 2: Nearby Climbers list with HP bars
            Constraint::Min(6),     // Panel 3: Tower Comms chat log + active input box
        ])
        .split(area);

    render_radar_minimap(frame, app, panels[0]);
    render_nearby_climbers(frame, app, panels[1]);
    render_tower_comms(frame, app, panels[2]);
}

fn render_radar_minimap(frame: &mut Frame, app: &AppState, area: Rect) {
    let block = Block::default()
        .title(" [ 1. Live Radar Minimap ] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(TITLE_COLOR_PRIMARY));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rw = inner.width as usize;
    let rh = inner.height.saturating_sub(1) as usize; // reserve last line for radar legend

    if rw == 0 || rh == 0 {
        return;
    }

    let (map_w, map_h) = app
        .current_map
        .as_ref()
        .map(|m| (m.width, m.height))
        .unwrap_or((80, 30));

    let mut radar_grid = vec![vec![(' ', Style::default()); rw]; rh];

    // 1. Draw base map layout onto radar
    if let Some(map) = &app.current_map {
        for ry in 0..rh {
            for rx in 0..rw {
                let wx = ((rx as f32 / rw as f32) * map_w as f32) as i32;
                let wy = ((ry as f32 / rh as f32) * map_h as f32) as i32;
                let pos = Position::new(wx, wy);

                if let Some(tile) = map.get_tile(pos) {
                    let (r_ch, r_style) = match tile {
                        Tile::Wall => ('#', Style::default().fg(Color::DarkGray)),
                        Tile::Floor => ('·', Style::default().fg(Color::Rgb(40, 48, 65))),
                        Tile::Chasm => (' ', Style::default().bg(Color::Black)),
                        Tile::Bridge => ('═', Style::default().fg(Color::Rgb(180, 120, 60))),
                        Tile::Shrine => (
                            '☼',
                            Style::default()
                                .fg(Color::Rgb(80, 240, 255))
                                .add_modifier(Modifier::BOLD),
                        ),
                        Tile::StairsUp => (
                            '▲',
                            Style::default()
                                .fg(Color::Rgb(80, 255, 140))
                                .add_modifier(Modifier::BOLD),
                        ),
                        Tile::Gate { open: true, .. } => (
                            '/',
                            Style::default().fg(Color::Rgb(80, 255, 140)),
                        ),
                        Tile::Gate { open: false, .. } => (
                            '+',
                            Style::default().fg(Color::Red),
                        ),
                        Tile::PressurePlate { .. } => (
                            'o',
                            Style::default().fg(Color::Yellow),
                        ),
                        Tile::Campfire => (
                            '*',
                            Style::default().fg(Color::Rgb(255, 160, 40)),
                        ),
                    };
                    radar_grid[ry][rx] = (r_ch, r_style);
                }
            }
        }
    }

    // 2. Plot enemies on radar ('x' or '!' for boss)
    if let Some(tick) = &app.tick_state {
        for enemy in &tick.enemies {
            if !enemy.is_alive() {
                continue;
            }
            let rx = ((enemy.pos.x as f32 / map_w as f32) * rw as f32) as usize;
            let ry = ((enemy.pos.y as f32 / map_h as f32) * rh as f32) as usize;
            if rx < rw && ry < rh {
                let glyph = if enemy.is_boss { '!' } else { 'x' };
                radar_grid[ry][rx] = (
                    glyph,
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                );
            }
        }

        // 3. Plot player dots ('•') on radar
        for player in &tick.players {
            if !player.is_alive() {
                continue;
            }
            let rx = ((player.pos.x as f32 / map_w as f32) * rw as f32) as usize;
            let ry = ((player.pos.y as f32 / map_h as f32) * rh as f32) as usize;
            if rx < rw && ry < rh {
                let is_me = app.my_player_id.as_deref() == Some(&player.id);
                let col = if is_me {
                    Color::Yellow
                } else {
                    Color::Rgb(player.color_rgb.0, player.color_rgb.1, player.color_rgb.2)
                };
                radar_grid[ry][rx] = (
                    '•',
                    Style::default()
                        .fg(col)
                        .add_modifier(Modifier::BOLD),
                );
            }
        }

        // 3.5. Plot active Holo-Decoys ('§') on radar
        for decoy in &tick.decoys {
            let rx = ((decoy.pos.x as f32 / map_w as f32) * rw as f32) as usize;
            let ry = ((decoy.pos.y as f32 / map_h as f32) * rh as f32) as usize;
            if rx < rw && ry < rh {
                radar_grid[ry][rx] = (
                    '§',
                    Style::default()
                        .fg(Color::Rgb(100, 230, 255))
                        .add_modifier(Modifier::BOLD),
                );
            }
        }
    }

    let mut lines: Vec<Line> = radar_grid
        .into_iter()
        .map(|row| {
            Line::from(
                row.into_iter()
                    .map(|(ch, style)| Span::styled(ch.to_string(), style))
                    .collect::<Vec<Span>>(),
            )
        })
        .collect();

    // Radar Legend line
    lines.push(Line::from(vec![
        Span::styled("•", Style::default().fg(Color::Yellow)),
        Span::raw(" You  "),
        Span::styled("•", Style::default().fg(Color::Green)),
        Span::raw(" Ally  "),
        Span::styled("百", Style::default().fg(Color::Cyan)),
        Span::raw(" Shrine  "),
        Span::styled("x", Style::default().fg(Color::Red)),
        Span::raw(" Foe"),
    ]));

    let p = Paragraph::new(lines);
    frame.render_widget(p, inner);
}

fn render_nearby_climbers(frame: &mut Frame, app: &AppState, area: Rect) {
    let block = Block::default()
        .title(" [ 2. Nearby Climbers ] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(TITLE_COLOR_ACCENT));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    if let Some(tick) = &app.tick_state {
        let me_pos = app.my_player().map(|p| p.pos);

        for player in &tick.players {
            let is_me = app.my_player_id.as_deref() == Some(&player.id);
            let dist_str = if let Some(mpos) = me_pos {
                if is_me {
                    "self".to_string()
                } else {
                    format!("{}m", mpos.manhattan_distance(player.pos))
                }
            } else {
                "-".to_string()
            };

            let (bar_str, bar_col) = format_mini_bar(player.hp, player.max_hp, 6);
            let name_prefix = if is_me { "> @" } else { "  @" };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{}{}", name_prefix, player.username),
                    Style::default()
                        .fg(if is_me { Color::Yellow } else { Color::White })
                        .add_modifier(if is_me { Modifier::BOLD } else { Modifier::empty() }),
                ),
                Span::styled(format!(" ({})", dist_str), Style::default().fg(Color::DarkGray)),
            ]));

            lines.push(Line::from(vec![
                Span::raw("   HP: "),
                Span::styled(bar_str, Style::default().fg(bar_col)),
                Span::styled(
                    format!(" {}/{}", player.hp, player.max_hp),
                    Style::default().fg(Color::Gray),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("MP:{}", player.mana),
                    Style::default().fg(Color::Cyan),
                ),
            ]));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No climbers nearby",
            Style::default().fg(TITLE_COLOR_MUTED),
        )));
    }

    let p = Paragraph::new(lines);
    frame.render_widget(p, inner);
}

fn render_tower_comms(frame: &mut Frame, app: &AppState, area: Rect) {
    let block = Block::default()
        .title(" [ 3. Tower Comms ] ")
        .borders(Borders::ALL)
        .border_style(match app.input_mode {
            InputMode::Chatting => Style::default().fg(TITLE_COLOR_ACCENT),
            InputMode::CarvingMessage => Style::default().fg(Color::LightYellow),
            InputMode::Normal => Style::default().fg(Color::DarkGray),
        });
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let chunks = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Min(2),    // Chat log history
            Constraint::Length(1), // Active input line
        ])
        .split(inner);

    // 1. History log
    let mut history_lines = Vec::new();
    for (sender, text) in app.chat_log.iter().rev().take(inner.height.saturating_sub(1) as usize).rev() {
        let is_system = sender == "SYSTEM" || sender == "TOWER SYSTEM";
        history_lines.push(Line::from(vec![
            Span::styled(
                format!("[{}]: ", sender),
                Style::default()
                    .fg(if is_system { Color::Yellow } else { Color::Cyan })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(text),
        ]));
    }

    let history_p = Paragraph::new(history_lines).wrap(Wrap { trim: true });
    frame.render_widget(history_p, chunks[0]);

    // 2. Active input line
    let input_line = match app.input_mode {
        InputMode::Chatting => Line::from(vec![
            Span::styled(
                "> Chat: ",
                Style::default().fg(TITLE_COLOR_ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(&app.chat_input),
            Span::styled("_", Style::default().fg(TITLE_COLOR_ACCENT)),
        ]),
        InputMode::CarvingMessage => Line::from(vec![
            Span::styled(
                "> Carve: ",
                Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD),
            ),
            Span::raw(&app.carving_input),
            Span::styled("_", Style::default().fg(Color::LightYellow)),
        ]),
        InputMode::Normal => {
            if let Some(carving) = app.nearby_carving() {
                Line::from(vec![
                    Span::styled(
                        format!("[U] Upvote @{}'s Runestone | ", carving.author),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "[Enter] Chat | [F] Carve",
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            } else {
                Line::from(Span::styled(
                    "[Enter] to send chat | [F] carve message",
                    Style::default().fg(Color::DarkGray),
                ))
            }
        }
    };

    frame.render_widget(Paragraph::new(input_line), chunks[1]);
}

// -------------------------------------------------------------------------
// 4. Bottom HUD: Local HP bar, Mana bar, Shards held, and keybind hints
// -------------------------------------------------------------------------
fn render_bottom_hud(frame: &mut Frame, app: &AppState, area: Rect) {
    let block = Block::default()
        .title(" [ Adventurer HUD & Tactical Controls ] ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(TITLE_COLOR_PRIMARY));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let rows = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(1), // HP / MP / Shards gauges
            Constraint::Length(1), // Keybind hints
        ])
        .split(inner);

    let my_player = app.my_player();
    let (hp, max_hp) = my_player.map(|p| (p.hp, p.max_hp)).unwrap_or((0, 100));
    let (mana, max_mana) = my_player
        .map(|p| (p.mana, p.max_mana))
        .unwrap_or((0, 100));
    let my_shards = my_player.map(|p| p.shards).unwrap_or(0);
    let tower_bank = app
        .tick_state
        .as_ref()
        .map(|t| t.tower_shards_bank)
        .unwrap_or(0);

    let hp_ratio = (hp as f64 / max_hp.max(1) as f64).clamp(0.0, 1.0);
    let mana_ratio = (mana as f64 / max_mana.max(1) as f64).clamp(0.0, 1.0);

    let cols = Layout::default()
        .direction(LayoutDirection::Horizontal)
        .constraints([
            Constraint::Percentage(28), // HP Bar
            Constraint::Percentage(28), // MP Bar
            Constraint::Percentage(44), // Shards & Class Details
        ])
        .split(rows[0]);

    // HP Bar
    let hp_gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(Color::Red)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .ratio(hp_ratio)
        .label(format!("HP: {}/{}", hp, max_hp));
    frame.render_widget(hp_gauge, cols[0]);

    // MP Bar
    let mana_gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(Color::Blue)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .ratio(mana_ratio)
        .label(format!("MP: {}/{}", mana, max_mana));
    frame.render_widget(mana_gauge, cols[1]);

    // Shards & Class details
    let class_name = app.current_class_preset().name;
    let reach_desc = match app.class_idx {
        1 => "Spell Reach 3",
        2 => "Ranged Reach 4",
        _ => "Melee Reach 1",
    };

    let shards_line = Line::from(vec![
        Span::raw("  Shards: "),
        Span::styled(
            format!("{}", my_shards),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | Bank: "),
        Span::styled(
            format!("{}", tower_bank),
            Style::default().fg(Color::LightYellow),
        ),
        Span::raw(" | ["),
        Span::styled(
            format!("{} {}", app.last_dir.arrow_glyph(), class_name),
            Style::default().fg(TITLE_COLOR_ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" - {}", reach_desc), Style::default().fg(Color::Gray)),
        Span::raw("]"),
    ]);
    frame.render_widget(Paragraph::new(shards_line), cols[2]);

    // Keybindings hints
    let mut hint_spans = vec![
        Span::styled("[WASD]", Style::default().fg(TITLE_COLOR_ACCENT)),
        Span::raw(" Move | "),
        Span::styled("[Space]", Style::default().fg(TITLE_COLOR_ACCENT)),
        Span::raw(" Slash Sweep | "),
        Span::styled("[Q]", Style::default().fg(Color::Rgb(120, 240, 255)).add_modifier(Modifier::BOLD)),
        Span::raw(" Tether ↯ (Ally/Decoy ≤8m) | "),
        Span::styled("[R]", Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
        Span::raw(" Decoy | "),
        Span::styled("[B]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Bridge (5S) | "),
        Span::styled("[E]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" Revive/Shrine/Ascend | "),
    ];

    if app.nearby_carving().is_some() {
        hint_spans.push(Span::styled(
            "[U] Runestone | ",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }

    hint_spans.extend(vec![
        Span::styled("[1-4]", Style::default().fg(Color::Rgb(80, 240, 255))),
        Span::raw(" Callouts | "),
        Span::styled("[C]", Style::default().fg(Color::Rgb(255, 140, 40))),
        Span::raw(" Rest | "),
        Span::styled("[Enter]", Style::default().fg(Color::White)),
        Span::raw(" Chat | "),
        Span::styled("[F]", Style::default().fg(Color::LightYellow)),
        Span::raw(" Carve | "),
        Span::styled("[Esc]", Style::default().fg(Color::Red)),
        Span::raw(" Quit"),
    ]);

    let hints_line = Line::from(hint_spans);
    frame.render_widget(Paragraph::new(hints_line).alignment(Alignment::Center), rows[1]);
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
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(LayoutDirection::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_server_banner(frame: &mut Frame, banner: &ServerBannerData, area: Rect) {
    let banner_h = 7u16;
    let banner_y = (area.height.saturating_sub(banner_h)) / 3;
    let banner_area = Rect::new(area.x, area.y + banner_y, area.width, banner_h.min(area.height));

    frame.render_widget(Clear, banner_area);

    let border_color = Color::Rgb(255, 215, 0); // Radiant Gold
    let banner_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
        .title(" ★ TOWER CO-OP ANNOUNCEMENT ★ ")
        .title_alignment(Alignment::Center);

    let inner = banner_block.inner(banner_area);
    frame.render_widget(banner_block, banner_area);

    if inner.height < 3 || inner.width < 10 {
        return;
    }

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            banner.title.clone(),
            Style::default()
                .fg(Color::Rgb(80, 240, 255))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            banner.message.clone(),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(
                "  [ Stand on Ascension Stairs [▲] and press [E] to Ascend! ]  ",
                Style::default()
                    .fg(Color::Rgb(80, 255, 160))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let p = Paragraph::new(text).alignment(Alignment::Center);
    frame.render_widget(p, inner);
}
