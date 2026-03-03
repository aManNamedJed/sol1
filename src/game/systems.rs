use crate::game::robot::Robot;
use crate::game::types::TileType;
use crate::game::world::World;

pub struct GameSystems;

impl GameSystems {
    /// Update time progression
    pub fn update_time(world: &mut World, delta_time: f32) {
        // One full day-night cycle takes about 2 minutes (120 seconds)
        let time_speed = 1.0 / 120.0;
        world.advance_time(delta_time * time_speed);
    }

    /// Handle energy recharge at base or operational charging stations during daytime
    pub fn update_energy_recharge(world: &World, robot: &mut Robot, delta_time: f32) {
        if !world.is_day() {
            return;
        }

        let can_recharge = robot.position == world.base_position
            || world.is_charging_station_operational(&robot.position);

        if can_recharge {
            // Slow recharge during day at base or charging station
            let recharge_rate = 5.0; // 5 energy per second
            robot.recharge(recharge_rate * delta_time);
        }
    }

    /// Handle robot power-down state
    /// Returns true if game over (stranded), false otherwise
    pub fn handle_power_down(world: &mut World, robot: &mut Robot) -> bool {
        if !robot.powered_down {
            return false;
        }

        // Check if at a recharge point
        let at_recharge_point = robot.position == world.base_position
            || world.is_charging_station_operational(&robot.position);

        if !at_recharge_point {
            // Game over - robot is stranded
            return true;
        }

        // Safe location - advance to next day
        world.time_of_day = 0.0;
        world.day_count += 1;

        // Advance charging station timers
        for station in &mut world.charging_stations {
            station.advance_day();
        }

        // Subtle terraforming progress every 5 days
        if world.day_count % 5 == 0 {
            world.mars_health = (world.mars_health + 0.01).min(1.0);
        }

        // Restore full power
        robot.restore_full_power();
        
        false
    }

    /// Check if robot can move to a position
    pub fn can_move_to(world: &World, target: &crate::game::types::Position) -> bool {
        match world.get_tile(target) {
            Some(TileType::Rock) => false, // Can't move through rocks
            Some(_) => true,
            None => false, // Out of bounds
        }
    }

    /// Handle ice deposit and processing at base
    /// Returns number of ice samples deposited (0 if none)
    pub fn update_ice_processing(world: &mut World, robot: &mut Robot, delta_time: f32) -> u32 {
        let mut deposited = 0;
        
        // Deposit ice if at base and carrying samples
        if robot.position == world.base_position && robot.ice_samples > 0 {
            deposited = robot.ice_samples;
            world.ice_stored += robot.ice_samples;
            robot.ice_samples = 0;
        }

        // Process stored ice at base
        if world.ice_stored > 0 {
            world.process_ice(delta_time);
        }
        
        deposited
    }
}
