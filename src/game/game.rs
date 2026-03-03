use crate::game::ai::RobotAI;
use crate::game::input::{InputAction, InputHandler};
use crate::game::renderer::Renderer;
use crate::game::robot::Robot;
use crate::game::systems::GameSystems;
use crate::game::types::Position;
use crate::game::world::World;
use web_sys::CanvasRenderingContext2d;

pub struct ActionMessage {
    pub text: String,
    pub timestamp: f64,
}

pub struct Game {
    world: World,
    robot: Robot,
    renderer: Renderer,
    input: InputHandler,
    accumulator: f32,
    last_timestamp: f64,
    messages: Vec<ActionMessage>,
    game_over: bool,
    ai: RobotAI,
    ai_enabled: bool,
}

impl Game {
    const FIXED_TIMESTEP: f32 = 1.0 / 60.0; // 60 updates per second

    pub fn new(canvas_width: f64, canvas_height: f64, input: InputHandler) -> Self {
        let world = World::new();
        let robot = Robot::new(world.base_position);
        let renderer = Renderer::new(canvas_width, canvas_height);

        Self {
            world,
            robot,
            renderer,
            input,
            accumulator: 0.0,
            last_timestamp: 0.0,
            messages: Vec::new(),
            game_over: false,
            ai: RobotAI::new(),
            ai_enabled: false,
        }
    }

    pub fn update(&mut self, timestamp: f64) {
        // Calculate delta time
        let delta_time = if self.last_timestamp == 0.0 {
            Self::FIXED_TIMESTEP as f64
        } else {
            (timestamp - self.last_timestamp) / 1000.0 // Convert ms to seconds
        };
        self.last_timestamp = timestamp;

        // Fixed timestep update
        self.accumulator += delta_time as f32;

        while self.accumulator >= Self::FIXED_TIMESTEP {
            self.fixed_update(Self::FIXED_TIMESTEP);
            self.accumulator -= Self::FIXED_TIMESTEP;
        }
    }

    fn fixed_update(&mut self, delta_time: f32) {
        // Don't update if game is over
        if self.game_over {
            return;
        }

        // Clean up old messages (older than 3 seconds)
        let current_time = self.last_timestamp;
        self.messages
            .retain(|msg| current_time - msg.timestamp < 3000.0);

        // Handle player input
        if let Some(action) = self.input.get_action() {
            // Check for AI toggle
            if matches!(action, InputAction::ToggleAI) {
                self.ai_enabled = !self.ai_enabled;
                let status = if self.ai_enabled {
                    "enabled"
                } else {
                    "disabled"
                };
                self.add_message(format!("AI autopilot {}", status));
            } else if !self.ai_enabled {
                // Handle input only if AI is disabled
                self.handle_input(action);
            }
        }
        self.input.clear_just_pressed();

        // Handle AI decisions if enabled
        if self.ai_enabled && !self.robot.powered_down {
            if let Some(ai_action) = self
                .ai
                .decide_action(&self.world, &self.robot, current_time)
            {
                self.handle_input(ai_action);
            }
        }

        // Handle power-down state transitions
        let is_game_over = GameSystems::handle_power_down(&mut self.world, &mut self.robot);
        if is_game_over {
            self.game_over = true;
            self.add_message("Mission failed. Robot stranded.".to_string());
            return;
        }

        // Update time progression
        GameSystems::update_time(&mut self.world, delta_time);

        // Update energy recharge
        GameSystems::update_energy_recharge(&self.world, &mut self.robot, delta_time);

        // Update ice processing and deposits
        let deposited =
            GameSystems::update_ice_processing(&mut self.world, &mut self.robot, delta_time);
        if deposited > 0 {
            self.add_message(format!("Deposited {} ice samples at base", deposited));
        }

        // Update fog of war (mark visible tiles as explored)
        self.update_fog_of_war();
    }

