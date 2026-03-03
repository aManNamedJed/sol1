use crate::game::input::InputAction;
use crate::game::robot::Robot;
use crate::game::types::{Position, TileType};
use crate::game::world::World;
use std::collections::{HashMap, VecDeque};

/// AI Mission: Terraform Mars by collecting as much ice as possible
/// Strategy: Aggressive exploration with charging station network
pub struct RobotAI {
    last_action_time: f64,
    action_cooldown: f64,
    exploration_direction: (i32, i32),
    stuck_counter: u32,
    last_position: Position,
    last_station_build_distance: f32,
    // Memory systems
    known_ice_locations: Vec<Position>,
    known_obstacles: HashMap<(i32, i32), bool>, // (x, y) -> is_obstacle
    unreachable_targets: HashMap<(i32, i32), u32>, // (x, y) -> failed attempts
    current_path: VecDeque<Position>,
    path_target: Option<Position>,
}

impl RobotAI {
    pub fn new() -> Self {
        Self {
            last_action_time: 0.0,
            action_cooldown: 0.3,
            exploration_direction: (1, 0),
            stuck_counter: 0,
            last_position: Position::new(100, 100),
            last_station_build_distance: 0.0,
            known_ice_locations: Vec::new(),
            known_obstacles: HashMap::new(),
            unreachable_targets: HashMap::new(),
            current_path: VecDeque::new(),
            path_target: None,
        }
    }

    pub fn decide_action(
        &mut self,
        world: &World,
        robot: &Robot,
        current_time: f64,
    ) -> Option<InputAction> {
        if current_time - self.last_action_time < self.action_cooldown {
            return None;
        }

        // Update memory from visible area
        self.update_memory(world, robot);

        // Track if stuck and handle aggressively
        if robot.position == self.last_position {
            self.stuck_counter += 1;
            if self.stuck_counter > 3 {
                // Getting stuck - take evasive action
                self.exploration_direction = self.rotate_direction(self.exploration_direction);
                self.current_path.clear();

                // Mark current target as unreachable
                if let Some(target) = self.path_target {
                    let key = (target.x, target.y);
                    let count = self.unreachable_targets.get(&key).copied().unwrap_or(0);
                    self.unreachable_targets.insert(key, count + 1);
                }
                self.path_target = None;

                // If really stuck, try random exploration
                if self.stuck_counter > 6 {
                    self.stuck_counter = 0;
                    // Pick a random direction by using position as seed
                    let rand_idx = ((robot.position.x + robot.position.y) as usize) % 4;
                    let directions = [(0, -1), (1, 0), (0, 1), (-1, 0)];
                    self.exploration_direction = directions[rand_idx];
                }
            }
        } else {
            self.stuck_counter = 0;
            self.last_position = robot.position;
        }

        let action = self.decide_behavior(world, robot);

        if action.is_some() {
            self.last_action_time = current_time;
        }

        action
    }

    fn update_memory(&mut self, world: &World, robot: &Robot) {
        // Scan visible area (8 tile radius) and remember ice/obstacles
        for dy in -8..=8 {
            for dx in -8..=8 {
                if dx * dx + dy * dy > 64 {
                    continue;
                }
                let pos = Position::new(robot.position.x + dx, robot.position.y + dy);

                if let Some(tile) = world.get_tile(&pos) {
                    let key = (pos.x, pos.y);

                    match tile {
                        TileType::Rock => {
                            self.known_obstacles.insert(key, true);
                        }
                        TileType::Ice => {
                            // Add to known ice if not already collected
                            if !self.known_ice_locations.contains(&pos) {
                                self.known_ice_locations.push(pos);
                            }
                        }
                        TileType::Regolith | TileType::Base | TileType::ChargingStation => {
                            self.known_obstacles.insert(key, false);
                        }
                    }
                }
            }
        }

        // Clean up collected ice from memory
        self.known_ice_locations
            .retain(|&pos| matches!(world.get_tile(&pos), Some(TileType::Ice)));
    }

