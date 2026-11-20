use super::{BRASS, DUSK};
use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Paragraph, Widget, Wrap},
    Frame,
};
use std::time::{Duration, Instant};

const TURN_TIME: Duration = Duration::from_millis(850);
const CLOSE_TIME: Duration = Duration::from_millis(610);
const FALL_TIME: Duration = Duration::from_millis(590);
const SETTLE_TIME: Duration = Duration::from_millis(310);
const PAPER: Color = Color::Rgb(188, 155, 110);
const INK: Color = Color::Rgb(36, 25, 23);
const LEATHER: Color = Color::Rgb(63, 36, 35);
const EDGE: Color = Color::Rgb(119, 88, 60);
const SHADOW: Color = Color::Rgb(37, 27, 25);

struct Entry {
    title: &'static str,
    text: &'static str,
    small: &'static str,
    engraving: &'static [&'static str],
}

const ENTRIES: &[Entry] = &[
    Entry {
        title: "THE DROWNED MILL",
        text: "In the winter of bare granaries, the Ash Sovereign laid claim to our copper mill. His men bore it away by barge. When they ran aground, he gave the word to open the upper sluice.\n\nThe flood took the crew and broke the mill. Come dawn, I found copper still warm in the mud beside my father's boots.",
        small: "The Sovereign took the mill that fed three valleys. At his word the sluice opened. The river gave us back warm copper.",
        engraving: &[
            r"          .-----.",
            r"        .'  .    '.",
            r"       / \  |  /   \",
            r"    .-[---\ o /-----]-.",
            r"    |  \   /|\    /   |",
            r"    '---'./ | \ .'----'",
            r"       ___'---'___",
            r"    ~~~\_.___._/~~~",
            r" ~~~~~~.~~~~.~~~~~~~~~",
        ],
    },
    Entry {
        title: "THE KEEPERS' TOLL",
        text: "I laid a shard upon a cold shrine. Water stirred behind the stone, and the stair rose. At the summit the river still turned the old axle for the Sovereign.\n\nHe set a keeper at every tenth landing. We paid our way to him in pieces of the mill he took.",
        small: "I laid a shard upon the stone, and the stair rose. The Sovereign made it his toll. A keeper stands at every tenth landing.",
        engraving: &[
            r"         .-===-.",
            r"       .'  /\   '.",
            r"      /   /__\    \",
            r"     |    |[]|     |",
            r"     |   _|\/|_    |",
            r"   __|__/ |__| \___|__",
            r"  /____/________\_____\\",
            r"  |  /____________\  |",
            r"  |_/______________\_|",
        ],
    },
    Entry {
        title: "OREN'S BOW",
        text: "At the tenth landing, the Ash Warden spoke with our father's voice. My brother Oren went up at the sound of it.\n\nHis bow lay upon the stair. The string was warm yet. I bore it down and did not call his name.",
        small: "The Ash Warden called in our father's voice. Oren went up. I bore his bow down and did not call his name.",
        engraving: &[
            r"             ._",
            r"             | \",
            r"             |  \.",
            r"     ___     |   |",
            r"  __/===\|---+--->",
            r"             |   |",
            r"             |  /'",
            r"             | /",
            r"             ''",
        ],
    },
    Entry {
        title: "A WATCH AT THE FIRE",
        text: "Rest at this hearth. I have drawn the bench clear of the door. Set your weight upon the south plate, and the first gate shall open.\n\nShould you return, leave word of what lies above the Warden. There is room in the margin.\n\n- Ione, first watch",
        small: "Rest at this hearth. Stand upon the south plate to open the first gate. Should you return, leave word. - Ione",
        engraving: &[
            r"    __________________",
            r"    | || || || || || |",
            r"    | || || || || || |",
            r"    |_|__||_||__||_|_|",
            r"      \    .       /",
            r"       \  / )     /",
            r"       _\/ (/\___/_",
            r"      /__\___/_____\\",
            r"         _===_",
        ],
    },
];

