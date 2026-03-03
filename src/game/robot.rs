use crate::game::types::Position;

pub struct Robot {
    pub position: Position,
    pub energy: f32,
    pub max_energy: f32,
    #[allow(dead_code)]
    pub integrity: f32,
    pub powered_down: bool,
    pub ice_samples: u32,
    pub movement_cost_multiplier: f32, // 1.0 = normal, lower = more efficient
    pub collection_cost_multiplier: f32, // 1.0 = normal, lower = more efficient
}

impl Robot {
    pub fn new(start_position: Position) -> Self {
        Self {
            position: start_position,
            energy: 100.0,
            max_energy: 100.0,
            integrity: 100.0,
            powered_down: false,
            ice_samples: 0,
            movement_cost_multiplier: 1.0,
            collection_cost_multiplier: 1.0,
        }
    }

    pub fn move_to(&mut self, new_position: Position) -> bool {
        if self.powered_down {
            return false;
        }

        let cost = 1.0 * self.movement_cost_multiplier;
        if self.energy >= cost {
            self.position = new_position;
            self.consume_energy(cost);
            true
        } else {
            false
        }
    }

    pub fn scan(&mut self) -> bool {
        if self.powered_down {
            return false;
        }

        let cost = 2.0;
        if self.energy >= cost {
            self.consume_energy(cost);
            true
        } else {
            false
        }
    }

    pub fn collect(&mut self) -> bool {
        if self.powered_down {
            return false;
        }

        let cost = 3.0 * self.collection_cost_multiplier;
        if self.energy >= cost {
            self.consume_energy(cost);
            true
        } else {
            false
        }
    }

    pub fn consume_energy(&mut self, amount: f32) {
        self.energy -= amount;
        if self.energy <= 0.0 {
            self.energy = 0.0;
            self.powered_down = true;
        }
    }

    pub fn recharge(&mut self, amount: f32) {
        if !self.powered_down {
            self.energy = (self.energy + amount).min(self.max_energy);
        }
    }

    #[allow(dead_code)]
    pub fn restore_emergency_power(&mut self) {
        self.energy = 10.0_f32.min(self.max_energy);
        self.powered_down = false;
    }

    pub fn restore_full_power(&mut self) {
        self.energy = self.max_energy;
        self.powered_down = false;
    }

    #[allow(dead_code)]
    pub fn energy_percentage(&self) -> f32 {
        (self.energy / self.max_energy * 100.0).round()
    }
}
