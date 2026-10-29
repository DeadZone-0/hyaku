use crate::direction::Direction;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn manhattan_distance(&self, other: Position) -> u32 {
        ((self.x - other.x).abs() + (self.y - other.y).abs()) as u32
    }

    pub fn chebyshev_distance(&self, other: Position) -> u32 {
        (self.x - other.x).abs().max((self.y - other.y).abs()) as u32
    }

    pub const fn distance_squared(&self, other: Position) -> i32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    pub fn step(&self, dir: Direction) -> Position {
        let (dx, dy) = dir.delta();
        Position {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    pub const fn in_bounds(&self, width: i32, height: i32) -> bool {
        self.x >= 0 && self.x < width && self.y >= 0 && self.y < height
    }

    pub const fn add_offset(&self, dx: i32, dy: i32) -> Position {
        Position {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    pub fn cardinal_neighbors(&self) -> [Position; 4] {
        [
            self.step(Direction::Up),
            self.step(Direction::Down),
            self.step(Direction::Left),
            self.step(Direction::Right),
        ]
    }

    pub fn all_neighbors(&self) -> [Position; 8] {
        [
            Position::new(self.x, self.y - 1),
            Position::new(self.x, self.y + 1),
            Position::new(self.x - 1, self.y),
            Position::new(self.x + 1, self.y),
            Position::new(self.x - 1, self.y - 1),
            Position::new(self.x + 1, self.y - 1),
            Position::new(self.x - 1, self.y + 1),
            Position::new(self.x + 1, self.y + 1),
        ]
    }

    pub fn direction_to(&self, target: Position) -> Option<Direction> {
        let dx = target.x - self.x;
        let dy = target.y - self.y;

        if dx == 0 && dy == 0 {
            return None;
        }

        if dx.abs() >= dy.abs() {
            if dx > 0 {
                Some(Direction::Right)
            } else {
                Some(Direction::Left)
            }
        } else if dy > 0 {
            Some(Direction::Down)
        } else {
            Some(Direction::Up)
        }
    }
}

impl Add for Position {
    type Output = Position;

    fn add(self, rhs: Position) -> Position {
        Position {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Position {
    type Output = Position;

    fn sub(self, rhs: Position) -> Position {
        Position {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}
