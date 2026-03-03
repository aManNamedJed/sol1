use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{KeyboardEvent, Window};

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum InputAction {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Collect,
    PlaceChargingStation,
    ToggleAI,
}

#[allow(dead_code)]
pub struct InputHandler {
    keys_pressed: Rc<RefCell<[bool; 256]>>,
    keys_just_pressed: Rc<RefCell<[bool; 256]>>,
}

impl InputHandler {
    pub fn new(window: &Window) -> Result<Self, JsValue> {
        let keys_pressed = Rc::new(RefCell::new([false; 256]));
        let keys_just_pressed = Rc::new(RefCell::new([false; 256]));

        // Set up keydown listener
        {
            let keys_pressed = keys_pressed.clone();
            let keys_just_pressed = keys_just_pressed.clone();
            let closure = Closure::wrap(Box::new(move |event: KeyboardEvent| {
                if let Some(key_code) = Self::map_key_code(&event.key()) {
                    let mut pressed = keys_pressed.borrow_mut();
                    let mut just_pressed = keys_just_pressed.borrow_mut();

                    if !pressed[key_code] {
                        just_pressed[key_code] = true;
                    }
                    pressed[key_code] = true;

                    event.prevent_default();
                }
            }) as Box<dyn FnMut(_)>);

            window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
            closure.forget();
        }

        // Set up keyup listener
        {
            let keys_pressed = keys_pressed.clone();
            let closure = Closure::wrap(Box::new(move |event: KeyboardEvent| {
                if let Some(key_code) = Self::map_key_code(&event.key()) {
                    let mut pressed = keys_pressed.borrow_mut();
                    pressed[key_code] = false;
                }
            }) as Box<dyn FnMut(_)>);

            window.add_event_listener_with_callback("keyup", closure.as_ref().unchecked_ref())?;
            closure.forget();
        }

        Ok(Self {
            keys_pressed,
            keys_just_pressed,
        })
    }

    pub fn get_action(&self) -> Option<InputAction> {
        let just_pressed = self.keys_just_pressed.borrow();

        if just_pressed[Self::KEY_UP] {
            Some(InputAction::MoveUp)
        } else if just_pressed[Self::KEY_DOWN] {
            Some(InputAction::MoveDown)
        } else if just_pressed[Self::KEY_LEFT] {
            Some(InputAction::MoveLeft)
        } else if just_pressed[Self::KEY_RIGHT] {
            Some(InputAction::MoveRight)
        } else if just_pressed[Self::KEY_E] {
            Some(InputAction::Collect)
        } else if just_pressed[Self::KEY_B] {
            Some(InputAction::PlaceChargingStation)
        } else if just_pressed[Self::KEY_A] {
            Some(InputAction::ToggleAI)
        } else {
            None
        }
    }

    pub fn clear_just_pressed(&self) {
        let mut just_pressed = self.keys_just_pressed.borrow_mut();
        for key in just_pressed.iter_mut() {
            *key = false;
        }
    }

    fn map_key_code(key: &str) -> Option<usize> {
        match key {
            "ArrowUp" => Some(Self::KEY_UP),
            "ArrowDown" => Some(Self::KEY_DOWN),
            "ArrowLeft" => Some(Self::KEY_LEFT),
            "ArrowRight" => Some(Self::KEY_RIGHT),
            "e" | "E" => Some(Self::KEY_E),
            "b" | "B" => Some(Self::KEY_B),
            "a" | "A" => Some(Self::KEY_A),
            _ => None,
        }
    }

    const KEY_UP: usize = 0;
    const KEY_DOWN: usize = 1;
    const KEY_LEFT: usize = 2;
    const KEY_RIGHT: usize = 3;
    const KEY_E: usize = 4;
    const KEY_B: usize = 5;
    const KEY_A: usize = 6;
}
