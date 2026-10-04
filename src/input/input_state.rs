//! Input State tracker capturing keyboard and mouse actions.

use std::collections::HashSet;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Debug, Default)]
pub struct InputState {
    pub pressed_keys: HashSet<KeyCode>,
    pub just_pressed_keys: HashSet<KeyCode>,
    pub is_dragging_mouse: bool,
    pub last_cursor_pos: Option<(f64, f64)>,
    pub mouse_drag_delta: (f32, f32),
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears transient one-frame events (just pressed keys, mouse delta).
    pub fn clear_frame_deltas(&mut self) {
        self.just_pressed_keys.clear();
        self.mouse_drag_delta = (0.0, 0.0);
    }

    /// Processes Winit window events and updates input state.
    pub fn process_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if let PhysicalKey::Code(key_code) = key_event.physical_key {
                    match key_event.state {
                        ElementState::Pressed => {
                            if !self.pressed_keys.contains(&key_code) {
                                self.just_pressed_keys.insert(key_code);
                            }
                            self.pressed_keys.insert(key_code);
                        }
                        ElementState::Released => {
                            self.pressed_keys.remove(&key_code);
                        }
                    }
                }
            }
            WindowEvent::MouseInput { button, state, .. } => {
                if *button == MouseButton::Left || *button == MouseButton::Right {
                    self.is_dragging_mouse = *state == ElementState::Pressed;
                    if !self.is_dragging_mouse {
                        self.last_cursor_pos = None;
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.is_dragging_mouse {
                    if let Some((last_x, last_y)) = self.last_cursor_pos {
                        let dx = (position.x - last_x) as f32;
                        let dy = (position.y - last_y) as f32;
                        self.mouse_drag_delta.0 += dx;
                        self.mouse_drag_delta.1 += dy;
                    }
                }
                self.last_cursor_pos = Some((position.x, position.y));
            }
            _ => {}
        }
    }

    /// Checks if a key is currently held down.
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.pressed_keys.contains(&key)
    }

    /// Checks if a key was pressed down in this frame.
    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.just_pressed_keys.contains(&key)
    }
}
