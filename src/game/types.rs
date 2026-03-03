/// Core types for Sol 1

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TileType {
    Regolith,
    Rock,
    Ice,
    Base,
    ChargingStation,
}

#[derive(Clone, Debug)]
pub struct ChargingStationState {
    pub position: Position,
    pub days_until_operational: u32,
}

impl ChargingStationState {
    pub fn new(position: Position) -> Self {
        Self {
            position,
            days_until_operational: 1,
        }
    }

    pub fn is_operational(&self) -> bool {
        self.days_until_operational == 0
    }

    pub fn advance_day(&mut self) {
        if self.days_until_operational > 0 {
            self.days_until_operational -= 1;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn distance_to(&self, other: &Position) -> f32 {
        let dx = (self.x - other.x) as f32;
        let dy = (self.y - other.y) as f32;
        (dx * dx + dy * dy).sqrt()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_rgba_string(&self) -> String {
        format!("rgba({}, {}, {}, {})", self.r, self.g, self.b, self.a)
    }

    pub fn lerp(&self, other: &Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        Color {
            r: (self.r as f32 + (other.r as f32 - self.r as f32) * t) as u8,
            g: (self.g as f32 + (other.g as f32 - self.g as f32) * t) as u8,
            b: (self.b as f32 + (other.b as f32 - self.b as f32) * t) as u8,
            a: self.a + (other.a - self.a) * t,
        }
    }
}
