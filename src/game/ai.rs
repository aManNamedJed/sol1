use crate::game::input::InputAction;
use crate::game::robot::Robot;
use crate::game::types::{Position, TileType};
use crate::game::world::World;
use std::collections::{HashMap, VecDeque};

/// AI Strategy for Mars Terraforming:
///
/// CORE PRINCIPLE: Build a DENSE network of charging stations (every 10-12 tiles)
/// This eliminates energy anxiety and enables deep exploration.
///
/// Decision Flow:
/// 1. Collect ice when standing on it
/// 2. Return to base when inventory full (3 samples)
/// 3. Emergency retreat if critically low energy
/// 4. At charger: charge enough for next mission, then pursue ice or explore
/// 5. AGGRESSIVELY place charging stations every 10+ tiles from nearest operational charger
/// 6. Pursue nearby ice (30 tile radius) if energy sufficient for round trip
/// 7. Explore for more ice, placing stations along the way
///
/// KEY INSIGHTS:
/// - Charging stations are cheap and activate at daybreak
/// - Can "camp" at stations during night instead of returning to base
/// - Distance measured from NEAREST charger (base or station), not just base
/// - Stations enable exponential expansion of operational range
pub struct RobotAI {
    last_action_time: f64,
    action_cooldown: f64,
    exploration_direction: (i32, i32),
    stuck_counter: u32,
    last_position: Position,
    // Memory systems
    known_ice_locations: Vec<Position>,
    known_obstacles: HashMap<(i32, i32), bool>, // (x, y) -> is_obstacle
    unreachable_targets: HashMap<(i32, i32), u32>, // (x, y) -> failed attempts
    current_path: VecDeque<Position>,
    path_target: Option<Position>,
    placed_stations: Vec<Position>, // Track all placed stations
}

