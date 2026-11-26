use crate::app::{
    AppState, InputMode, Screen, ServerBannerData, CLASS_PREVIEWS, TITLE_COLOR_ACCENT,
    TITLE_COLOR_MUTED,
};
use hyaku_protocol::{BossPhase, EnemyState, Position, RaidBoss, Tile, RANGER_CLASS};
use ratatui::{
    layout::{Alignment, Constraint, Direction as LayoutDirection, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, Paragraph, Wrap},
    Frame,
};

use std::time::{Duration, Instant};

pub(crate) mod prologue;
mod ending;
mod raid_art;
use raid_art::{mirror_wing, raid_sprite, raid_wings};
pub(crate) use ending::ENDING_DURATION;

#[derive(Clone, Copy)]
pub(crate) enum BurstKind {
    Enemy,
    Shards,
    Shrine,
}

impl BurstKind {
    fn motion(self) -> (usize, f32, f32, (u8, u8, u8)) {
        match self {
            Self::Enemy => (12, 10.0, 0.48, (255, 116, 76)),
            Self::Shards => (9, 7.0, 0.40, (255, 206, 96)),
            Self::Shrine => (24, 12.0, 0.78, (144, 238, 223)),
        }
    }
}

struct Burst {
    origin: Position,
    kind: BurstKind,
    started: Instant,
}

#[derive(Default)]
pub(crate) struct ParticleField {
    bursts: Vec<Burst>,
}

impl ParticleField {
    pub(crate) fn emit(&mut self, origin: Position, kind: BurstKind) {
        if self.bursts.len() == 24 {
            self.bursts.remove(0);
        }
        self.bursts.push(Burst { origin, kind, started: Instant::now() });
    }

    pub(crate) fn clear(&mut self) {
        self.bursts.clear();
    }

    pub(crate) fn expire(&mut self, now: Instant) {
        self.bursts.retain(|burst| now.duration_since(burst.started).as_secs_f32() < burst.kind.motion().2);
    }

    fn draw(&self, now: Instant, camera: (i32, i32), grid: &mut [Vec<(char, Style)>]) {
        for burst in &self.bursts {
            let (count, velocity, lifetime, (r, g, b)) = burst.kind.motion();
            let elapsed = now.duration_since(burst.started).as_secs_f32();
            if elapsed >= lifetime {
                continue;
            }
            let age = elapsed / lifetime;
            let glyph = if age < 0.24 { '*' } else if age < 0.56 { '+' } else { '.' };
            let light = 1.0 - age * 0.70;
            let color = Color::Rgb(
                (f32::from(r) * light) as u8,
                (f32::from(g) * light) as u8,
                (f32::from(b) * light) as u8,
            );
            // Integrate an exponentially slowing launch, then give the embers a downward fall.
            let travel = (1.0 - (-3.2 * elapsed).exp()) / 3.2 * velocity;
            let fall = 3.2 * elapsed * elapsed;
            let phase = burst.origin.x as f32 * 0.23 + burst.origin.y as f32 * 0.17;
            for spoke in 0..count {
                let angle = phase + std::f32::consts::TAU * spoke as f32 / count as f32;
                let (sin, cos) = angle.sin_cos();
                let speed = 0.62 + (spoke * 7 % 11) as f32 * 0.08;
                let x = burst.origin.x + (cos * speed * travel).round() as i32 - camera.0;
                let y = burst.origin.y + (sin * speed * travel * 0.64 + fall).round() as i32 - camera.1;
                if x < 0 || y < 0 {
                    continue;
                }
                if let Some(cell) = grid.get_mut(y as usize).and_then(|row| row.get_mut(x as usize)) {
                    if !matches!(cell.0, '#' | '+' | 'o' | '═' | '☼' | '▲' | '♨' | '✉') {
                        *cell = (glyph, cell.1.fg(color));
                    }
                }
            }
        }
    }
}

const DUSK: Color = Color::Rgb(19, 19, 23);
const BRASS: Color = Color::Rgb(222, 183, 119);
const FRAME_STONE: Color = Color::Rgb(123, 112, 94);

fn tower_panel() -> Block<'static> {
    Block::bordered()
        .border_style(Style::default().fg(FRAME_STONE))
        .title_style(Style::default().fg(BRASS))
}

// TODO: Revise the stratum palettes and shrine art.
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
            wall_color: Color::Rgb(135, 128, 115),
            floor_primary: Color::Rgb(103, 97, 86),
            floor_secondary: Color::Rgb(87, 104, 85),
            floor_tertiary: Color::Rgb(127, 119, 103),
            accent_color: BRASS,
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
        Screen::Connected => {
            if let Some(book) = &app.prologue {
                book.render(frame);
            } else if let Some(start) = app.ending_started {
                ending::render(frame, start.elapsed());
            } else {
                render_game_screen(frame, app);
            }
        }
        Screen::Disconnected => render_disconnected_screen(frame, app),
    }
}

