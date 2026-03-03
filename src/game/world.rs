use crate::game::types::{ChargingStationState, Position, TileType};

pub const WORLD_WIDTH: usize = 200;
pub const WORLD_HEIGHT: usize = 200;

pub struct World {
    pub tiles: Vec<Vec<TileType>>,
    pub explored: Vec<Vec<bool>>,
    pub charging_stations: Vec<ChargingStationState>,
    pub day_count: u32,
    pub time_of_day: f32,
    pub mars_health: f32,
    pub base_position: Position,
    pub ice_stored: u32,
}

impl World {
    pub fn new() -> Self {
        let mut tiles = vec![vec![TileType::Regolith; WORLD_WIDTH]; WORLD_HEIGHT];

        // Base at center
        let base_x = WORLD_WIDTH / 2;
        let base_y = WORLD_HEIGHT / 2;
        tiles[base_y][base_x] = TileType::Base;

        // Procedural generation: sparse rocks and ice
        // Using simple pseudo-random based on coordinates
        for y in 0..WORLD_HEIGHT {
            for x in 0..WORLD_WIDTH {
                if tiles[y][x] == TileType::Base {
                    continue;
                }

                let hash = Self::hash_coords(x, y);

                // 5% chance of rock
                if hash % 100 < 5 {
                    tiles[y][x] = TileType::Rock;
                }
                // 3% chance of ice (check different hash offset)
                else if (hash / 100) % 100 < 3 {
                    tiles[y][x] = TileType::Ice;
                }
            }
        }

        let mut explored = vec![vec![false; WORLD_WIDTH]; WORLD_HEIGHT];

        // Mark starting area around base as explored
        let vision_radius = 6;
        for dy in -(vision_radius as i32)..=(vision_radius as i32) {
            for dx in -(vision_radius as i32)..=(vision_radius as i32) {
                let x = (base_x as i32 + dx) as usize;
                let y = (base_y as i32 + dy) as usize;
                if x < WORLD_WIDTH && y < WORLD_HEIGHT {
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq <= vision_radius * vision_radius {
                        explored[y][x] = true;
                    }
                }
            }
        }

        Self {
            tiles,
            explored,
            charging_stations: Vec::new(),
            day_count: 1,
            time_of_day: 0.25, // Start at morning
            mars_health: 0.0,
            base_position: Position::new(base_x as i32, base_y as i32),
            ice_stored: 0,
        }
    }

    pub fn get_tile(&self, pos: &Position) -> Option<TileType> {
        if pos.x < 0 || pos.y < 0 || pos.x >= WORLD_WIDTH as i32 || pos.y >= WORLD_HEIGHT as i32 {
            return None;
        }
        Some(self.tiles[pos.y as usize][pos.x as usize])
    }

    pub fn set_tile(&mut self, pos: &Position, tile_type: TileType) {
        if pos.x >= 0 && pos.y >= 0 && pos.x < WORLD_WIDTH as i32 && pos.y < WORLD_HEIGHT as i32 {
            self.tiles[pos.y as usize][pos.x as usize] = tile_type;
        }
    }

    pub fn is_day(&self) -> bool {
        self.time_of_day < 0.5
    }

    pub fn mark_as_explored(&mut self, pos: &Position) {
        if pos.x >= 0 && pos.y >= 0 && pos.x < WORLD_WIDTH as i32 && pos.y < WORLD_HEIGHT as i32 {
            self.explored[pos.y as usize][pos.x as usize] = true;
        }
    }

    pub fn is_explored(&self, pos: &Position) -> bool {
        if pos.x < 0 || pos.y < 0 || pos.x >= WORLD_WIDTH as i32 || pos.y >= WORLD_HEIGHT as i32 {
            return false;
        }
        self.explored[pos.y as usize][pos.x as usize]
    }

    pub fn advance_time(&mut self, delta: f32) {
        self.time_of_day += delta;
        if self.time_of_day >= 1.0 {
            self.time_of_day = 0.0;
            self.day_count += 1;

            // Advance charging station boot-up timers
            for station in &mut self.charging_stations {
                station.advance_day();
            }

            // Subtle terraforming progress every 5 days
            if self.day_count % 5 == 0 {
                self.mars_health = (self.mars_health + 0.01).min(1.0);
            }
        }
    }

    pub fn can_place_charging_station(&self, pos: &Position) -> bool {
        // Can't place out of bounds
        if pos.x < 0 || pos.y < 0 || pos.x >= WORLD_WIDTH as i32 || pos.y >= WORLD_HEIGHT as i32 {
            return false;
        }

        let tile = self.tiles[pos.y as usize][pos.x as usize];

        // Can only place on regolith or ice
        if tile != TileType::Regolith && tile != TileType::Ice {
            return false;
        }

        // Can't place if one already exists here
        if self.charging_stations.iter().any(|s| s.position == *pos) {
            return false;
        }

        true
    }

    pub fn place_charging_station(&mut self, pos: Position) -> bool {
        if !self.can_place_charging_station(&pos) {
            return false;
        }

        self.tiles[pos.y as usize][pos.x as usize] = TileType::ChargingStation;
        self.charging_stations.push(ChargingStationState::new(pos));
        true
    }

    pub fn get_charging_station(&self, pos: &Position) -> Option<&ChargingStationState> {
        self.charging_stations.iter().find(|s| s.position == *pos)
    }

    pub fn is_charging_station_operational(&self, pos: &Position) -> bool {
        self.get_charging_station(pos)
            .map(|s| s.is_operational())
            .unwrap_or(false)
    }

    pub fn process_ice(&mut self, delta_time: f32) {
        if self.ice_stored == 0 {
            return;
        }

        // Process 1 ice sample every ~10 seconds at base
        // Each ice sample adds significant terraforming progress
        let process_rate = 0.1; // 0.1 ice per second = 1 ice per 10 seconds
        let ice_to_process = (process_rate * delta_time).min(self.ice_stored as f32);

        if ice_to_process >= 1.0 {
            self.ice_stored -= 1;
            // Each ice sample processed adds 0.02 to mars_health (50 ice = full terraform)
            self.mars_health = (self.mars_health + 0.02).min(1.0);
        }
    }

    // Simple hash function for procedural generation
    fn hash_coords(x: usize, y: usize) -> usize {
        let mut hash = x.wrapping_mul(374761393);
        hash = hash.wrapping_add(y.wrapping_mul(668265263));
        hash ^= hash >> 13;
        hash = hash.wrapping_mul(1274126177);
        hash ^= hash >> 16;
        hash
    }
}