pub(crate) struct Prologue {
    page: usize,
    previous: Option<usize>,
    turned_at: Instant,
    closing_at: Option<Instant>,
    landed: bool,
}

impl Prologue {
    pub(crate) fn new() -> Self {
        Self { page: 0, previous: None, turned_at: Instant::now(), closing_at: None, landed: false }
    }

    pub(crate) fn read_key(&mut self, key: KeyCode) -> bool {
        if self.closing_at.is_some() || self.turned_at.elapsed() < TURN_TIME {
            return false;
        }
        let next = match key {
            KeyCode::Left if self.page > 0 => self.page - 1,
            KeyCode::Enter | KeyCode::Right | KeyCode::Char(' ') if self.page + 1 < ENTRIES.len() => self.page + 1,
            KeyCode::Enter | KeyCode::Right | KeyCode::Char(' ') => {
                self.closing_at = Some(Instant::now());
                return true;
            }
            _ => return false,
        };
        self.previous = Some(self.page);
        self.page = next;
        self.turned_at = Instant::now();
        true
    }

    pub(crate) fn take_landing(&mut self) -> bool {
        if self.landed || !self.closing_at.is_some_and(|start| start.elapsed() >= CLOSE_TIME + FALL_TIME) {
            return false;
        }
        self.landed = true;
        true
    }

    pub(crate) fn finished(&self) -> bool {
        self.closing_at.is_some_and(|start| start.elapsed() >= CLOSE_TIME + FALL_TIME + SETTLE_TIME)
    }

    pub(crate) fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        frame.render_widget(Block::default().style(Style::default().bg(DUSK)), area);
        if area.width < 18 || area.height < 8 {
            frame.render_widget(Paragraph::new("[Esc] Enter tower").centered(), area);
            return;
        }

        let wide = area.width >= 64 && area.height >= 19;
        let leaf_width = if wide { (area.width.min(78) - 4) / 2 } else { area.width.min(52) - 4 };
        let book_width = leaf_width * if wide { 2 } else { 1 };
        let height = (area.height - 5).min(20);
        let book = Rect::new(
            area.x + (area.width - book_width) / 2,
            area.y + (area.height - height) / 2,
            book_width,
            height,
        );
        if let Some(start) = self.closing_at {
            self.render_close(frame, book, area, leaf_width, wide, start.elapsed());
            return;
        }
        let progress = (self.turned_at.elapsed().as_secs_f32() / TURN_TIME.as_secs_f32()).min(1.0);
        let turning = progress < 1.0;
        let backward = self.previous.is_some_and(|old| old > self.page);
        let right = if wide { book.x + leaf_width } else { book.x };
        let buffer = frame.buffer_mut();

        let cover = self.previous.is_none() && turning && progress < 0.5;
        if wide && !cover {
            let left_page = if turning && !backward { self.previous.unwrap_or(self.page) } else { self.page };
            let leaf = page(left_page, leaf_width, height, false);
            place(buffer, &leaf, book.x, book.y);
        }
        let right_page = if wide && turning && backward {
            self.previous.unwrap_or(self.page)
        } else {
            self.page
        };
        let leaf = page(right_page, leaf_width, height, true);
        place(buffer, &leaf, right, book.y);

        let bound_from = if wide && cover { right } else { book.x };
        let raised_edge = Style::default().fg(EDGE).bg(LEATHER);
        let width = book.right() - bound_from;
        buffer.set_string(bound_from, book.bottom(), "═".repeat(usize::from(width)), raised_edge);
        buffer.set_string(bound_from, book.bottom() + 1, "─".repeat(usize::from(width)), Style::default().fg(SHADOW));

