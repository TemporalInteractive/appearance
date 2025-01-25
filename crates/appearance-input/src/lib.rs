use glam::Vec2;
use winit::{
    event::{DeviceEvent, ElementState, MouseButton, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

#[derive(Debug, Clone)]
struct InputState {
    keys: [bool; 512],
    mouse_buttons: [bool; 32],
    mouse_position: Vec2,
    mouse_motion: Vec2,
    mouse_wheel: f32,
}

#[derive(Debug, Default)]
pub struct InputHandler {
    state: InputState,
    previous_state: InputState,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            keys: [false; 512],
            mouse_buttons: [false; 32],
            mouse_position: Vec2::ZERO,
            mouse_motion: Vec2::ZERO,
            mouse_wheel: 0.0,
        }
    }
}

impl InputHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn key(&self, key_code: KeyCode) -> bool {
        self.state.keys[key_code as usize]
    }

    pub fn key_down(&self, key_code: KeyCode) -> bool {
        self.state.keys[key_code as usize] && !self.previous_state.keys[key_code as usize]
    }

    pub fn mouse_button(&self, button: MouseButton) -> bool {
        self.state.mouse_buttons[mouse_button_to_usize(&button)]
    }

    pub fn mouse_button_down(&self, button: MouseButton) -> bool {
        self.state.mouse_buttons[mouse_button_to_usize(&button)]
            && !self.previous_state.mouse_buttons[mouse_button_to_usize(&button)]
    }

    pub fn mouse_position(&self) -> Vec2 {
        self.state.mouse_position
    }

    pub fn mouse_position_delta(&self) -> Vec2 {
        self.state.mouse_position - self.previous_state.mouse_position
    }

    pub fn mouse_motion(&self) -> Vec2 {
        self.state.mouse_motion
    }

    pub fn mouse_wheel(&self) -> f32 {
        self.state.mouse_wheel
    }

    pub fn mouse_wheel_delta(&self) -> f32 {
        self.state.mouse_wheel - self.previous_state.mouse_wheel
    }

    pub fn handle_window_input(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, _) => *x,
                    winit::event::MouseScrollDelta::PixelDelta(x) => x.x as f32,
                };
                self.state.mouse_wheel += delta;
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.state.mouse_position = Vec2::new(position.x as f32, position.y as f32);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if !event.repeat {
                    if let PhysicalKey::Code(key_code) = event.physical_key {
                        self.state.keys[key_code as usize] = event.state == ElementState::Pressed;
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.state.mouse_buttons[mouse_button_to_usize(button)] =
                    *state == ElementState::Pressed;
            }
            _ => {}
        }
    }

    pub fn handle_device_input(&mut self, device_event: &DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = device_event {
            self.state.mouse_motion += Vec2::new(delta.0 as f32, delta.1 as f32);
        }
    }

    pub fn update(&mut self) {
        self.previous_state = self.state.clone();
        self.state.mouse_motion = Vec2::ZERO;
    }
}

fn mouse_button_to_usize(button: &MouseButton) -> usize {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
        MouseButton::Back => 3,
        MouseButton::Forward => 4,
        MouseButton::Other(i) => 4 + *i as usize,
    }
}