    fn decide_behavior(&mut self, world: &World, robot: &Robot) -> Option<InputAction> {
        // PRIORITY 1: Standing on ice? Collect it!
        if matches!(world.get_tile(&robot.position), Some(TileType::Ice)) {
            return Some(InputAction::Collect);
        }

        // PRIORITY 2: Full inventory? Return to base to deposit
        if robot.ice_samples >= 3 {
            return self.return_to_base(world, robot);
        }

        // Calculate key metrics for decision making
        let nearest_charger = self.find_nearest_charger(world, robot);
        let distance_to_charger = robot.position.distance_to(&nearest_charger);
        let critical_energy = distance_to_charger * 1.2 + 5.0; // Safe margin
        let at_charger = robot.position == world.base_position
            || world.is_charging_station_operational(&robot.position);

        // PRIORITY 3: Dynamic charging strategy for speed
        if at_charger {
            // Check if there's nearby ice worth getting immediately
            let nearby_ice = self.find_nearest_ice(world, robot, 12);

            // Smart charging based on context:
            // - If ice is very close (<=3 tiles), leave at 50% to grab it quickly
            // - If ice nearby (<=8 tiles), leave at 70%
            // - If exploring far, charge to 90%
            // - During night, always charge to 95% for safety
            let is_night = world.time_of_day >= 0.5;
            let min_charge = if is_night {
                95.0
            } else if let Some(ice_pos) = nearby_ice {
                let ice_dist = robot.position.distance_to(&ice_pos);
                if ice_dist <= 3.0 {
                    50.0 // Very close ice - go now!
                } else if ice_dist <= 8.0 {
                    70.0 // Nearby ice - moderate charge
                } else {
                    85.0 // Far ice - good charge
                }
            } else {
                85.0 // No known ice - charge well for exploration
            };

            if robot.energy < min_charge {
                // Keep charging
                return None;
            }
        }

        // PRIORITY 4: Critical energy? Get to nearest charger NOW
        if robot.energy < critical_energy {
            return self.navigate_to(world, robot, nearest_charger);
        }

        // PRIORITY 5: Proactive charging station network building
        // Build stations systematically every 30 tiles to create infrastructure
        let distance_from_base = robot.position.distance_to(&world.base_position);
        let distance_from_last_station = distance_from_base - self.last_station_build_distance;

        let should_build_station = world.can_place_charging_station(&robot.position)
            && world.charging_stations.len() < 40  // Allow generous network
            && robot.energy > 60.0  // Good energy for building
            && distance_to_charger > 30.0  // Far enough from existing chargers
            && distance_from_last_station > 30.0  // Regular 30-tile intervals
            && distance_from_base > 35.0; // Don't clutter near base

        if should_build_station {
            self.last_station_build_distance = distance_from_base;
            return Some(InputAction::PlaceChargingStation);
        }

        // PRIORITY 6: Ice nearby? Go get it!
        if let Some(ice_pos) = self.find_nearest_ice(world, robot, 8) {
            return self.navigate_to(world, robot, ice_pos);
        }

        // PRIORITY 7: Explore aggressively to find ice
        self.explore_for_ice(world, robot)
    }

    fn return_to_base(&mut self, world: &World, robot: &Robot) -> Option<InputAction> {
        self.navigate_to(world, robot, world.base_position)
    }

    fn explore_for_ice(&mut self, world: &World, robot: &Robot) -> Option<InputAction> {
        // Update exploration direction towards unexplored areas
        if self.stuck_counter > 2 || (robot.position.x + robot.position.y) % 15 == 0 {
            self.exploration_direction = self.find_best_exploration_direction(world, robot);
        }

        let (dx, dy) = self.exploration_direction;

        if self.can_move(world, robot, dx, dy) {
            return self.move_direction(dx, dy);
        } else {
            // Blocked, find new direction
            self.exploration_direction = self.find_best_exploration_direction(world, robot);
            return self.try_any_move(world, robot);
        }
    }

    fn navigate_to(
        &mut self,
        world: &World,
        robot: &Robot,
        target: Position,
    ) -> Option<InputAction> {
        if robot.position == target {
            self.current_path.clear();
            self.path_target = None;
            return None;
        }

        // If we have a path for this target, try to follow it
        if self.path_target == Some(target) && !self.current_path.is_empty() {
            // Peek at next position in path
            if let Some(&next_pos) = self.current_path.front() {
                let dx = next_pos.x - robot.position.x;
                let dy = next_pos.y - robot.position.y;

                // Verify next position is adjacent (path might be stale if robot moved unexpectedly)
                if dx.abs() <= 1 && dy.abs() <= 1 && (dx != 0 || dy != 0) {
                    // Check if we can move to this next position
                    if self.can_move(world, robot, dx, dy) {
                        self.current_path.pop_front(); // Remove this waypoint
                        return self.move_direction(dx, dy);
                    } else {
                        // Path is blocked, need to recalculate
                        self.current_path.clear();
                        self.path_target = None;
                    }
                } else {
                    // Path is invalid (waypoint too far), recalculate
                    self.current_path.clear();
                    self.path_target = None;
                }
            }
        }

        // Need new path or path failed - compute using BFS
        if self.path_target != Some(target) || self.current_path.is_empty() {
            self.current_path = self.find_path(world, robot, target);
            self.path_target = Some(target);

            // Try to follow the newly calculated path immediately
            if let Some(&next_pos) = self.current_path.front() {
                let dx = next_pos.x - robot.position.x;
                let dy = next_pos.y - robot.position.y;

                if dx.abs() <= 1 && dy.abs() <= 1 && self.can_move(world, robot, dx, dy) {
                    self.current_path.pop_front();
                    return self.move_direction(dx, dy);
                }
            }
        }

        // Path calculation failed or path is blocked, use greedy approach
        self.current_path.clear();
        self.path_target = None;
        self.greedy_move_to(world, robot, target)
    }