        if turning {
            let angle = progress * std::f32::consts::PI;
            let front = progress < 0.5;
            let show_right = if wide { front != backward } else { true };
            let side = if show_right { 1 } else { -1 };
            let hinge = if wide { right } else { book.x };
            let face = if cover {
                cover_leaf(leaf_width, height)
            } else {
                let index = if front { self.previous.unwrap_or(self.page) } else { self.page };
                page(index, leaf_width, height, show_right)
            };
            turn_leaf(buffer, &face, hinge, book.y, side, angle);
        }

        let actions = if self.page + 1 == ENTRIES.len() {
            "[←] Back   [Enter] Enter tower"
        } else if book_width < 32 {
            "[←] Back  [→] Turn  [Esc] Skip"
        } else if self.page == 0 {
            "[→ / Enter] Turn   [Esc] Skip"
        } else {
            "[←] Back   [→ / Enter] Turn   [Esc] Skip"
        };
        frame.render_widget(
            Paragraph::new(actions).centered().style(Style::default().fg(BRASS)),
            Rect::new(area.x, area.bottom() - 1, area.width, 1),
        );
    }

    fn render_close(&self, frame: &mut Frame, book: Rect, area: Rect, width: u16, wide: bool, elapsed: Duration) {
        let right = if wide { book.x + width } else { book.x };
        let buffer = frame.buffer_mut();
        if elapsed < CLOSE_TIME {
            let progress = elapsed.as_secs_f32() / CLOSE_TIME.as_secs_f32();
            let right_leaf = page(self.page, width, book.height, true);
            place(buffer, &right_leaf, right, book.y);
            let front = progress < 0.5;
            let side = if wide && front { -1 } else { 1 };
            let leaf = if front {
                page(self.page, width, book.height, !wide)
            } else {
                cover_leaf(width, book.height)
            };
            turn_leaf(buffer, &leaf, right, book.y, side, progress * std::f32::consts::PI);
            return;
        }

        let fall = ((elapsed - CLOSE_TIME).as_secs_f32() / FALL_TIME.as_secs_f32()).min(1.0);
        let fall = fall * fall;
        let floor = area.bottom() - 2;
        let height = book.height - ((book.height.saturating_sub(3)) as f32 * fall) as u16;
        let width = width - (f32::from(width / 3) * fall) as u16;
        let x = right - (f32::from((right - area.x) / 3) * fall) as u16;
        let y = book.y + (f32::from(floor.saturating_sub(book.y + height)) * fall) as u16;
        let leather = if height < 5 {
            let rect = Rect::new(0, 0, width, height);
            let mut cover = Buffer::empty(rect);
            Block::bordered().style(Style::default().fg(BRASS).bg(LEATHER)).render(rect, &mut cover);
            cover
        } else {
            cover_leaf(width, height)
        };
        place(buffer, &leather, x, y);

        let stone = Style::default().fg(EDGE);
        let seam = "-___".repeat(usize::from(area.width) / 4 + 1);
        buffer.set_stringn(area.x, floor, seam, usize::from(area.width), stone);
        if elapsed >= CLOSE_TIME + FALL_TIME {
            let age = elapsed - CLOSE_TIME - FALL_TIME;
            let spread = (age.as_millis() / 35) as u16;
            for dust in [x.saturating_sub(spread + 1).max(area.x), (x + width + spread).min(area.right() - 1)] {
                buffer.set_string(dust, floor - 1, ".", Style::default().fg(BRASS));
            }
        }
    }
}