fn render_title_screen(frame: &mut Frame, app: &AppState) {
    let area = frame.area();

    let wide = area.width >= 72 && area.height >= 23;
    let large_title = area.width >= 34 && area.height >= 16;
    let title_height = if wide { 11 } else if large_title { 5 } else { 1 };
    let menu = area.centered_horizontally(Constraint::Length(area.width.min(86)));
    let chunks = Layout::vertical([
        Constraint::Length(title_height),
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .margin(1)
    .split(menu);

    let gold = Style::default()
        .fg(BRASS)
        .add_modifier(Modifier::BOLD);
    let stone = Color::Rgb(148, 143, 129);
    frame.render_widget(Block::default().style(Style::default().bg(DUSK)), area);
    if area.width >= 48 && area.height >= 16 {
        render_title_trim(frame, area, menu);
    }

    let wordmark = [
        r"|   | \   /  /-\  |  / |   |",
        r"|   |  \ /  /   \ | /  |   |",
        r"|---|   |   |---| |<   |   |",
        r"|   |   |   |   | | \  |   |",
        r"|   |   |   |   | |  \  \_/ ",
    ];
    let title_area = if wide {
        let hero = Layout::horizontal([Constraint::Percentage(49), Constraint::Percentage(51)])
            .split(chunks[0]);
        render_title_keep(frame, hero[1], stone);
        hero[0].centered_vertically(Constraint::Length(7))
    } else {
        chunks[0]
    };

    let mut title = if large_title {
        wordmark.into_iter().map(|row| Line::styled(row, gold)).collect::<Vec<_>>()
    } else {
        vec![Line::styled("HYAKU", gold)]
    };
    if wide {
        title.extend([
            Line::default(),
            Line::styled("THE HUNDRED FLOORS", Style::default().fg(stone)),
        ]);
    }
    frame.render_widget(Paragraph::new(title).centered(), title_area);

    let name_style = if app.title_edit_name {
        Style::default()
            .fg(Color::Black)
            .bg(TITLE_COLOR_ACCENT)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let cursor = if app.title_edit_name { "_" } else { "" };
    let name = Line::from(vec![
        Span::raw("Name  "),
        Span::styled(format!("{}{cursor}", app.username), name_style),
    ]);
    let name_hint = if app.title_edit_name { "" } else { "[Tab] Edit name" };
    frame.render_widget(
        Paragraph::new(vec![name, Line::styled(name_hint, Style::default().fg(Color::Gray))]).centered(),
        chunks[1],
    );

    let preview = area.width >= 60 && chunks[2].height >= 7;
    let class_chunks = Layout::horizontal([
        Constraint::Percentage(if preview { 60 } else { 100 }),
        Constraint::Min(0),
    ])
    .spacing(if preview { 2 } else { 0 })
    .split(chunks[2]);
    let preset = app.current_class_preset();
    let class_info = vec![
        Line::styled(format!("<  {}  >", preset.name), gold),
        Line::raw(format!("HP {}   MP {}   ATK {}", preset.max_hp, preset.max_mana, preset.attack_power)),
        Line::styled(preset.description, Style::default().fg(Color::Gray)),
    ];
    let panel = tower_panel();
    frame.render_widget(
        Paragraph::new(class_info)
            .centered()
            .wrap(Wrap { trim: true })
            .block(panel.clone().title(" Class ")),
        class_chunks[0],
    );
    if preview {
        let weapon: Vec<_> = CLASS_PREVIEWS[app.class_idx].iter().copied().map(Line::raw).collect();
        frame.render_widget(
            Paragraph::new(weapon)
                .centered()
                .style(gold)
                .block(panel.title(" Weapon ")),
            class_chunks[1],
        );
    }

    let controls = if app.title_edit_name {
        ["[Enter] Confirm  [Esc] Close", ""]
    } else {
        ["[Enter] Climb  [Q/Esc] Quit", "[A/D or arrows] Class"]
    };
    frame.render_widget(Paragraph::new(controls.join("\n")).centered(), chunks[3]);
}

fn render_title_trim(frame: &mut Frame, area: Rect, menu: Rect) {
    let left = area.x;
    let right = area.right() - 1;
    let top = area.y;
    let bottom = area.bottom() - 1;
    let iron = Style::default().fg(Color::Rgb(53, 47, 47));
    let bronze = Style::default().fg(Color::Rgb(127, 99, 73));
    let gold = Style::default().fg(BRASS);
    let tick = frame.count();
    let buf = frame.buffer_mut();

    for x in left + 1..right {
        buf.set_string(x, top, "─", iron);
        buf.set_string(x, bottom, "─", iron);
    }
    for y in top + 1..bottom {
        buf.set_string(left, y, "│", iron);
        buf.set_string(right, y, "│", iron);
    }

    buf.set_string(left, top, "╔═╦═╤═╾─◇─╴", bronze);
    buf.set_string(right - 10, top, "╶─◇─╼═╤═╦═╗", bronze);
    buf.set_string(left, bottom, "╚═╩═╧═╾─◇─╴", bronze);
    buf.set_string(right - 10, bottom, "╶─◇─╼═╧═╩═╝", bronze);
    let center = left + (area.width - 9) / 2;
    buf.set_string(center, top, "─╼◇·◆·◇╾─", bronze);
    buf.set_string(center, bottom, "─╼◇·◆·◇╾─", bronze);

    for (edge, inward, room, offset) in [
        (left, 1, menu.x - left, 0),
        (right, -1, area.right() - menu.right(), 4),
    ] {
        let x = |distance: i16| (i32::from(edge) + i32::from(inward * distance)) as u16;
        let glint = (tick / 6 + offset) % 28;
        if glint < 9 {
            buf.set_string(x(1 + glint as i16), top, "─", gold);
        }
        for y in [top + 1, top + 2, bottom - 1] {
            buf.set_string(edge, y, "║", bronze);
        }

        let flame = ["✦", "♦", "✧", "♦", "✦", "♦"][(tick / 8 + offset) % 6];
        let ember = Style::default().fg(Color::Rgb(218, 126, 67));
        let fire_x = if room >= 6 { x(3) } else { edge };
        buf.set_string(fire_x, bottom - 3, flame, ember);
        buf.set_string(fire_x, bottom - 2, "╨", bronze);
        for delay in [0, 5] {
            let age = (tick / 5 + offset + delay) % 12;
            if age < 7 {
                let drift = if room >= 6 { [0, 1, 0, -1][age % 4] } else { 0 };
                let spark_x = (i32::from(fire_x) + drift) as u16;
                let color = Color::Rgb(193 - age as u8 * 16, 105 - age as u8 * 7, 54);
                buf.set_string(spark_x, bottom - 4 - age as u16, "·", Style::default().fg(color));
            }
        }

        if room >= 6 {
            let sway = [0, 1, 1, 0, -1, -1][(tick / 18 + offset) % 6];
            let chain = match sway * inward {
                -1 => "╱",
                1 => "╲",
                _ => "│",
            };
            buf.set_string(x(2), top + 1, "│", bronze);
            buf.set_string(x(2), top + 2, chain, bronze);
            buf.set_string(x(2 + sway) - 1, top + 3, "[◇]", bronze);
            buf.set_string(x(2 + sway), top + 3, flame, ember);
            buf.set_string(fire_x - 2, bottom - 2, "╲_╨_╱", bronze);
        }
    }
}

fn render_title_keep(frame: &mut Frame, area: Rect, stone: Color) {
    let keep = [
        r"                 /\             .",
        r"                /  \        .-' )",
        r"          /\   /____\       '--'",
        r"         /__\   |[]|        _",
        r"     ____|  |___|  |____   / \ ",
        r"    /    |[]|  .|  |    \_/___\ ",
        r" __/..   |__|___|__|___ |  [] |_",
        r"/_______/|  _   __    |\|_____|_\ ",
        r"    \___/|_/ \_/  \___|  \___/",
        r"          /____|_ _|____\ ",
        r"       __/____/___\____\___",
    ];
    let area = area.centered_horizontally(Constraint::Length(36));
    let scene: Vec<_> = keep.into_iter().map(Line::raw).collect();
    frame.render_widget(Paragraph::new(scene).style(Style::default().fg(stone)), area);

    let tick = frame.count();
    let buf = frame.buffer_mut();

    let drift = (tick / 15) % 20;
    let cloud_x = area.x + 2 + drift.min(20 - drift) as u16;
    buf.set_string(cloud_x, area.y, ".--.", Style::default().fg(Color::Rgb(84, 89, 106)));
    let far_cloud_x = area.x + 1 + ((tick / 24 + 2) % 5) as u16;
    buf.set_string(far_cloud_x, area.y + 2, ".-.", Style::default().fg(Color::Rgb(67, 71, 85)));

    let pennant = ["|-~.", "|~~'", "|--~", "|~.."][(tick / 11) % 4];
    buf.set_string(area.x + 11, area.y + 1, pennant, Style::default().fg(Color::Rgb(145, 96, 80)));

    let (fire, glow) = [(".", 190), ("*", 255), ("+", 221), ("*", 240)][(tick / 7) % 4];
    buf.set_string(area.x + 17, area.y + 9, fire, Style::default().fg(Color::Rgb(glow, 157, 86)));

    let (x, y, glyph, light) = [
        (17, 8, ".", 239),
        (16, 8, "'", 212),
        (18, 7, ".", 164),
        (19, 7, ".", 120),
    ][(tick / 8) % 4];
    buf.set_string(area.x + x, area.y + y, glyph, Style::default().fg(Color::Rgb(light, 106, 61)));
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
        tower_panel().title(" Connecting "),
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
    frame.render_widget(Block::default().style(Style::default().bg(DUSK)), area);
    let boss = app.tick_state.as_ref().filter(|tick| tick.floor == app.current_floor).and_then(|tick| {
        tick.enemies.iter().find(|e| e.is_alive() && e.boss_phase.is_some())
    });
    let boss_bar_height = if boss.is_none() {
        0
    } else if area.height < 16 {
        1
    } else {
        3
    };

    // 4-Pane TUI Command Deck:
    // 1. Top Status Bar: Height 3
    // 2. Main Middle Deck: Left 72% Camera Viewport, Right 28% Sidebar (3 stacked panels)
    // 3. Bottom HUD: Height 4
    let deck_layout = Layout::default()
        .direction(LayoutDirection::Vertical)
        .constraints([
            Constraint::Length(3), // 1. Top Status Bar
            Constraint::Length(boss_bar_height),
            Constraint::Min(if boss.is_some() { 0 } else { 10 }),
            Constraint::Length(4), // 4. Bottom HUD
        ])
        .split(area);

    render_top_status_bar(frame, app, deck_layout[0]);
    if let Some(boss) = boss {
        render_boss_bar(frame, boss, app.class_idx == RANGER_CLASS, app.floor_entered_at, deck_layout[1]);
    }

    let middle_layout = Layout::default()
        .direction(LayoutDirection::Horizontal)
        .constraints([
            Constraint::Percentage(72), // 2. Main Camera Viewport (Left 72%)
            Constraint::Percentage(28), // 3. Sidebar (Right 28%)
        ])
        .split(deck_layout[2]);

    render_camera_viewport(frame, app, middle_layout[0]);
    render_sidebar(frame, app, middle_layout[1]);
    render_bottom_hud(frame, app, deck_layout[3]);

    // Slain adventurer overlay
    if app.my_player().map_or(false, |p| !p.is_alive()) {
        render_death_screen(frame, area);
    }

    // Full-width ServerBanner popup overlay across all connected terminals
    if let Some(ref banner) = app.server_banner {
        render_server_banner(frame, banner, area);
    }
    if app.lore_open {
        render_lore(frame, app, area);
    }
    if app.victory_open {
        render_result(frame, area, true);
    } else if app.respawned_at.is_some()
        && app.my_player().is_some_and(hyaku_protocol::PlayerState::is_alive)
    {
        render_result(frame, area, false);
    }
}

fn render_lore(frame: &mut Frame, app: &AppState, area: Rect) {
    let popup = area.centered(
        Constraint::Length(area.width.min(56)),
        Constraint::Length(area.height.min(26)),
    );
    let panel = tower_panel().title(" Hall of Fame & Lore ").style(Style::default().bg(DUSK));
    let inner = panel.inner(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(panel, popup);

    let crest_height = if inner.height >= 12 { 4 } else { 0 };
    let sections = Layout::vertical([
        Constraint::Length(crest_height),
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(if inner.width >= 36 { 1 } else { 2 }),
    ]).split(inner);
    let crest = sections[0];
    let header = sections[1];
    let ledger = sections[2];
    let footer = sections[3];

    if crest_height > 0 {
        let ember = [".", "*", "+", "*"][(frame.count() / 12) % 4];
        let candles = Line::from(format!(r" {ember}    __/\__    {ember} "));
        let art = vec![
            candles,
            Line::raw(r" |   /__\/__\   | "),
            Line::raw(r" |   \__/ \_/   | "),
        ];
        frame.render_widget(Paragraph::new(art).centered().style(Style::default().fg(BRASS)), crest);
    }

    let highest = app.highest_unlocked_floor;
    frame.render_widget(
        Paragraph::new(format!("Highest floor reached  {highest}/100"))
            .centered()
            .style(Style::default().fg(BRASS)),
        header,
    );

    let height = u32::from(ledger.height);
    let selected = app.lore_floor;
    let first = selected.saturating_sub(height.saturating_sub(1)).max(1);
    let lines: Vec<_> = (first..=highest).take(ledger.height as usize).map(|floor| {
        let name = app.hall_of_fame.get(&floor).map(String::as_str)
            .unwrap_or(if floor == 1 { "Tower entrance" } else { "Unrecorded" });
        let cursor = if floor == selected { '>' } else { ' ' };
        let style = if floor == selected {
            Style::default().fg(BRASS).bg(Color::Rgb(45, 34, 26))
        } else {
            Style::default().fg(Color::Rgb(210, 197, 176))
        };
        Line::styled(format!(" {cursor} F{floor:03}  {name} "), style)
    }).collect();
    frame.render_widget(Paragraph::new(lines), ledger);

    let keys = if inner.width >= 36 {
        "[↑/↓] Browse     [Tab/Esc] Close"
    } else {
        "[↑/↓] Browse
[Tab/Esc] Close"
    };
    frame.render_widget(Paragraph::new(keys).centered().style(Style::default().fg(BRASS)), footer);
}

fn render_boss_bar(frame: &mut Frame, boss: &EnemyState, ranged: bool, entered_at: Option<Instant>, area: Rect) {
    use RaidBoss::*;

    let raid_boss = boss.raid_boss.unwrap_or(AshWarden);
    let (barrage, tremor) = match raid_boss {
        AshWarden => ("ASH SPIRAL", "ASHFALL"),
        BellKeeper => ("DOUBLE TOLL", "FUNERAL PEAL"),
        MireMatron => ("VENOM FAN", "BOG BLOOM"),
        ChainKnight => ("IRON LANES", "CHAIN SWEEP"),
        GlassRegent => ("SPLIT HALO", "SHATTER"),
        HollowHuntsman => ("TRIPLE VOLLEY", "DEADFALL"),
        LanternSaint => ("TWIN FLAMES", "RING OF WICKS"),
        FrostReaver => ("ICE PINCERS", "RIME FIELD"),
        CinderColossus => ("CINDER TIDE", "FAULT LINES"),
        AshSovereign => ("CROWN OF ASH", "RUIN OF THE CROWN"),
    };
    let (phase, color) = match boss.boss_phase {
        Some(BossPhase::Barrage) => (barrage, raid_boss_color(raid_boss)),
        Some(BossPhase::Tremor) => (tremor, Color::LightRed),
        Some(BossPhase::Shielded) => ("SHIELD [Q] TETHER ACROSS", Color::Cyan),
        Some(BossPhase::Exposed) => ("SHIELD DOWN", Color::LightRed),
        None => return,
    };
    let name = boss.name().to_ascii_uppercase();
    let title = format!(" {name} / {phase} ");
    let hint = match boss.boss_phase {
        Some(BossPhase::Shielded) => "Cross the boss with an ally or your solo decoy",
        _ if ranged => "[Space] Shoot within 7 tiles of the body",
        _ => "[Space] Slash within 2 tiles of the body",
    };
    let hp = if area.width >= 62 {
        format!("{} / {}  ·  {hint}", boss.hp, boss.max_hp)
    } else {
        format!("{} / {}", boss.hp, boss.max_hp)
    };
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(color).bg(Color::Rgb(39, 29, 38)))
        .ratio((f64::from(boss.hp) / f64::from(boss.max_hp.max(1))).clamp(0.0, 1.0));
    if area.height >= 3 {
        let reveal = entered_at.map(|at| at.elapsed().as_millis()).filter(|&age| {
            age < 1800 && usize::from(area.width) >= title.len() + 14
        });
        let title = if let Some(age) = reveal {
            let sigil = match raid_boss {
                AshWarden => "△",
                BellKeeper => "Ω",
                MireMatron => "≈",
                ChainKnight => "∞",
                GlassRegent => "◇",
                HollowHuntsman => "»",
                LanternSaint => "✦",
                FrostReaver => "✧",
                CinderColossus => "▲",
                AshSovereign => "♜",
            };
            let (left, right) = [("·", "·"), ("─·", "·─"), ("╼─·", "·─╾")][(age / 180).min(2) as usize];
            let crest = Style::default().fg(raid_boss_color(raid_boss));
            Line::from(vec![
                Span::styled(format!(" {left} {sigil}"), crest),
                Span::raw(title),
                Span::styled(format!("{sigil} {right} "), crest),
            ]).centered()
        } else {
            Line::raw(title)
        };
        let border = Block::bordered().border_style(Style::default().fg(color)).title(title);
        frame.render_widget(
            gauge.block(border).label(hp),
            area,
        );
    } else {
        let label = format!("{name} {phase} {}/{}", boss.hp, boss.max_hp);
        frame.render_widget(gauge.label(label), area);
    }
}

fn raid_boss_color(boss: RaidBoss) -> Color {
    use RaidBoss::*;

    match boss {
        AshWarden => Color::LightRed,
        BellKeeper => Color::Rgb(211, 169, 102),
        MireMatron => Color::Rgb(139, 182, 88),
        ChainKnight => Color::Rgb(176, 185, 195),
        GlassRegent => Color::Rgb(213, 177, 239),
        HollowHuntsman => Color::Rgb(190, 137, 108),
        LanternSaint => Color::Rgb(250, 210, 115),
        FrostReaver => Color::Rgb(148, 205, 245),
        CinderColossus => Color::Rgb(241, 129, 75),
        AshSovereign => Color::Rgb(255, 194, 121),
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

    let mut shard_ratio = if shrine_goal == 0 {
        if stairs_unlocked { 1.0 } else { 0.0 }
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

    let top_block = tower_panel();
    let inner = top_block.inner(area);
    frame.render_widget(top_block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }
    if let Some(age) = app.hud_glow_at.map(|at| at.elapsed().as_millis()).filter(|age| *age < 900) {
        let center = i32::from(area.x + area.width / 2);
        let wave = (age as i32 * i32::from(area.width) / 1800).max(1);
        let buffer = frame.buffer_mut();
        for x in inner.x..inner.right() {
            let distance = ((i32::from(x) - center).abs() - wave).abs();
            if distance <= 3 {
                let glow = if distance <= 1 { BRASS } else { Color::Rgb(178, 111, 63) };
                buffer.set_string(x, area.y, "─", Style::default().fg(glow));
            }
        }
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
            " ◇ ",
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

    let raid = RaidBoss::for_floor(app.current_floor).is_some();
    let gauge_label = if raid && is_gate_locked {
        let held = app.tick_state.as_ref().map_or(0, |tick| {
            tick.tiles_override.iter().filter(|(_, tile)| {
                matches!(tile, Tile::PressurePlate { active: true, .. })
            }).count()
        });
        let total = app.current_map.as_ref().map_or(2, |map| map.plates.len()).max(1);
        shard_ratio = (held as f64 / total as f64).min(1.0);
        format!("Plates {held}/{total}")
    } else if raid {
        if stairs_unlocked { "Raid cleared".to_string() } else { "Slay the boss".to_string() }
    } else if shrine_goal == 0 || stairs_unlocked {
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
            if !raid && !stairs_unlocked && !is_gate_locked {
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
        FRAME_STONE
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

    let biome = palette.name.split_once(": ").map_or(palette.name, |(_, name)| name);
    let map_block = tower_panel()
        .title(format!(" F{} / {} {} ", app.current_floor, biome, title_badge))
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
    let mut campfire_positions: Vec<Position> = app
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
    if let Some(map) = &app.current_map {
        let width = map.width as usize;
        campfire_positions.extend(map.tiles.iter().enumerate().filter_map(|(index, tile)| {
            (*tile == Tile::Campfire).then(|| Position::new((index % width) as i32, (index / width) as i32))
        }));
    }

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

        if let Some(map) = &app.current_map {
            for enemy in tick.enemies.iter().filter(|enemy| enemy.is_alive()) {
                let (Some(boss), Some(phase)) = (enemy.raid_boss, enemy.boss_phase) else { continue };
                let wings = raid_wings(boss, phase, tick.tick);
                let (half_width, half_height) = boss.half_size();
                let shade = match phase {
                    BossPhase::Shielded => Color::Rgb(85, 156, 168),
                    BossPhase::Exposed => Color::Rgb(154, 106, 83),
                    _ => Color::Rgb(204, 160, 107),
                };
                for (row, line) in wings.iter().enumerate() {
                    let dy = row as i32 - half_height;
                    for (col, glyph) in line.chars().rev().enumerate().filter(|(_, ch)| *ch != ' ') {
                        let dx = half_width + 1 + col as i32;
                        for (dx, glyph) in [(-dx, glyph), (dx, mirror_wing(glyph))] {
                            let pos = enemy.pos.add_offset(dx, dy);
                            let (x, y) = app.world_to_screen(pos);
                            if (0..inner_w).contains(&x) && (0..inner_h).contains(&y)
                                && map.get_tile(pos) == Some(Tile::Floor)
                                && !tick.tiles_override.iter().any(|(p, _)| *p == pos)
                            {
                                let cell = &mut grid[y as usize][x as usize];
                                *cell = (glyph, cell.1.fg(shade));
                            }
                        }
                    }
                }
            }
        }

        for carving in &tick.carvings {
            let (x, y) = app.world_to_screen(carving.pos);
            if (0..inner_w).contains(&x) && (0..inner_h).contains(&y) {
                let glow = if (tick_counter / 12) % 2 == 0 { BRASS } else { Color::Rgb(170, 131, 89) };
                grid[y as usize][x as usize] = (
                    '✉',
                    Style::default().fg(glow).bg(Color::Rgb(45, 34, 23)),
                );
            }
        }

        app.particles.draw(Instant::now(), app.camera_offset, &mut grid);

        // 4. Render projectiles (directional melee slash arcs, arrows, aether shards, ether bolts)
        // Sort so slash arcs ('╱', '─', '│', '╲') render with priority over ground shard particles
        let danger = |glyph| matches!(glyph, '░' | '▒' | '▓' | '█');
        let mut sorted_projs: Vec<_> = tick.projectiles.iter().filter(|p| !danger(p.glyph)).collect();
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
                let remaining_secs = (u64::from(decoy.remaining_ticks) * hyaku_protocol::SERVER_TICK_MS).div_ceil(1000);
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
            if let Some(phase) = enemy.boss_phase {
                let boss = enemy.raid_boss.unwrap_or(RaidBoss::AshWarden);
                let color = if phase == BossPhase::Shielded { Color::Cyan } else { raid_boss_color(boss) };
                let style = Style::default().fg(color).bg(Color::Rgb(38, 24, 33)).add_modifier(Modifier::BOLD);
                let sprite = if enemy.raid_boss.is_some() {
                    raid_sprite(boss, tick.tick)
                } else {
                    &[r"/^^^\", "|o#o|", r"\_V_/"]
                };
                let half_width = sprite[0].len() as i32 / 2;
                let half_height = sprite.len() as i32 / 2;
                for (row, line) in sprite.iter().enumerate() {
                    let y = sy + row as i32 - half_height;
                    for (col, glyph) in line.chars().enumerate() {
                        let x = sx + col as i32 - half_width;
                        if (0..inner_w).contains(&x) && (0..inner_h).contains(&y) {
                            let ink = if matches!(glyph, 'o' | '*') && phase != BossPhase::Shielded {
                                style.fg(Color::Rgb(255, 231, 174))
                            } else {
                                style
                            };
                            grid[y as usize][x as usize] = (glyph, ink);
                        }
                    }
                }
                continue;
            }
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
                    if p.is_alive() && p.pos.manhattan_distance(target_pos) <= hyaku_protocol::tether_range(tick.floor) {
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
        let raid_in_progress = tick.enemies.iter().any(|enemy| enemy.raid_boss.is_some() && enemy.is_alive());
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

                if !(is_me && raid_in_progress) && tag_y >= 0 && tag_y < inner_h {
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

        if let Some(me) = app.my_player().filter(|p| p.is_alive()) {
            let mut has_target = false;
            for tile in app.attack_tiles() {
                if !tick.enemies.iter().any(|enemy| enemy.is_alive() && enemy.occupies(tile.pos)) {
                    continue;
                }
                has_target = true;
                let (x, y) = app.world_to_screen(tile.pos);
                if (0..inner_w).contains(&x) && (0..inner_h).contains(&y) {
                    let cell = &mut grid[y as usize][x as usize];
                    cell.1 = cell.1.bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD);
                }
            }

            if !has_target {
                let (x, y) = app.world_to_screen(me.pos.step(app.last_dir));
                if (0..inner_w).contains(&x) && (0..inner_h).contains(&y) {
                    let cell = &mut grid[y as usize][x as usize];
                    if !matches!(cell.0, '#' | '+' | '▲' | '♨' | '☼' | '✉') {
                        *cell = (
                            app.last_dir.arrow_glyph(),
                            Style::default().fg(Color::Black).bg(BRASS).add_modifier(Modifier::BOLD),
                        );
                    }
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
        for warning in tick.projectiles.iter().filter(|p| danger(p.glyph)) {
            let (x, y) = app.world_to_screen(warning.pos);
            if (0..inner_w).contains(&x) && (0..inner_h).contains(&y) {
                let cell = &mut grid[y as usize][x as usize];
                let (r, g, b) = warning.color_rgb;
                if tick.players.iter().any(|p| p.pos == warning.pos) {
                    cell.1 = cell.1.fg(Color::White).bg(Color::Rgb(r / 2, g / 4, b / 4));
                } else {
                    *cell = (warning.glyph, Style::default().fg(Color::Rgb(r, g, b)).bg(Color::Rgb(48, 20, 22)));
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
    if let Some(carving) = app.nearby_carving().filter(|_| inner_area.height >= 4) {
        let slip = Rect::new(inner_area.x, inner_area.bottom() - 3, inner_area.width, 3);
        let text = vec![
            Line::styled(
                format!("[L] Praise  ·  @{}  ·  {}", carving.author, carving.upvotes),
                Style::default().fg(BRASS).add_modifier(Modifier::BOLD),
            ),
            Line::raw(&carving.message),
        ];
        frame.render_widget(Clear, slip);
        frame.render_widget(
            Paragraph::new(text).wrap(Wrap { trim: true })
                .style(Style::default().fg(Color::Rgb(225, 209, 186)).bg(Color::Rgb(44, 33, 25))),
            slip,
        );
    }
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
                        '·',
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
                    2..=5 => ('·', palette.floor_primary),
                    _ => (' ', palette.floor_primary),
                };
                (ch, Style::default().fg(col).bg(DUSK))
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
                .fg(if (tick / 8) % 6 == 0 { Color::Rgb(238, 188, 115) } else { BRASS })
                .bg(Color::Rgb(55, 35, 20))
                .add_modifier(Modifier::BOLD),
        ),
        Tile::Campfire => {
            let ember = [Color::Rgb(227, 126, 66), BRASS, Color::Rgb(249, 193, 119)]
                [(tick as usize / 7) % 3];
            (
                '♨',
                Style::default().fg(ember).bg(Color::Rgb(54, 26, 18)).add_modifier(Modifier::BOLD),
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
    let block = tower_panel().title(" Radar ");
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
    let block = tower_panel().title(" Nearby Climbers ");
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
    let block = tower_panel()
        .title(" Tower Comms ")
        .border_style(match app.input_mode {
            InputMode::Chatting => Style::default().fg(TITLE_COLOR_ACCENT),
            InputMode::CarvingMessage => Style::default().fg(Color::LightYellow),
            InputMode::Normal => Style::default().fg(FRAME_STONE),
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
                        format!("[L] Praise @{} | ", carving.author),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "[Enter] Chat | [M] Carve",
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            } else {
                Line::from(Span::styled(
                    "[Enter] Chat | [M] Carve",
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
    let block = tower_panel().title(" Climber ");
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
    let shards_line = Line::from(vec![
        Span::raw("Shards "),
        Span::styled(
            format!("{}", my_shards),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        Span::styled(my_player.map_or_else(String::new, |p| {
            format!("  Gear +{}  ATK {}", p.temper, p.attack_power())
        }), Style::default().fg(TITLE_COLOR_ACCENT)),
        Span::raw(format!("  {}  Bank {tower_bank}", app.last_dir.arrow_glyph())),
    ]);
    frame.render_widget(Paragraph::new(shards_line), cols[2]);

    let raid = RaidBoss::for_floor(app.current_floor).is_some();
    let attack = if app.class_idx == RANGER_CLASS { "Shoot" } else { "Slash" };
    let keys = match (raid, inner.width >= 77) {
        (true, true) => "[Q] Link  [R] Decoy  [G] Temper at fire  [E] Use  [Tab] Lore",
        (true, false) => "[Q] Link  [R] Decoy  [G] Temper",
        (false, true) => "[E] Use  [G] Temper  [C] Fire  [M] Carve  [L] Praise  [Tab] Lore",
        (false, false) => "[E] Use  [G] Temper  [Tab] Lore",
    };
    frame.render_widget(
        Paragraph::new(format!("[Space] {attack}  {keys}")).alignment(Alignment::Center).style(Style::default().fg(BRASS)),
        rows[1],
    );
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

fn render_result(frame: &mut Frame, area: Rect, victory: bool) {
    let wide = area.width >= 42 && area.height >= 16;
    let popup = area.centered(
        Constraint::Length(area.width.min(60)),
        Constraint::Length(if wide { 15 } else { 5 }),
    );
    let title = if victory { " THE SOVEREIGN HAS FALLEN " } else { " THE HEARTH REMEMBERS " };
    let panel = tower_panel()
        .title(title)
        .style(Style::default().bg(Color::Rgb(29, 23, 24)));
    let inner = panel.inner(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(panel, popup);

    let spark = [".", "*", "+", "*"][(frame.count() / 12) % 4];
    let mut text = Vec::new();
    if wide {
        let art: &[&str] = if victory {
            &[
                r"                      /\                ",
                r"                _    /  \    _          ",
                r"               / \__/____\__/ \         ",
                r"           ___/___|  |[]|  |___\___     ",
                r"          /  . |[]|__|  |__|[]| .  \    ",
                r"       __/_____|__| /____\ |__|_____\__ ",
                r"      /__/___.-----'      '----.___\__\ ",
                r"       _.._           __          _.._  ",
            ]
        } else {
            &[
                r"                  .      .              ",
                r"                    . /                 ",
                r"                   /\\/  .              ",
                r"                  /   \\/               ",
                r"                _/ /\\  \_              ",
                r"              _/__/  \___\_             ",
                r"             /_____====_____\           ",
                r"             \_____\  /_____/           ",
            ]
        };
        text.push(Line::styled(spark, Style::default().fg(BRASS)));
        text.extend(art.iter().copied().map(|row| Line::styled(row, Style::default().fg(BRASS))));
        text.push(Line::default());
    }
    if victory {
        text.push(Line::raw("The hundredth floor is yours."));
        text.push(Line::styled("[Enter] Stay  [Esc] Quit", Style::default().fg(BRASS)));
    } else {
        text.push(Line::raw("The ember held."));
    }
    frame.render_widget(Paragraph::new(text).centered().style(Style::default().fg(Color::Rgb(223, 210, 191))), inner);
}

pub(crate) const RESPAWN_OVERLAY: Duration = Duration::from_millis(1400);

fn render_death_screen(frame: &mut Frame, area: Rect) {
    let art = area.width >= 38 && area.height >= 16;
    let popup = area.centered(
        Constraint::Length(area.width.min(54)),
        Constraint::Length(if art { 15 } else { 5 }),
    );
    frame.render_widget(Clear, popup);

    let ash = Color::Rgb(175, 158, 146);
    let ember = Color::Rgb(232, 116, 92);
    let mut lines = vec![Line::styled(
        "YOU HAVE FALLEN",
        Style::default().fg(ember).add_modifier(Modifier::BOLD),
    )];
    if art {
        let (spark_top, spark_low) = [(" ", "."), ("'", " "), ("*", " "), (" ", ".")]
            [(frame.count() / 12) % 4];
        let spark = Style::default().fg(ember);
        lines.extend([
            Line::default(),
            Line::raw(r"           /\              "),
            Line::raw(r"          /  \             "),
            Line::raw(r"          |  /             "),
            Line::from(vec![
                Span::raw("          |/'      "),
                Span::styled(spark_top, spark),
                Span::raw("       "),
            ]),
            Line::from(vec![
                Span::raw("              /|   "),
                Span::styled(spark_low, spark),
                Span::raw("       "),
            ]),
            Line::raw(r"        _____/ |_____      "),
            Line::raw(r"        '--.__   __.--'    "),
            Line::raw(r"             \_/           "),
            Line::default(),
        ]);
    }
    lines.extend([
        Line::styled("[R] Respawn", Style::default().fg(Color::White)),
        Line::styled("[Q/Esc] Quit", Style::default().fg(ash)),
    ]);

    let block = Block::bordered()
        .border_style(Style::default().fg(Color::Rgb(142, 81, 75)))
        .style(Style::default().fg(ash).bg(Color::Rgb(29, 19, 21)));
    frame.render_widget(Paragraph::new(lines).centered().block(block), popup);
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

#[cfg(test)]
mod tests {
    use super::*;
    use hyaku_protocol::{EnemyKind, FloorMap, PlayerState, ProjectileState, TickState};
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn title_and_death_art_animate_without_clipping_controls() {
        let app = AppState::new("Alice".into(), "ws://localhost".into(), 0);
        for (width, height) in [(80, 24), (30, 12)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let render_title = |frame: &mut Frame| render_title_screen(frame, &app);
            terminal.draw(render_title).unwrap();
            let read = |terminal: &Terminal<TestBackend>| -> String {
                terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect()
            };
            let title = read(&terminal);
            assert_eq!(terminal.backend().buffer()[(0, 0)].bg, DUSK);
            assert!(title.contains("[Enter] Climb") && title.contains("[Q/Esc] Quit"));
            assert!(title.contains("Blade") && title.contains("Alice"));
            if width == 80 {
                assert!(title.contains(r"__________/\"));
                for _ in 0..16 {
                    terminal.draw(render_title).unwrap();
                }
                assert!(read(&terminal).contains("/____|_+_|____"));
                for _ in 0..16 {
                    terminal.draw(render_title).unwrap();
                }
                let animated = read(&terminal);
                let sky = width as usize..2 * width as usize;
                assert_ne!(&animated[sky.clone()], &title[sky]);
                for (idx, (before, after)) in title.bytes().zip(animated.bytes()).enumerate() {
                    if before != after {
                        assert!(idx / (width as usize) < 12 && idx % width as usize >= 39);
                    }
                }
            } else {
                assert!(title.contains("HYAKU"));
            }

            let render_death = |frame: &mut Frame| render_death_screen(frame, frame.area());
            terminal.draw(render_death).unwrap();
            let death = read(&terminal);
            assert!(death.contains("YOU HAVE FALLEN") && death.contains("[R] Respawn"));
            assert!(death.contains("[Q/Esc] Quit"));
            if width == 80 {
                for _ in 0..12 {
                    terminal.draw(render_death).unwrap();
                }
                assert_ne!(read(&terminal), death);
            }
        }
    }

    #[test]
    fn lore_opens_scrolls_and_animates() {
        let mut app = AppState::new("Alice".into(), "ws://localhost".into(), 0);
        app.screen = Screen::Connected;
        app.handle_server_packet(hyaku_protocol::ServerPacket::HallOfFame {
            highest_unlocked_floor: 3,
            hall_of_fame: [(2, "Alice".into()), (3, "Bob".into())].into(),
        });
        app.handle_key(crossterm::event::KeyCode::Tab);
        assert!(app.lore_open);
        assert_eq!(app.lore_floor, 3);
        app.handle_key(crossterm::event::KeyCode::Up);
        assert_eq!(app.lore_floor, 2);

        for (width, height) in [(80, 24), (30, 12)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let draw = |frame: &mut Frame| render_lore(frame, &app, frame.area());
            terminal.draw(draw).unwrap();
            let read = |terminal: &Terminal<TestBackend>| {
                terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect::<String>()
            };
            let still = read(&terminal);
            assert!(still.contains("Hall of Fame & Lore"));
            assert!(still.contains("> F002  Alice") && still.contains("F003  Bob"));
            assert!(still.contains("[Tab/Esc] Close"));
            if width == 80 {
                for _ in 0..12 {
                    terminal.draw(draw).unwrap();
                }
                assert_ne!(read(&terminal), still);
            }
        }

        app.handle_key(crossterm::event::KeyCode::Tab);
        assert!(!app.lore_open);
        assert!(!app.should_quit);
    }

    #[test]
    fn floor_snapshots_emit_bursts_and_particles_expire() {
        let mut app = AppState::new("Alice".into(), "ws://localhost".into(), 0);
        app.screen = Screen::Connected;
        app.current_floor = 2;
        let map = FloorMap::generate(42, 2);
        let shrine = map.shrine_pos.unwrap();
        let origin = shrine.add_offset(5, 0);
        app.current_map = Some(map);
        let mut tick = TickState {
            tick: 1,
            floor: 2,
            players: vec![PlayerState::new("p1".into(), "Alice".into(), 0, origin, 2)],
            enemies: vec![EnemyState::new(1, EnemyKind::Crawler, origin, 1.0)],
            projectiles: Vec::new(),
            tiles_override: Vec::new(),
            carvings: Vec::new(),
            tower_shards_bank: 0,
            decoys: Vec::new(),
            shrine_shards: 9,
            shrine_goal: 10,
            stairs_unlocked: false,
        };
        app.handle_server_packet(hyaku_protocol::ServerPacket::Tick(tick.clone()));
        assert!(app.particles.bursts.is_empty());
        tick.tick += 1;
        tick.enemies.clear();
        tick.players[0].shards += 2;
        tick.shrine_shards = 10;
        tick.stairs_unlocked = true;
        app.handle_server_packet(hyaku_protocol::ServerPacket::Tick(tick.clone()));
        app.handle_server_packet(hyaku_protocol::ServerPacket::Tick(tick));
        assert!(matches!(app.particles.bursts.as_slice(),
            [Burst { kind: BurstKind::Shards, .. }, Burst { kind: BurstKind::Shrine, .. },
             Burst { kind: BurstKind::Enemy, .. }]
        ));

        let now = Instant::now();
        let camera = (shrine.x - 3, shrine.y - 4);
        let mut grid = vec![vec![(' ', Style::default()); 13]; 9];
        grid[4][3].0 = '☼';
        app.particles.draw(now + Duration::from_millis(250), camera, &mut grid);
        assert!(grid.iter().flatten().filter(|cell| matches!(cell.0, '*' | '+' | '.')).count() > 4);
        assert_eq!(grid[4][3].0, '☼');
        app.particles.expire(now + Duration::from_secs(1));
        assert!(app.particles.bursts.is_empty());
    }

    #[test]
    fn summit_and_respawn_banners_fit_wide_and_compact_terminals() {
        for (width, height) in [(80, 24), (30, 12)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            for (victory, title) in [(true, "THE SOVEREIGN HAS FALLEN"), (false, "THE HEARTH REMEMBERS")] {
                terminal.draw(|frame| render_result(frame, frame.area(), victory)).unwrap();
                let text: String = terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect();
                assert!(text.contains(title));
                assert!(text.contains(if victory { "[Enter] Stay" } else { "The ember held." }));
                if width == 80 {
                    assert!(text.contains(if victory { "|[]|" } else { "====" }));
                }
            }
        }
    }

    #[test]
    fn raid_sprites_animate_inside_their_hitboxes() {
        let mut sprites = std::collections::HashSet::new();
        for floor in (10..=100).step_by(10) {
            let boss = RaidBoss::for_floor(floor).unwrap();
            let (half_width, half_height) = boss.half_size();
            for tick in [0, 6, 12] {
                let sprite = raid_sprite(boss, tick);
                assert!(sprites.insert(sprite));
                assert_eq!(sprite.len() as i32, 2 * half_height + 1);
                assert!(sprite.iter().all(|row| row.is_ascii() && row.len() as i32 == 2 * half_width + 1), "{boss:?}: {sprite:?}");
            }
        }
    }

    #[test]
    fn raid_screen_draws_the_shield_cue_sprite_and_warning() {
        let map = FloorMap::generate(42, 10);
        let mut boss = map.initial_enemies.iter().find(|e| e.kind == EnemyKind::FloorBoss)
            .unwrap().clone();
        boss.boss_phase = Some(BossPhase::Shielded);
        let pos = Position::new(boss.pos.x, boss.pos.y + 4);
        let warning = Position::new(pos.x + 4, pos.y);
        let mut app = AppState::new("Alice".into(), "ws://localhost".into(), 0);
        app.current_floor = 10;
        app.current_map = Some(map);
        app.my_player_id = Some("p1".into());
        app.tick_state = Some(TickState {
            tick: 1,
            floor: 10,
            players: vec![PlayerState::new("p1".into(), "Alice".into(), 0, pos, 10)],
            enemies: vec![boss],
            projectiles: vec![ProjectileState::new(warning, '▓', (255, 101, 66))],
            tiles_override: Vec::new(),
            carvings: Vec::new(),
            tower_shards_bank: 0,
            decoys: Vec::new(),
            shrine_shards: 0,
            shrine_goal: 0,
            stairs_unlocked: false,
        });

        for (width, height) in [(80, 24), (48, 14)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| render_game_screen(frame, &mut app)).unwrap();
            let corner = &terminal.backend().buffer()[(0, 0)];
            assert_eq!((corner.fg, corner.bg), (FRAME_STONE, DUSK));
            let screen: String = terminal.backend().buffer().content().iter().map(|cell| cell.symbol()).collect();
            assert!(screen.contains("SHIELD [Q] TETHER ACROSS"), "{screen}");
            if height == 24 {
                assert!(screen.contains("o<|#|>o"), "{screen}");
                assert!(screen.contains('▓'), "{screen}");
            } else {
                assert!(screen.contains("788/788"), "{screen}");
            }
        }
    }
}
