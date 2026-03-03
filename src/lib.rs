mod game;

use game::input::InputHandler;
use game::Game;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, Window};

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    
    let canvas = document
        .get_element_by_id("canvas")
        .ok_or("no canvas element")?
        .dyn_into::<HtmlCanvasElement>()?;

    let context = canvas
        .get_context("2d")?
        .ok_or("no 2d context")?
        .dyn_into::<CanvasRenderingContext2d>()?;

    // Get canvas dimensions
    let canvas_width = canvas.width() as f64;
    let canvas_height = canvas.height() as f64;

    // Initialize input handler
    let input = InputHandler::new(&window)?;

    // Create game instance
    let game = Rc::new(RefCell::new(Game::new(canvas_width, canvas_height, input)));

    // Start game loop
    start_game_loop(window, context, game)?;

    Ok(())
}

fn start_game_loop(
    window: Window,
    context: CanvasRenderingContext2d,
    game: Rc<RefCell<Game>>,
) -> Result<(), JsValue> {
    let f: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move |timestamp: f64| {
        // Update game state
        game.borrow_mut().update(timestamp);

        // Render
        if let Err(e) = game.borrow().render(&context) {
            web_sys::console::error_1(&format!("Render error: {:?}", e).into());
        }

        // Request next frame
        if let Some(window) = web_sys::window() {
            window
                .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
                .ok();
        }
    }) as Box<dyn FnMut(f64)>));

    window.request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref())?;

    Ok(())
}
