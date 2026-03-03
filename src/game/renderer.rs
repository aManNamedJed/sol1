#![allow(deprecated)]

use crate::game::game::ActionMessage;
use crate::game::robot::Robot;
use crate::game::types::{Color, Position, TileType};
use crate::game::world::World;
use web_sys::CanvasRenderingContext2d;

pub const TILE_SIZE: i32 = 16;

#[allow(deprecated)]
pub struct Renderer {
    canvas_width: f64,
    canvas_height: f64,
}

impl Renderer {
    pub fn new(canvas_width: f64, canvas_height: f64) -> Self {
        Self {
            canvas_width,
            canvas_height,
        }
    }

    pub fn render(
        &self,
        ctx: &CanvasRenderingContext2d,
        world: &World,
        robot: &Robot,
        messages: &[ActionMessage],
        game_over: bool,
        ai_enabled: bool,
    ) -> Result<(), wasm_bindgen::JsValue> {
        // Clear canvas
        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("#000000"));
        ctx.fill_rect(0.0, 0.0, self.canvas_width, self.canvas_height);

        // Calculate viewport
        let viewport = self.calculate_viewport(robot);

        // Render tiles
        self.render_tiles(ctx, world, robot, &viewport)?;

        // Render robot
        self.render_robot(ctx, robot, &viewport)?;

        // Apply day/night overlay
        self.render_day_night_overlay(ctx, world)?;

        // Render UI overlay
        self.render_ui(ctx, world, robot, messages, ai_enabled)?;

        // Render game over screen if needed
        if game_over {
            self.render_game_over(ctx, world)?;
        }