    fn find_path(&self, world: &World, robot: &Robot, target: Position) -> VecDeque<Position> {
        // BFS pathfinding with proper obstacle avoidance
        let mut queue = VecDeque::new();
        let mut visited = HashMap::new();
        let mut parent: HashMap<(i32, i32), Position> = HashMap::new();

        queue.push_back(robot.position);
        visited.insert((robot.position.x, robot.position.y), true);

        let max_search_distance = 80; // Search up to 80 tiles away
        let mut iterations = 0;
        let max_iterations = 8000; // Larger search space for complex paths

        while let Some(current) = queue.pop_front() {
            iterations += 1;
            if iterations > max_iterations {
                break; // Prevent infinite loops
            }

            // Found target!
            if current == target {
                // Reconstruct path from target back to start
                let mut path = VecDeque::new();
                let mut pos = target;

                while pos != robot.position {
                    path.push_front(pos);
                    if let Some(&prev) = parent.get(&(pos.x, pos.y)) {
                        pos = prev;
                    } else {
                        // Path broken - shouldn't happen but handle it
                        return VecDeque::new();
                    }
                }

                return path;
            }

            // Explore all 4 adjacent tiles
            for &(dx, dy) in &[(0, -1), (1, 0), (0, 1), (-1, 0)] {
                let next = Position::new(current.x + dx, current.y + dy);
                let key = (next.x, next.y);

                // Already visited this tile
                if visited.contains_key(&key) {
                    continue;
                }

                // Don't search too far from robot - check on NEXT position not current
                if robot.position.distance_to(&next) > max_search_distance as f32 {
                    continue;
                }

                // Check known obstacle memory
                if let Some(&is_rock) = self.known_obstacles.get(&key) {
                    if is_rock {
                        continue; // Known rock, skip it
                    }
                }

                // Check actual world state
                match world.get_tile(&next) {
                    Some(TileType::Rock) => {
                        continue; // Can't walk through rocks
                    }
                    None => {
                        continue; // Out of bounds
                    }
                    Some(_) => {
                        // Walkable tile (Regolith, Ice, Base, ChargingStation)
                    }
                }

                // This tile is walkable and unvisited
                visited.insert(key, true);
                parent.insert(key, current);
                queue.push_back(next);
            }
        }

        // No path found - return empty path
        VecDeque::new()
    }

    fn greedy_move_to(
        &self,
        world: &World,
        robot: &Robot,
        target: Position,
    ) -> Option<InputAction> {
        let dx = target.x - robot.position.x;
        let dy = target.y - robot.position.y;

        // Try moves in order of preference
        let moves = if dx.abs() > dy.abs() {
            [
                (dx.signum(), 0),
                (0, dy.signum()),
                (dx.signum(), dy.signum()),
                (-dx.signum(), 0),
            ]
        } else {
            [
                (0, dy.signum()),
                (dx.signum(), 0),
                (dx.signum(), dy.signum()),
                (0, -dy.signum()),
            ]
        };

        for (mx, my) in moves {
            if mx == 0 && my == 0 {
                continue;
            }
            if self.can_move(world, robot, mx, my) {
                return self.move_direction(mx, my);
            }
        }

        self.try_any_move(world, robot)
    }

