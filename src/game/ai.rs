use crate::game::input::InputAction;
use crate::game::robot::Robot;
use crate::game::types::{Position, TileType};
use crate::game::world::World;

#[derive(Clone, Copy, PartialEq, Debug)]
enum AIState {
    Exploring,
    ReturningToBase,
    Collecting,
    BuildingStation,
}

pub struct RobotAI {
    state: AIState,
    target_position: Option<Position>,
    last_action_time: f64,
    action_cooldown: f64,
    exploration_direction: (i32, i32),
    stuck_counter: u32,
    last_position: Position,
}

impl RobotAI {
    pub fn new() -> Self {
        Self {
            state: AIState::Exploring,
            target_position: None,
            last_action_time: 0.0,
            action_cooldown: 0.3, // 300ms between actions
            exploration_direction: (1, 0),
            stuck_counter: 0,
            last_position: Position::new(100, 100),
        }
    }

    pub fn decide_action(
        &mut self,
        world: &World,
        robot: &Robot,
        current_time: f64,
    ) -> Option<InputAction> {
        // Cooldown between actions for smooth movement
        if current_time - self.last_action_time < self.action_cooldown {
            return None;
        }

        // Check if stuck
        if robot.position == self.last_position {
            self.stuck_counter += 1;
            if self.stuck_counter > 5 {
                // Change direction if stuck
                self.exploration_direction = self.get_random_direction();
                self.stuck_counter = 0;
            }
        } else {
            self.stuck_counter = 0;
            self.last_position = robot.position;
        }

        // Decide state based on conditions
        self.update_state(world, robot);

        let action = match self.state {
            AIState::ReturningToBase => self.navigate_to(world, robot, world.base_position),
            AIState::Collecting => self.collect_nearby_ice(world, robot),
            AIState::BuildingStation => self.consider_building_station(world, robot),
            AIState::Exploring => self.explore(world, robot),
        };

        if action.is_some() {
            self.last_action_time = current_time;
        }

        action
    }

    fn update_state(&mut self, world: &World, robot: &Robot) {
        // Critical energy - must return
        let distance_to_base = robot.position.distance_to(&world.base_position);
        let energy_needed = distance_to_base + 10.0; // Safety margin
        
        if robot.energy < energy_needed && self.state != AIState::ReturningToBase {
            self.state = AIState::ReturningToBase;
            self.target_position = Some(world.base_position);
            return;
        }

        // At base and carrying ice - we're done returning
        if robot.position == world.base_position && self.state == AIState::ReturningToBase {
            self.state = AIState::Exploring;
            self.target_position = None;
            return;
        }

        // Carrying ice and not critical on energy - return to deposit
        if robot.ice_samples >= 3 && self.state != AIState::ReturningToBase {
            self.state = AIState::ReturningToBase;
            self.target_position = Some(world.base_position);
            return;
        }

        // See ice nearby - collect it (but not if we're too close to base - explore first!)
        if self.is_ice_nearby(world, robot) && robot.energy > 20.0 && distance_to_base > 15.0 {
            self.state = AIState::Collecting;
            return;
        }

        // Consider building charging station if far from base
        if distance_to_base > 30.0 
            && robot.energy > 60.0 
            && world.charging_stations.len() < 5 
            && self.should_build_station_here(world, robot) {
            self.state = AIState::BuildingStation;
            return;
        }

        // Default to exploring
        if self.state != AIState::Exploring {
            self.state = AIState::Exploring;
        }
    }

    fn navigate_to(&mut self, world: &World, robot: &Robot, target: Position) -> Option<InputAction> {
        let dx = target.x - robot.position.x;
        let dy = target.y - robot.position.y;

        // Prioritize larger delta for more direct movement
        if dx.abs() > dy.abs() {
            if dx > 0 {
                if self.can_move(world, robot, 1, 0) {
                    return Some(InputAction::MoveRight);
                }
            } else if dx < 0 {
                if self.can_move(world, robot, -1, 0) {
                    return Some(InputAction::MoveLeft);
                }
            }
            // Try vertical if horizontal blocked
            if dy > 0 && self.can_move(world, robot, 0, 1) {
                return Some(InputAction::MoveDown);
            } else if dy < 0 && self.can_move(world, robot, 0, -1) {
                return Some(InputAction::MoveUp);
            }
        } else {
            if dy > 0 {
                if self.can_move(world, robot, 0, 1) {
                    return Some(InputAction::MoveDown);
                }
            } else if dy < 0 {
                if self.can_move(world, robot, 0, -1) {
                    return Some(InputAction::MoveUp);
                }
            }
            // Try horizontal if vertical blocked
            if dx > 0 && self.can_move(world, robot, 1, 0) {
                return Some(InputAction::MoveRight);
            } else if dx < 0 && self.can_move(world, robot, -1, 0) {
                return Some(InputAction::MoveLeft);
            }
        }

        // Try any available direction if blocked
        self.try_any_direction(world, robot)
    }