    fn update_fog_of_war(&mut self) {
        const VISION_RADIUS: i32 = 6;
        let robot_pos = self.robot.position;

        for dy in -VISION_RADIUS..=VISION_RADIUS {
            for dx in -VISION_RADIUS..=VISION_RADIUS {
                let dist_sq = dx * dx + dy * dy;
                if dist_sq <= VISION_RADIUS * VISION_RADIUS {
                    let pos = Position::new(robot_pos.x + dx, robot_pos.y + dy);
                    self.world.mark_as_explored(&pos);
                }
            }
        }
    }

    fn handle_input(&mut self, action: InputAction) {
        match action {
            InputAction::MoveUp => self.try_move(0, -1),
            InputAction::MoveDown => self.try_move(0, 1),
            InputAction::MoveLeft => self.try_move(-1, 0),
            InputAction::MoveRight => self.try_move(1, 0),
            InputAction::Scan => {
                if self.robot.scan() {
                    let tile = self
                        .world
                        .get_tile(&self.robot.position)
                        .unwrap_or(crate::game::types::TileType::Regolith);
                    let tile_name = match tile {
                        crate::game::types::TileType::Regolith => "regolith",
                        crate::game::types::TileType::Rock => "rock formation",
                        crate::game::types::TileType::Ice => "ice deposit",
                        crate::game::types::TileType::Base => "base station",
                        crate::game::types::TileType::ChargingStation => {
                            if let Some(station) =
                                self.world.get_charging_station(&self.robot.position)
                            {
                                if station.is_operational() {
                                    "operational charging station"
                                } else {
                                    "charging station (booting...)"
                                }
                            } else {
                                "charging station"
                            }
                        }
                    };
                    self.add_message(format!("Scanning... detected {}", tile_name));
                } else {
                    self.add_message("Insufficient energy".to_string());
                }
            }
            InputAction::Collect => {
                if self.robot.collect() {
                    let tile = self
                        .world
                        .get_tile(&self.robot.position)
                        .unwrap_or(crate::game::types::TileType::Regolith);
                    match tile {
                        crate::game::types::TileType::Ice => {
                            self.robot.ice_samples += 1;
                            // Deplete the ice resource - turn it into regolith
                            self.world.set_tile(&self.robot.position, crate::game::types::TileType::Regolith);
                            self.add_message(format!(
                                "Collected ice sample (carrying {})",
                                self.robot.ice_samples
                            ));
                        }
                        _ => {
                            self.add_message("Nothing to collect here".to_string());
                        }
                    }
                } else {
                    self.add_message("Insufficient energy".to_string());
                }
            }
            InputAction::PlaceChargingStation => {
                const PLACEMENT_COST: f32 = 5.0;

                if self.robot.powered_down {
                    return;
                }

                if self.robot.energy < PLACEMENT_COST {
                    self.add_message("Insufficient energy to build".to_string());
                    return;
                }

                if self.world.place_charging_station(self.robot.position) {
                    self.robot.consume_energy(PLACEMENT_COST);
                    self.add_message("Charging station placed (boots in 1 day)".to_string());
                } else {
                    self.add_message("Cannot place station here".to_string());
                }
            }
            InputAction::ToggleAI => {
                // Handled in fixed_update, should not reach here
            }
        }
    }

    fn add_message(&mut self, text: String) {
        self.messages.push(ActionMessage {
            text,
            timestamp: self.last_timestamp,
        });
    }

    fn try_move(&mut self, dx: i32, dy: i32) {
        let new_pos = Position::new(self.robot.position.x + dx, self.robot.position.y + dy);

        if GameSystems::can_move_to(&self.world, &new_pos) {
            self.robot.move_to(new_pos);
        }
    }

    pub fn render(&self, ctx: &CanvasRenderingContext2d) -> Result<(), wasm_bindgen::JsValue> {
        self.renderer.render(
            ctx,
            &self.world,
            &self.robot,
            &self.messages,
            self.game_over,
            self.ai_enabled,
        )
    }

    #[allow(dead_code)]
    pub fn get_stats(&self) -> GameStats {
        GameStats {
            day: self.world.day_count,
            energy: self.robot.energy_percentage(),
            mars_health: (self.world.mars_health * 100.0).round(),
        }
    }
}

#[allow(dead_code)]
pub struct GameStats {
    pub day: u32,
    pub energy: f32,
    pub mars_health: f32,
}