        Ok(())
    }

    fn calculate_viewport(&self, robot: &Robot) -> Viewport {
        let tiles_wide = (self.canvas_width / TILE_SIZE as f64).ceil() as i32 + 2;
        let tiles_high = (self.canvas_height / TILE_SIZE as f64).ceil() as i32 + 2;

        let center_x = robot.position.x;
        let center_y = robot.position.y;

        Viewport {
            start_x: center_x - tiles_wide / 2,
            start_y: center_y - tiles_high / 2,
            tiles_wide,
            tiles_high,
        }
    }

    fn render_tiles(
        &self,
        ctx: &CanvasRenderingContext2d,
        world: &World,
        robot: &Robot,
        viewport: &Viewport,
    ) -> Result<(), wasm_bindgen::JsValue> {
        const VISION_RADIUS: i32 = 6;

        for ty in 0..viewport.tiles_high {
            for tx in 0..viewport.tiles_wide {
                let world_x = viewport.start_x + tx;
                let world_y = viewport.start_y + ty;
                let pos = Position::new(world_x, world_y);

                // Check if tile is explored
                if !world.is_explored(&pos) {
                    // Unexplored: render as dark
                    let screen_x = (tx * TILE_SIZE) as f64;
                    let screen_y = (ty * TILE_SIZE) as f64;
                    ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(10, 10, 15, 1.0)"));
                    ctx.fill_rect(screen_x, screen_y, TILE_SIZE as f64, TILE_SIZE as f64);
                    continue;
                }

                // Check if tile is currently visible
                let dx = pos.x - robot.position.x;
                let dy = pos.y - robot.position.y;
                let dist_sq = dx * dx + dy * dy;
                let is_visible = dist_sq <= VISION_RADIUS * VISION_RADIUS;

                if let Some(tile_type) = world.get_tile(&pos) {
                    let color = self.get_tile_color(tile_type, world, &pos);
                    let screen_x = (tx * TILE_SIZE) as f64;
                    let screen_y = (ty * TILE_SIZE) as f64;

                    ctx.set_fill_style(&wasm_bindgen::JsValue::from_str(&color.to_rgba_string()));
                    ctx.fill_rect(screen_x, screen_y, TILE_SIZE as f64, TILE_SIZE as f64);

                    // Draw boot-up indicator for charging stations
                    if tile_type == TileType::ChargingStation && is_visible {
                        if let Some(station) = world.get_charging_station(&pos) {
                            if !station.is_operational() {
                                // Draw a small indicator showing days remaining
                                ctx.set_font("8px 'Courier New', monospace");
                                ctx.set_fill_style(&wasm_bindgen::JsValue::from_str(
                                    "rgba(255, 255, 255, 0.9)",
                                ));
                                let text = format!("{}", station.days_until_operational);
                                ctx.fill_text(&text, screen_x + 5.0, screen_y + 11.0)?;
                            }
                        }
                    }

                    // If explored but not visible, apply fog overlay
                    if !is_visible {
                        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(0, 0, 10, 0.6)"));
                        ctx.fill_rect(screen_x, screen_y, TILE_SIZE as f64, TILE_SIZE as f64);
                    }
                }
            }
        }
        Ok(())
    }

    fn get_tile_color(&self, tile_type: TileType, world: &World, pos: &Position) -> Color {
        let base_color = match tile_type {
            TileType::Regolith => Color::new(139, 69, 19, 1.0), // Mars red-brown
            TileType::Rock => Color::new(80, 80, 90, 1.0),      // Dark gray
            TileType::Ice => Color::new(200, 220, 255, 1.0),    // Light blue
            TileType::Base => Color::new(100, 120, 140, 1.0),   // Blue-gray
            TileType::ChargingStation => {
                // Check if operational
                if world.is_charging_station_operational(pos) {
                    Color::new(100, 200, 150, 1.0) // Green-ish when operational
                } else {
                    Color::new(120, 100, 60, 1.0) // Dim orange when booting
                }
            }
        };

        // Apply terraforming effect near base
        if tile_type == TileType::Regolith {
            let distance = pos.distance_to(&world.base_position);
            let terraform_range = 20.0;

            if distance < terraform_range {
                let influence = (1.0 - distance / terraform_range) * world.mars_health;
                let green_tint = Color::new(100, 140, 80, 1.0);
                return base_color.lerp(&green_tint, influence * 0.3);
            }
        }

        base_color
    }

    fn render_robot(
        &self,
        ctx: &CanvasRenderingContext2d,
        robot: &Robot,
        viewport: &Viewport,
    ) -> Result<(), wasm_bindgen::JsValue> {
        let screen_x = ((robot.position.x - viewport.start_x) * TILE_SIZE) as f64;
        let screen_y = ((robot.position.y - viewport.start_y) * TILE_SIZE) as f64;

        // Draw subtle vision radius indicator
        if !robot.powered_down {
            const VISION_RADIUS: f64 = 6.0;
            let radius_pixels = VISION_RADIUS * TILE_SIZE as f64;
            ctx.set_stroke_style(&wasm_bindgen::JsValue::from_str(
                "rgba(255, 200, 100, 0.15)",
            ));
            ctx.set_line_width(1.0);
            ctx.begin_path();
            ctx.arc(
                screen_x + (TILE_SIZE as f64 / 2.0),
                screen_y + (TILE_SIZE as f64 / 2.0),
                radius_pixels,
                0.0,
                std::f64::consts::PI * 2.0,
            )?;
            ctx.stroke();
        }

        // Robot body
        let robot_color = if robot.powered_down {
            Color::new(60, 60, 60, 0.8) // Dim gray when powered down
        } else {
            Color::new(255, 200, 100, 1.0) // Warm glow
        };

        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str(
            &robot_color.to_rgba_string(),
        ));
        ctx.fill_rect(
            screen_x + 2.0,
            screen_y + 2.0,
            (TILE_SIZE - 4) as f64,
            (TILE_SIZE - 4) as f64,
        );

        // Energy indicator (subtle border)
        if !robot.powered_down {
            let energy_ratio = robot.energy / robot.max_energy;
            let energy_color = if energy_ratio > 0.5 {
                Color::new(100, 255, 100, 0.6)
            } else if energy_ratio > 0.2 {
                Color::new(255, 200, 100, 0.6)
            } else {
                Color::new(255, 150, 100, 0.6)
            };

            ctx.set_stroke_style(&wasm_bindgen::JsValue::from_str(
                &energy_color.to_rgba_string(),
            ));
            ctx.set_line_width(2.0);
            ctx.stroke_rect(
                screen_x + 1.0,
                screen_y + 1.0,
                (TILE_SIZE - 2) as f64,
                (TILE_SIZE - 2) as f64,
            );
        }

        Ok(())
    }

    fn render_day_night_overlay(
        &self,
        ctx: &CanvasRenderingContext2d,
        world: &World,
    ) -> Result<(), wasm_bindgen::JsValue> {
        // Calculate darkness based on time of day
        let darkness = if world.time_of_day < 0.5 {
            // Day: minimal darkness
            0.0
        } else {
            // Night: gradual darkness from 0.5 to 1.0
            ((world.time_of_day - 0.5) * 2.0) * 0.6
        };

        if darkness > 0.01 {
            let overlay = Color::new(0, 0, 20, darkness);
            ctx.set_fill_style(&wasm_bindgen::JsValue::from_str(&overlay.to_rgba_string()));
            ctx.fill_rect(0.0, 0.0, self.canvas_width, self.canvas_height);
        }

        Ok(())
    }

    fn render_ui(
        &self,
        ctx: &CanvasRenderingContext2d,
        world: &World,
        robot: &Robot,
        messages: &[ActionMessage],
        ai_enabled: bool,
    ) -> Result<(), wasm_bindgen::JsValue> {
        let padding = 16.0;

        // Energy bar
        let bar_width = 150.0;
        let bar_height = 20.0;
        let energy_ratio = robot.energy / robot.max_energy;

        // Energy bar background
        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(0, 0, 0, 0.6)"));
        ctx.fill_rect(padding, padding, bar_width, bar_height);

        // Energy bar fill
        let energy_color = if energy_ratio > 0.5 {
            "rgba(100, 255, 100, 0.8)"
        } else if energy_ratio > 0.2 {
            "rgba(255, 200, 100, 0.8)"
        } else {
            "rgba(255, 150, 100, 0.8)"
        };
        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str(energy_color));
        ctx.fill_rect(
            padding + 2.0,
            padding + 2.0,
            (bar_width - 4.0) * energy_ratio as f64,
            bar_height - 4.0,
        );

        // Energy text
        ctx.set_font("12px 'Courier New', monospace");
        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(220, 220, 220, 0.9)"));
        let energy_text = format!("ENERGY: {:.0}/{:.0}", robot.energy, robot.max_energy);
        ctx.fill_text(&energy_text, padding + 4.0, padding + 14.0)?;

        // Day count
        let day_text = format!("SOL {}", world.day_count);
        ctx.set_font("14px 'Courier New', monospace");
        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(184, 136, 107, 0.9)"));
        ctx.fill_text(&day_text, padding, padding + bar_height + 24.0)?;

        // Time of day indicator
        let time_text = if world.is_day() {
            "☀ DAY"
        } else {
            "☾ NIGHT"
        };
        ctx.set_font("12px 'Courier New', monospace");
        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(180, 180, 180, 0.8)"));
        ctx.fill_text(time_text, padding, padding + bar_height + 42.0)?;

        // Ice samples and stored (if any)
        if robot.ice_samples > 0 || world.ice_stored > 0 {
            let ice_text = format!(
                "ICE: {} carried, {} stored",
                robot.ice_samples, world.ice_stored
            );
            ctx.set_font("11px 'Courier New', monospace");
            ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(150, 200, 255, 0.9)"));
            ctx.fill_text(&ice_text, padding, padding + bar_height + 60.0)?;
        }

        // Mars health indicator
        if world.mars_health > 0.01 {
            let terraform_text = format!("TERRAFORM: {:.0}%", world.mars_health * 100.0);
            ctx.set_font("11px 'Courier New', monospace");
            ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(100, 200, 120, 0.9)"));
            ctx.fill_text(&terraform_text, padding, padding + bar_height + 78.0)?;
        }

        // Sun/Moon dial (top right)
        self.render_day_night_dial(ctx, world)?;

        // AI status indicator (top right, below dial)
        if ai_enabled {
            let ai_x = self.canvas_width - 100.0;
            let ai_y = 90.0;

            // Background
            ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(0, 100, 200, 0.3)"));
            ctx.fill_rect(ai_x - 5.0, ai_y - 15.0, 95.0, 22.0);

            // Text
            ctx.set_font("bold 12px 'Courier New', monospace");
            ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(100, 200, 255, 1.0)"));
            ctx.fill_text("AI AUTOPILOT", ai_x, ai_y)?;
        }

        // Action messages (bottom left, fade out over time)
        if !messages.is_empty() {
            let msg_y_start = self.canvas_height - padding - 20.0;
            ctx.set_font("13px 'Courier New', monospace");

            for (i, msg) in messages.iter().rev().take(3).enumerate() {
                let y = msg_y_start - (i as f64 * 22.0);

                // Message background
                let text_width = 250.0; // Approximate width
                ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(0, 0, 0, 0.7)"));
                ctx.fill_rect(padding - 4.0, y - 16.0, text_width, 20.0);

                // Message text
                ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(200, 220, 200, 0.9)"));
                ctx.fill_text(&msg.text, padding, y)?;
            }
        }

        // Robot status (if powered down)
        if robot.powered_down {
            ctx.set_font("16px 'Courier New', monospace");
            ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(255, 200, 100, 0.9)"));
            let status_text = "POWERING DOWN...";
            let text_x = self.canvas_width / 2.0 - 80.0;
            let text_y = self.canvas_height / 2.0 - 50.0;

            ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(0, 0, 0, 0.8)"));
            ctx.fill_rect(text_x - 10.0, text_y - 20.0, 180.0, 30.0);

            ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(255, 200, 100, 0.9)"));
            ctx.fill_text(status_text, text_x, text_y)?;
        }

        Ok(())
    }

    fn render_day_night_dial(
        &self,
        ctx: &CanvasRenderingContext2d,
        world: &World,
    ) -> Result<(), wasm_bindgen::JsValue> {
        let dial_radius = 35.0;
        let dial_x = self.canvas_width - dial_radius - 20.0;
        let dial_y = dial_radius + 20.0;

        // Draw dial background (circular gradient representing day/night cycle)
        // We'll draw it as a circle with gradient fill
        let angle = (world.time_of_day as f64) * std::f64::consts::PI * 2.0;

        // Draw background circle
        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(20, 20, 30, 0.8)"));
        ctx.begin_path();
        ctx.arc(dial_x, dial_y, dial_radius, 0.0, std::f64::consts::PI * 2.0)?;
        ctx.fill();

        // Draw day/night gradient as two semicircles
        // Left half = day (blue), Right half = night (purple)
        // Angles: 0 = right, PI/2 = top, PI = left, 3*PI/2 = bottom
        
        // Day side (light blue) - left half
        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(135, 206, 235, 0.6)"));
        ctx.begin_path();
        ctx.arc(dial_x, dial_y, dial_radius - 3.0, std::f64::consts::PI / 2.0, 3.0 * std::f64::consts::PI / 2.0)?;
        ctx.close_path();
        ctx.fill();

        // Night side (dark purple) - right half
        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(50, 30, 80, 0.8)"));
        ctx.begin_path();
        ctx.arc(
            dial_x,
            dial_y,
            dial_radius - 3.0,
            -std::f64::consts::PI / 2.0,
            std::f64::consts::PI / 2.0,
        )?;
        ctx.close_path();
        ctx.fill();

        // Draw border
        ctx.set_stroke_style(&wasm_bindgen::JsValue::from_str("rgba(150, 150, 150, 0.6)"));
        ctx.set_line_width(2.0);
        ctx.begin_path();
        ctx.arc(dial_x, dial_y, dial_radius, 0.0, std::f64::consts::PI * 2.0)?;
        ctx.stroke();

        // Draw rotating sun/moon indicator
        let indicator_angle = angle + std::f64::consts::PI / 2.0; // Start from bottom (day)
        let indicator_x = dial_x + (dial_radius - 8.0) * indicator_angle.cos();
        let indicator_y = dial_y + (dial_radius - 8.0) * indicator_angle.sin();

        // Choose sun or moon based on time
        let icon_color = if world.is_day() {
            "rgba(255, 220, 100, 1.0)" // Sun yellow
        } else {
            "rgba(220, 220, 255, 1.0)" // Moon white
        };

        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str(icon_color));
        ctx.begin_path();
        ctx.arc(
            indicator_x,
            indicator_y,
            6.0,
            0.0,
            std::f64::consts::PI * 2.0,
        )?;
        ctx.fill();

        // Add glow for current time indicator
        ctx.set_stroke_style(&wasm_bindgen::JsValue::from_str(icon_color));
        ctx.set_line_width(1.5);
        ctx.begin_path();
        ctx.arc(
            indicator_x,
            indicator_y,
            8.0,
            0.0,
            std::f64::consts::PI * 2.0,
        )?;
        ctx.stroke();

        Ok(())
    }

    fn render_game_over(
        &self,
        ctx: &CanvasRenderingContext2d,
        world: &World,
    ) -> Result<(), wasm_bindgen::JsValue> {
        // Dark overlay
        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(0, 0, 0, 0.85)"));
        ctx.fill_rect(0.0, 0.0, self.canvas_width, self.canvas_height);

        let center_x = self.canvas_width / 2.0;
        let center_y = self.canvas_height / 2.0;

        // Game over title
        ctx.set_font("bold 32px 'Courier New', monospace");
        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(255, 100, 80, 1.0)"));
        ctx.set_text_align("center");
        ctx.fill_text("MISSION FAILED", center_x, center_y - 60.0)?;

        // Message
        ctx.set_font("16px 'Courier New', monospace");
        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(200, 200, 200, 0.9)"));
        ctx.fill_text("Robot stranded without power", center_x, center_y - 20.0)?;

        // Stats
        let stats_text = format!("Survived {} sols", world.day_count);
        ctx.set_font("14px 'Courier New', monospace");
        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(180, 180, 180, 0.8)"));
        ctx.fill_text(&stats_text, center_x, center_y + 20.0)?;

        // Instruction
        ctx.set_font("13px 'Courier New', monospace");
        ctx.set_fill_style(&wasm_bindgen::JsValue::from_str("rgba(150, 150, 150, 0.7)"));
        ctx.fill_text("Refresh page to try again", center_x, center_y + 60.0)?;

        // Reset text alignment
        ctx.set_text_align("start");

        Ok(())
    }
}

struct Viewport {
    start_x: i32,
    start_y: i32,
    tiles_wide: i32,
    tiles_high: i32,
}
