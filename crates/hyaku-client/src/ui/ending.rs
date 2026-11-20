use super::{BRASS, DUSK};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
    Frame,
};
use std::time::Duration;

const BEAT: Duration = Duration::from_secs(5);
pub(crate) const ENDING_DURATION: Duration = BEAT.saturating_mul(3);

const SCENES: [(&str, &str, &[&str]); 3] = [
    (
        "THE SUMMIT",
        "Behind the Sovereign's seat, the copper axle turns still.\nYou let the chain run through your hands. The upper sluice opens.",
        &[
            r"      ________               ________",
            r"     /________\_____________/________\",
            r"     |    |       .---.       |    |",
            r"     |    |======(  @  )======|    |",
            r"     |    |       '---'       |    |",
            r"     |____|_________o_________|____|",
            r"      \_____________o_____________/",
            r"                    o",
        ],
    ),
    (
        "THE RIVER BELOW",
        "The river takes its old course, past the tenth landing and through the broken weir.\nFor the first time since winter, water fills the millrace.",
        &[
            r"      ___________            ___________",
            r"     /           \__________/           \",
            r"    _|___________|          |___________|_",
            r"    \____________|          |____________/",
            r"      ___________|          |___________",
            r"     /___________|          |___________\",
            r"               _/            \_",
            r"       _______/                \_______",
        ],
    ),
    (
        "A LIGHT FOR OREN",
        "The sound of water wakes Ione. Before dawn, she sets a lamp in the window.\nThere is a place at her table for whoever comes down the road.",
        &[
            r"               .-'''-.",
            r"             .'       '.",
            r"  __________/___________\_______________",
            r"        /\                        /\",
            r"     __/  \__                  __/  \__",
            r"     |  +   |        /\        |  []  |",
            r"  ___|___ __|_______/__\_______|___ __|___",
            r" ~~~~~~~~~~~\           /~~~~~~~~~~~~~~~~~",
        ],
    ),
];

const SMALL_ART: [[&str; 3]; 3] = [
    ["  .-----.  ", "==(  @  )==", "  '---o-'  "],
    [r" __\   /__ ", r" __|   |__ ", r"   /   \   "],
    [r"    /\     ", r"  _/  \_   ", r"__| +  |___"],
];

pub(super) fn render(frame: &mut Frame, elapsed: Duration) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(DUSK)), area);
    let scene = (elapsed.as_secs() / BEAT.as_secs()).min(2) as usize;
    let time = (elapsed.as_millis() % BEAT.as_millis()) as u32;
    let (title, caption, full_art) = SCENES[scene];
    let art: &[&str] = if area.width >= 44 && area.height >= 15 {
        full_art
    } else if area.width >= 22 && area.height >= 11 {
        &SMALL_ART[scene]
    } else {
        &[]
    };
    let center = area
        .centered_horizontally(Constraint::Length(area.width.min(70)))
        .centered_vertically(Constraint::Length(if art.len() > 3 { 15 } else { 12 }));
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(art.len() as u16),
        Constraint::Min(1),
        Constraint::Length(1),
    ]).split(center);

    let gold = Style::default().fg(BRASS);
    let mist = (time / 25).min(58) as u8;
    let stone = Style::default().fg(Color::Rgb(133 + mist, 111 + mist, 98 + mist));
    frame.render_widget(
        Paragraph::new(title).centered().style(gold.add_modifier(Modifier::BOLD)),
        rows[0],
    );
    if !art.is_empty() {
        let lines: Vec<_> = art.iter().enumerate().map(|(y, line)| {
            let spans: Vec<_> = line.chars().enumerate().map(|(x, glyph)| match (scene, glyph) {
                (0, '@') => Span::styled(["|", "/", "-", "\\"][(time as usize / 105) % 4], gold),
                (0, 'o') if time as usize / 190 > y.saturating_sub(5) => Span::styled(":", gold),
                (1, ' ') if (if art.len() == 3 { 4..7 } else { 18..26 }).contains(&x)
                    && time as usize / 115 > y
                    && (x + 2 * y + time as usize / 90) % 7 < 3 =>
                {
                    Span::styled("|", Style::default().fg(Color::Rgb(153, 196, 207)))
                }
                (2, '+') => Span::styled(if time / 165 % 3 == 0 { "*" } else { "+" }, gold),
                (2, '.' | '\'') if y <= 1 => Span::styled(glyph.to_string(), gold),
                _ => Span::styled(glyph.to_string(), stone),
            }).collect();
            Line::from(spans)
        }).collect();
        frame.render_widget(Paragraph::new(lines).centered(), rows[1]);
    }
    frame.render_widget(
        Paragraph::new(caption).centered().wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::Rgb(225, 213, 198))),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new("[Enter / Esc] Skip").centered().style(gold),
        rows[3],
    );
}
