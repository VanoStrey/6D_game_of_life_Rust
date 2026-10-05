//! Input State tracker capturing keyboard and mouse actions.

use std::collections::HashSet;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Debug, Default)]
pub struct InputState {
    pub pressed_keys: HashSet<KeyCode>,
    pub just_pressed_keys: HashSet<KeyCode>,
    pub is_mouse_captured: bool,
    pub is_dragging_mouse: bool,
    pub last_cursor_pos: Option<(f64, f64)>,
    pub mouse_drag_delta: (f32, f32),
    pub mouse_motion_delta: (f32, f32),
    pub cursor_moved_delta: (f32, f32),
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears transient one-frame events (just pressed keys, mouse deltas).
    pub fn clear_frame_deltas(&mut self) {
        self.just_pressed_keys.clear();
        self.mouse_drag_delta = (0.0, 0.0);
        self.mouse_motion_delta = (0.0, 0.0);
        self.cursor_moved_delta = (0.0, 0.0);
    }

    /// Processes raw Winit DeviceEvent (unaccelerated mouse motion for camera look).
    pub fn process_device_event(&mut self, event: &DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            if self.is_mouse_captured {
                self.mouse_motion_delta.0 += *dx as f32;
                self.mouse_motion_delta.1 += *dy as f32;
            }
        }
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
                if let Some((last_x, last_y)) = self.last_cursor_pos {
                    let dx = (position.x - last_x) as f32;
                    let dy = (position.y - last_y) as f32;
                    if self.is_mouse_captured {
                        self.cursor_moved_delta.0 += dx;
                        self.cursor_moved_delta.1 += dy;
                    } else if self.is_dragging_mouse {
                        self.mouse_drag_delta.0 += dx;
                        self.mouse_drag_delta.1 += dy;
                    }
                }
                self.last_cursor_pos = Some((position.x, position.y));
            }
            _ => {}
        }
    }

    /// Returns the effective mouse rotation delta for this frame.
    /// Prefers raw hardware DeviceEvent::MouseMotion when captured, falling back to cursor delta.
    pub fn effective_mouse_delta(&self) -> (f32, f32) {
        if self.is_mouse_captured {
            if self.mouse_motion_delta.0 != 0.0 || self.mouse_motion_delta.1 != 0.0 {
                self.mouse_motion_delta
            } else {
                self.cursor_moved_delta
            }
        } else {
            self.mouse_drag_delta
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

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::PhysicalPosition;

    #[test]
    fn test_mouse_capture_motion_accumulation() {
        let mut input = InputState::new();
        assert!(!input.is_mouse_captured);

        // When not captured, DeviceEvent::MouseMotion should NOT accumulate
        input.process_device_event(&DeviceEvent::MouseMotion {
            delta: (15.0, -10.0),
        });
        assert_eq!(input.mouse_motion_delta, (0.0, 0.0));
        assert_eq!(input.effective_mouse_delta(), (0.0, 0.0));

        // When captured, DeviceEvent::MouseMotion should accumulate freely
        input.is_mouse_captured = true;
        input.process_device_event(&DeviceEvent::MouseMotion {
            delta: (12.5, -8.5),
        });
        input.process_device_event(&DeviceEvent::MouseMotion {
            delta: (-2.5, 4.0),
        });

        assert_eq!(input.mouse_motion_delta, (10.0, -4.5));
        assert_eq!(input.effective_mouse_delta(), (10.0, -4.5));

        // Clear frame deltas resets the motion
        input.clear_frame_deltas();
        assert_eq!(input.mouse_motion_delta, (0.0, 0.0));
        assert_eq!(input.effective_mouse_delta(), (0.0, 0.0));
    }

    #[test]
    fn test_cursor_moved_fallback_when_captured() {
        let mut input = InputState::new();
        input.is_mouse_captured = true;

        // First cursor moved sets initial position
        input.process_event(&WindowEvent::CursorMoved {
            device_id: unsafe { std::mem::zeroed() },
            position: PhysicalPosition::new(100.0, 100.0),
        });
        assert_eq!(input.cursor_moved_delta, (0.0, 0.0));

        // Subsequent cursor moved updates delta
        input.process_event(&WindowEvent::CursorMoved {
            device_id: unsafe { std::mem::zeroed() },
            position: PhysicalPosition::new(140.0, 120.0),
        });
        assert_eq!(input.cursor_moved_delta, (40.0, 20.0));
        // Fallback takes effect because mouse_motion_delta is (0, 0)
        assert_eq!(input.effective_mouse_delta(), (40.0, 20.0));

        // But if DeviceEvent::MouseMotion arrives, it takes priority!
        input.process_device_event(&DeviceEvent::MouseMotion { delta: (5.0, 3.0) });
        assert_eq!(input.effective_mouse_delta(), (5.0, 3.0));
    }

    #[test]
    fn test_normal_mouse_drag_when_not_captured() {
        let mut input = InputState::new();
        input.is_mouse_captured = false;

        // Move cursor without button pressed: no drag delta
        input.process_event(&WindowEvent::CursorMoved {
            device_id: unsafe { std::mem::zeroed() },
            position: PhysicalPosition::new(50.0, 50.0),
        });
        input.process_event(&WindowEvent::CursorMoved {
            device_id: unsafe { std::mem::zeroed() },
            position: PhysicalPosition::new(60.0, 70.0),
        });
        assert_eq!(input.mouse_drag_delta, (0.0, 0.0));
        assert_eq!(input.effective_mouse_delta(), (0.0, 0.0));

        // Press Left button: drag begins
        input.process_event(&WindowEvent::MouseInput {
            device_id: unsafe { std::mem::zeroed() },
            state: ElementState::Pressed,
            button: MouseButton::Left,
        });
        input.process_event(&WindowEvent::CursorMoved {
            device_id: unsafe { std::mem::zeroed() },
            position: PhysicalPosition::new(80.0, 95.0),
        });
        assert_eq!(input.mouse_drag_delta, (20.0, 25.0));
        assert_eq!(input.effective_mouse_delta(), (20.0, 25.0));
    }
}