    fn find_best_exploration_direction(&self, world: &World, robot: &Robot) -> (i32, i32) {
        let directions = [
            (0, -1),
            (1, 0),
            (0, 1),
            (-1, 0),
            (1, -1),
            (1, 1),
            (-1, 1),
            (-1, -1),
        ];

        let mut best_dir = self.exploration_direction;
        let mut best_score = 0;

        for &(dx, dy) in &directions {
            let mut score = 0;

            // Look ahead 30 tiles in this direction
            for dist in 1..30 {
                let check_pos =
                    Position::new(robot.position.x + dx * dist, robot.position.y + dy * dist);

                // Check if blocked by known obstacles
                if self
                    .known_obstacles
                    .get(&(check_pos.x, check_pos.y))
                    .copied()
                    .unwrap_or(false)
                {
                    score -= 5; // Penalize directions with obstacles
                    continue;
                }

                // Score unexplored tiles highly
                if !world.is_explored(&check_pos) {
                    score += 3;
                }

                // Score visible ice even higher
                if let Some(TileType::Ice) = world.get_tile(&check_pos) {
                    score += 10;
                }
            }

            // Bonus score if direction leads toward known ice
            for &ice_pos in &self.known_ice_locations {
                let ice_dx = ice_pos.x - robot.position.x;
                let ice_dy = ice_pos.y - robot.position.y;

                // Check if this direction moves toward the ice
                if (dx * ice_dx + dy * ice_dy) > 0 {
                    score += 5;
                }
            }

            if score > best_score {
                best_score = score;
                best_dir = (dx, dy);
            }
        }

        // If no good direction found, move away from base
        if best_score < 5 {
            let away_x = robot.position.x - world.base_position.x;
            let away_y = robot.position.y - world.base_position.y;

            if away_x.abs() > away_y.abs() {
                best_dir = (away_x.signum(), 0);
            } else {
                best_dir = (0, away_y.signum());
            }
        }

        best_dir
    }

    fn find_nearest_ice(&self, world: &World, robot: &Robot, radius: i32) -> Option<Position> {
        let mut best_ice = None;
        let mut best_dist = f32::MAX;

        // First check known ice locations in memory
        for &ice_pos in &self.known_ice_locations {
            // Skip ice that we've repeatedly failed to reach
            let key = (ice_pos.x, ice_pos.y);
            if let Some(&fail_count) = self.unreachable_targets.get(&key) {
                if fail_count > 2 {
                    continue; // Skip unreachable targets
                }
            }

            let dist = robot.position.distance_to(&ice_pos);
            if dist < best_dist && dist > 0.1 && dist <= radius as f32 {
                // Verify it's still ice
                if matches!(world.get_tile(&ice_pos), Some(TileType::Ice)) {
                    best_dist = dist;
                    best_ice = Some(ice_pos);
                }
            }
        }

        // Also search immediate visible area
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy > radius * radius {
                    continue;
                }

                let pos = Position::new(robot.position.x + dx, robot.position.y + dy);
                if matches!(world.get_tile(&pos), Some(TileType::Ice)) {
                    let dist = robot.position.distance_to(&pos);
                    if dist < best_dist && dist > 0.1 {
                        best_dist = dist;
                        best_ice = Some(pos);
                    }
                }
            }
        }

        best_ice
    }

    fn find_nearest_charger(&self, world: &World, robot: &Robot) -> Position {
        let mut nearest = world.base_position;
        let mut nearest_dist = robot.position.distance_to(&world.base_position);

        for station in &world.charging_stations {
            if station.is_operational() {
                let dist = robot.position.distance_to(&station.position);
                if dist < nearest_dist {
                    nearest = station.position;
                    nearest_dist = dist;
                }
            }
        }

        nearest
    }

    fn can_move(&self, world: &World, robot: &Robot, dx: i32, dy: i32) -> bool {
        if robot.energy < 1.0 || dx == 0 && dy == 0 {
            return false;
        }

        let pos = Position::new(robot.position.x + dx, robot.position.y + dy);
        !matches!(world.get_tile(&pos), Some(TileType::Rock) | None)
    }

    fn move_direction(&self, dx: i32, dy: i32) -> Option<InputAction> {
        match (dx, dy) {
            (0, -1) => Some(InputAction::MoveUp),
            (0, 1) => Some(InputAction::MoveDown),
            (-1, 0) => Some(InputAction::MoveLeft),
            (1, 0) => Some(InputAction::MoveRight),
            _ => {
                if dx != 0 {
                    self.move_direction(dx, 0)
                } else {
                    self.move_direction(0, dy)
                }
            }
        }
    }

    fn try_any_move(&self, world: &World, robot: &Robot) -> Option<InputAction> {
        for &(dx, dy) in &[(0, -1), (1, 0), (0, 1), (-1, 0)] {
            if self.can_move(world, robot, dx, dy) {
                return self.move_direction(dx, dy);
            }
        }
        None
    }

    fn rotate_direction(&self, dir: (i32, i32)) -> (i32, i32) {
        match dir {
            (0, -1) => (1, 0),
            (1, 0) => (0, 1),
            (0, 1) => (-1, 0),
            (-1, 0) => (0, -1),
            _ => (1, 0),
        }
    }
}