fn page(index: usize, width: u16, height: u16, story: bool) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut leaf = Buffer::empty(area);
    let style = Style::default().fg(INK).bg(PAPER);
    Block::default().style(style).render(area, &mut leaf);
    let binding = Style::default().fg(EDGE).bg(PAPER);
    let inner_edge = if story { 0 } else { width - 1 };
    for y in 0..height {
        leaf.set_string(inner_edge, y, "│", binding);
    }
    if height < 5 {
        return leaf;
    }

    let entry = &ENTRIES[index];
    let title = if story { entry.title } else { "THE ASH REGISTER" };
    Paragraph::new(title)
        .centered()
        .style(style.add_modifier(Modifier::BOLD))
        .render(Rect::new(2, 1, width - 4, 1), &mut leaf);
    if story {
        let text = if height >= 16 && width >= 33 { entry.text } else { entry.small };
        let body_y = if height >= 9 { 3 } else { 2 };
        Paragraph::new(text).style(style).wrap(Wrap { trim: true })
            .render(Rect::new(3, body_y, width - 6, height - body_y - 1), &mut leaf);
        leaf.set_string(width - 4, height - 1, format!("{:02}", index + 1), binding);
    } else {
        Paragraph::new(entry.engraving.join("\n"))
            .centered()
            .style(style)
            .render(Rect::new(1, (height - 9) / 2, width - 2, 9), &mut leaf);
    }
    leaf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finished_leaves_wait_for_the_reader_and_can_turn_back() {
        let mut book = Prologue::new();
        assert!(!book.read_key(KeyCode::Enter));
        assert_eq!(book.page, 0);

        for key in [KeyCode::Right, KeyCode::Right, KeyCode::Left, KeyCode::Enter, KeyCode::Char(' ')] {
            book.turned_at = Instant::now() - TURN_TIME;
            assert!(book.read_key(key));
        }
        assert_eq!(book.page, ENTRIES.len() - 1);
        book.turned_at = Instant::now() - TURN_TIME;
        assert!(book.read_key(KeyCode::Enter));
        assert!(!book.finished());
        book.closing_at = Some(Instant::now() - CLOSE_TIME - FALL_TIME - SETTLE_TIME);
        assert!(book.take_landing());
        assert!(!book.take_landing());
        assert!(book.finished());
    }
}

fn cover_leaf(width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut leaf = Buffer::empty(area);
    let style = Style::default().fg(BRASS).bg(LEATHER);
    Block::bordered().style(style).render(area, &mut leaf);
    let stamp = if height >= 10 { ".-^-.\n |#|\n /_\\\n\nTHE ASH REGISTER" } else { "ASH REGISTER" };
    Paragraph::new(stamp).centered().style(style).render(
        Rect::new(1, if height >= 10 { (height - 5) / 2 } else { height / 2 }, width - 2, height.min(5)),
        &mut leaf,
    );
    leaf
}

fn place(target: &mut Buffer, leaf: &Buffer, x: u16, y: u16) {
    for row in 0..leaf.area.height {
        for col in 0..leaf.area.width {
            target[(x + col, y + row)] = leaf[(col, row)].clone();
        }
    }
}

fn turn_leaf(target: &mut Buffer, leaf: &Buffer, hinge: u16, y: u16, side: i32, angle: f32) {
    let width = leaf.area.width;
    let folded = (f32::from(width) * angle.cos().abs()).round().max(1.0) as u16;
    let curl = angle.sin();
    for col in 0..folded {
        let tip = f32::from(col + 1) / f32::from(folded);
        let lift = (2.4 * curl * tip * tip).round() as u16;
        let x = if side > 0 { hinge + col } else { hinge - col - 1 };
        let source = (u32::from(col) * u32::from(width) / u32::from(folded)) as u16;
        let source = if side > 0 { source } else { width - 1 - source };
        for row in 0..leaf.area.height {
            let cell = &mut target[(x, y + row - lift)];
            *cell = leaf[(source, row)].clone();
            if col + 1 == folded && curl > 0.12 {
                cell.set_char(if side > 0 { ')' } else { '(' });
                cell.set_fg(BRASS);
                cell.set_bg(EDGE);
            } else if curl > 0.10 && cell.bg == PAPER {
                let shade = (curl * tip * 42.0) as u8;
                cell.set_bg(Color::Rgb(188 - shade, 155 - shade, 110 - shade));
            }
        }
        if lift != 0 {
            let shadow = y + leaf.area.height - lift;
            for row in shadow..y + leaf.area.height {
                target[(x, row)].set_bg(SHADOW);
            }
        }
    }
}