    fn explore(&mut self, world: &World, robot: &Robot) -> Option<InputAction> {
        // Move in current exploration direction, looking for unexplored tiles
        let (dx, dy) = self.exploration_direction;
        
        if self.can_move(world, robot, dx, dy) {
            let next_pos = Position::new(robot.position.x + dx, robot.position.y + dy);
            
            // Change direction if heading into explored territory
            // Use position-based pseudo-random to decide if we should change direction
            let pseudo_rand = ((robot.position.x + robot.position.y * 3) % 100) as u32;
            if world.is_explored(&next_pos) && pseudo_rand < 30 {
                self.exploration_direction = self.find_unexplored_direction(world, robot);
            }
            
            return self.move_in_direction(dx, dy);
        } else {
            // Hit obstacle, change direction
            self.exploration_direction = self.find_unexplored_direction(world, robot);
            return self.try_any_direction(world, robot);
        }
    }

    fn collect_nearby_ice(&mut self, world: &World, robot: &Robot) -> Option<InputAction> {
        // Check if standing on ice
        if let Some(TileType::Ice) = world.get_tile(&robot.position) {
            return Some(InputAction::Collect);
        }

        // Move toward nearby ice
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let check_pos = Position::new(robot.position.x + dx, robot.position.y + dy);
                if let Some(TileType::Ice) = world.get_tile(&check_pos) {
                    if self.can_move(world, robot, dx, dy) {
                        return self.move_in_direction(dx, dy);
                    }
                }
            }
        }

        None
    }

    fn consider_building_station(&mut self, world: &World, robot: &Robot) -> Option<InputAction> {
        // Build if on suitable ground and no nearby stations
        if world.can_place_charging_station(&robot.position) && robot.energy > 10.0 {
            self.state = AIState::Exploring;
            return Some(InputAction::PlaceChargingStation);
        }

        // Move to better spot
        self.state = AIState::Exploring;
        None
    }

    fn is_ice_nearby(&self, world: &World, robot: &Robot) -> bool {
        for dy in -2..=2 {
            for dx in -2..=2 {
                let check_pos = Position::new(robot.position.x + dx, robot.position.y + dy);
                if let Some(TileType::Ice) = world.get_tile(&check_pos) {
                    return true;
                }
            }
        }
        false
    }

    fn should_build_station_here(&self, world: &World, robot: &Robot) -> bool {
        // Don't build if station is nearby
        for station in &world.charging_stations {
            if robot.position.distance_to(&station.position) < 15.0 {
                return false;
            }
        }

        // Build if on a good tile (regolith or ice)
        let tile = world.get_tile(&robot.position);
        matches!(tile, Some(TileType::Regolith) | Some(TileType::Ice))
    }

    fn find_unexplored_direction(&self, world: &World, robot: &Robot) -> (i32, i32) {
        let directions = [(0, -1), (1, 0), (0, 1), (-1, 0), (1, -1), (1, 1), (-1, 1), (-1, -1)];
        
        for &(dx, dy) in &directions {
            let check_pos = Position::new(robot.position.x + dx * 3, robot.position.y + dy * 3);
            if !world.is_explored(&check_pos) {
                return (dx, dy);
            }
        }

        // All explored, pick random
        self.get_random_direction()
    }

    fn get_random_direction(&self) -> (i32, i32) {
        let directions = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let idx = (self.stuck_counter as usize + self.last_position.x as usize) % directions.len();
        directions[idx]
    }

    fn can_move(&self, world: &World, robot: &Robot, dx: i32, dy: i32) -> bool {
        let new_pos = Position::new(robot.position.x + dx, robot.position.y + dy);
        
        if robot.energy < 1.0 {
            return false;
        }

        match world.get_tile(&new_pos) {
            Some(TileType::Rock) => false,
            Some(_) => true,
            None => false,
        }
    }

    fn move_in_direction(&self, dx: i32, dy: i32) -> Option<InputAction> {
        match (dx, dy) {
            (0, -1) => Some(InputAction::MoveUp),
            (0, 1) => Some(InputAction::MoveDown),
            (-1, 0) => Some(InputAction::MoveLeft),
            (1, 0) => Some(InputAction::MoveRight),
            _ => {
                // Diagonal - pick one axis
                if dx != 0 {
                    self.move_in_direction(dx, 0)
                } else {
                    self.move_in_direction(0, dy)
                }
            }
        }
    }

    fn try_any_direction(&self, world: &World, robot: &Robot) -> Option<InputAction> {
        let directions = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        for &(dx, dy) in &directions {
            if self.can_move(world, robot, dx, dy) {
                return self.move_in_direction(dx, dy);
            }
        }
        None
    }
}