impl RobotAI {
    pub fn new() -> Self {
        Self {
            last_action_time: 0.0,
            action_cooldown: 0.3,
            exploration_direction: (1, 0),
            stuck_counter: 0,
            last_position: Position::new(100, 100),
            known_ice_locations: Vec::new(),
            known_obstacles: HashMap::new(),
            unreachable_targets: HashMap::new(),
            current_path: VecDeque::new(),
            path_target: None,
            placed_stations: Vec::new(),
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
        // Scan visible area (10 tile radius for better awareness)
        for dy in -10..=10 {
            for dx in -10..=10 {
                if dx * dx + dy * dy > 100 {
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
                            // If we can see ice, it's probably reachable - clear unreachable flag
                            self.unreachable_targets.remove(&key);
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

        // Periodically reset unreachable targets (maybe paths changed with new stations)
        // Keep the HashMap small and give targets another chance
        if self.unreachable_targets.len() > 20 {
            // Remove entries with low fail counts, keep only the really unreachable ones
            self.unreachable_targets.retain(|_, &mut count| count > 3);
        }

        // Clean up placed_stations list to prevent unbounded growth
        if self.placed_stations.len() > 100 {
            // Keep only the most recent 50 stations
            let new_stations = self
                .placed_stations
                .split_off(self.placed_stations.len() - 50);
            self.placed_stations = new_stations;
        }
    }

    fn decide_behavior(&mut self, world: &World, robot: &Robot) -> Option<InputAction> {
        // Step 1: Collect ice if standing on it
        if matches!(world.get_tile(&robot.position), Some(TileType::Ice)) && robot.ice_samples < 3 {
            return Some(InputAction::Collect);
        }

        // Step 2: Full inventory? Must return to base to deposit
        if robot.ice_samples >= 3 {
            return self.navigate_to(world, robot, world.base_position);
        }

        // Calculate distances from all chargers
        let nearest_charger = self.find_nearest_charger(world, robot);
        let dist_to_nearest_charger = robot.position.distance_to(&nearest_charger);

        // Energy thresholds - EXTREMELY conservative to prevent stranding
        let energy_to_reach_charger = dist_to_nearest_charger * 2.5 + 25.0; // Extra conservative!
        let at_charger = robot.position == world.base_position
            || world.is_charging_station_operational(&robot.position);

        // Step 3: Emergency - critically low energy, get to charger NOW
        if robot.energy < energy_to_reach_charger {
            return self.navigate_to(world, robot, nearest_charger);
        }

        // Step 4: At charger - decide what to do
        if at_charger {
            return self.decide_at_charger(world, robot);
        }

        // Step 5: AGGRESSIVE station placement - every 10-12 tiles from nearest operational charger
        // This creates a dense network that enables deep exploration
        let dist_from_nearest_operational =
            self.distance_to_nearest_operational_charger(world, robot);

        if world.can_place_charging_station(&robot.position)
            && dist_from_nearest_operational > 10.0  // Just 10 tiles out!
            && robot.energy > energy_to_reach_charger + 25.0  // Ensure we can get back after placing
            && !self.placed_stations.contains(&robot.position)
        {
            // Place the station
            self.placed_stations.push(robot.position);
            return Some(InputAction::PlaceChargingStation);
        }

        // Step 6: Look for nearby ice and pursue it aggressively
        if let Some(ice_pos) = self.find_nearest_ice(world, robot, 30) {
            let dist_to_ice = robot.position.distance_to(&ice_pos);

            // Calculate energy for: current -> ice -> nearest charger from ice
            let charger_from_ice = self.find_nearest_charger_from_position(world, &ice_pos);
            let dist_ice_to_charger = ice_pos.distance_to(&charger_from_ice);
            let total_trip_energy = (dist_to_ice + dist_ice_to_charger) * 2.0 + 25.0;

            if robot.energy >= total_trip_energy {
                return self.navigate_to(world, robot, ice_pos);
            }
        }

        // Step 7: Explore for more ice - but ONLY if we have enough energy to return!
        let safe_exploration_energy = dist_to_nearest_charger * 3.0 + 30.0;
        if robot.energy >= safe_exploration_energy {
            self.explore_for_ice(world, robot)
        } else {
            // Not enough energy to explore safely - return to charger
            self.navigate_to(world, robot, nearest_charger)
        }
    }

    fn decide_at_charger(&mut self, world: &World, robot: &Robot) -> Option<InputAction> {
        let is_night = world.time_of_day >= 0.5;

        // At base and inventory full? Deposit
        if robot.position == world.base_position && robot.ice_samples > 0 {
            // Let the game system handle deposit automatically
            return None;
        }

        // Check for nearby ice
        let nearby_ice = self.find_nearest_ice(world, robot, 20);

        // Charging strategy: charge enough to complete next mission
        let target_charge = if is_night {
            // Night: charge to 100% to prepare for dawn
            100.0
        } else if let Some(ice_pos) = nearby_ice {
            // Ice nearby: charge enough for journey + healthy margin
            let dist = robot.position.distance_to(&ice_pos);
            (dist * 3.0 + 25.0).min(100.0) // More conservative!
        } else {
            // No ice: charge to 90% for exploration
            90.0 // Higher than before
        };

        if robot.energy < target_charge {
            // Keep charging
            return None;
        }

        // Charged enough - decide where to go
        if let Some(ice_pos) = nearby_ice {
            // Go get that ice!
            return self.navigate_to(world, robot, ice_pos);
        }

        // No ice nearby - explore
        self.explore_for_ice(world, robot)
    }

    fn distance_to_nearest_operational_charger(&self, world: &World, robot: &Robot) -> f32 {
        let mut nearest_dist = robot.position.distance_to(&world.base_position);

        for station in &world.charging_stations {
            if station.is_operational() {
                let dist = robot.position.distance_to(&station.position);
                if dist < nearest_dist {
                    nearest_dist = dist;
                }
            }
        }

        nearest_dist
    }

    fn explore_for_ice(&mut self, world: &World, robot: &Robot) -> Option<InputAction> {
        // Exploration strategy: spiral out from base, placing stations as we go

        // Update exploration direction periodically
        if self.stuck_counter > 2 || (robot.position.x + robot.position.y) % 12 == 0 {
            self.exploration_direction = self.find_best_exploration_direction(world, robot);
        }

        let (dx, dy) = self.exploration_direction;

        if self.can_move(world, robot, dx, dy) {
            return self.move_direction(dx, dy);
        } else {
            // Blocked - find new direction
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
        let mut best_score = -100;

        for &(dx, dy) in &directions {
            let mut score = 0;

            // Look ahead in this direction
            for dist in 1..40 {
                let check_pos =
                    Position::new(robot.position.x + dx * dist, robot.position.y + dy * dist);

                // Penalize if blocked by known obstacles
                if self
                    .known_obstacles
                    .get(&(check_pos.x, check_pos.y))
                    .copied()
                    .unwrap_or(false)
                {
                    score -= 3;
                    if dist < 8 {
                        score -= 5; // Extra penalty for nearby obstacles
                    }
                    continue;
                }

                // Highly value unexplored tiles
                if !world.is_explored(&check_pos) {
                    score += 4;
                }

                // MASSIVELY value visible ice
                if let Some(TileType::Ice) = world.get_tile(&check_pos) {
                    score += 30;
                }
            }

            // Bonus if direction moves toward known ice
            for &ice_pos in &self.known_ice_locations {
                let ice_dx = ice_pos.x - robot.position.x;
                let ice_dy = ice_pos.y - robot.position.y;

                if (dx * ice_dx + dy * ice_dy) > 0 {
                    let ice_dist = robot.position.distance_to(&ice_pos);
                    if ice_dist < 40.0 {
                        score += 10;
                    }
                }
            }

            if score > best_score {
                best_score = score;
                best_dir = (dx, dy);
            }
        }

        // If no good direction found, spiral out from base
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

    fn find_nearest_ice(&self, world: &World, robot: &Robot, max_radius: i32) -> Option<Position> {
        let mut best_ice = None;
        let mut best_dist = f32::MAX;

        // Check known ice locations in memory
        for &ice_pos in &self.known_ice_locations {
            // Skip ice that we've repeatedly failed to reach
            let key = (ice_pos.x, ice_pos.y);
            if let Some(&fail_count) = self.unreachable_targets.get(&key) {
                if fail_count > 2 {
                    continue;
                }
            }

            let dist = robot.position.distance_to(&ice_pos);
            if dist < best_dist && dist > 0.1 && dist <= max_radius as f32 {
                if matches!(world.get_tile(&ice_pos), Some(TileType::Ice)) {
                    best_dist = dist;
                    best_ice = Some(ice_pos);
                }
            }
        }

        // Also scan immediate area for ice we might not have in memory
        for dy in -max_radius..=max_radius {
            for dx in -max_radius..=max_radius {
                if dx * dx + dy * dy > max_radius * max_radius {
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
        self.find_nearest_charger_from_position(world, &robot.position)
    }

    fn find_nearest_charger_from_position(&self, world: &World, pos: &Position) -> Position {
        let mut nearest = world.base_position;
        let mut nearest_dist = pos.distance_to(&world.base_position);

        for station in &world.charging_stations {
            if station.is_operational() {
                let dist = pos.distance_to(&station.position);
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
