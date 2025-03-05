use crate::renderer::types::camera::Camera;
use glam::{Mat4, Vec3, vec3};
use winit::event::KeyEvent;
use winit::keyboard::{KeyCode, PhysicalKey};

/// A combined FPS camera + controller in one struct.
///
/// This lumps together:
///  - Camera parameters (position, yaw/pitch, perspective)
///  - Movement input states (which keys are pressed)
///  - Mouse look logic
pub struct FpsCamera {
    // Camera parameters
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,

    // Perspective
    pub fovy: f32,
    pub aspect: f32,
    pub znear: f32,
    pub zfar: f32,

    // Movement + mouse config
    pub speed: f32,
    pub mouse_sensitivity: f32,

    // Key states
    pub forward_pressed: bool,
    pub backward_pressed: bool,
    pub left_pressed: bool,
    pub right_pressed: bool,
    pub up_pressed: bool,
    pub down_pressed: bool,

    // For accumulating mouse deltas
    pub yaw_delta: f32,
    pub pitch_delta: f32,
}

impl FpsCamera {
    /// Create a new combined camera + controller
    ///
    /// fovy_degs is FOV in degrees for convenience.
    pub fn new(
        position: Vec3,
        yaw_degs: f32,
        pitch_degs: f32,
        fovy_degs: f32,
        aspect: f32,
        znear: f32,
        zfar: f32,
        speed: f32,
        mouse_sensitivity: f32,
    ) -> Self {
        Self {
            position,
            yaw: yaw_degs.to_radians(),
            pitch: pitch_degs.to_radians(),
            fovy: fovy_degs.to_radians(),
            aspect,
            znear,
            zfar,

            speed,
            mouse_sensitivity,

            forward_pressed: false,
            backward_pressed: false,
            left_pressed: false,
            right_pressed: false,
            up_pressed: false,
            down_pressed: false,

            yaw_delta: 0.0,
            pitch_delta: 0.0,
        }
    }

    /// Handle keyboard press/release events.
    /// For example, call this from your winit event handler:
    ///
    /// ```
    /// if let Some(key) = input.virtual_keycode {
    ///     let pressed = input.state == ElementState::Pressed;
    ///     camera.process_keyboard(key, pressed);
    /// }
    /// ```
    pub fn process_keyboard(&mut self, key: KeyEvent) {
        let pressed = key.state == winit::event::ElementState::Pressed;
        let key = key.physical_key;
        let key = match key {
            PhysicalKey::Code(key) => key,
            _ => return,
        };
        match key {
            KeyCode::KeyW => self.forward_pressed = pressed,
            KeyCode::KeyS => self.backward_pressed = pressed,
            KeyCode::KeyA => self.left_pressed = pressed,
            KeyCode::KeyD => self.right_pressed = pressed,
            KeyCode::Space => self.up_pressed = pressed,
            KeyCode::ShiftLeft => self.down_pressed = pressed,
            _ => {}
        }
    }

    /// Handle mouse movement deltas.
    /// Typically you'd call this from DeviceEvent::MouseMotion in winit:
    ///
    /// ```
    /// if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
    ///     camera.process_mouse(dx as f32, dy as f32);
    /// }
    /// ```
    pub fn process_mouse(&mut self, dx: f32, dy: f32) {
        self.yaw_delta += dx * self.mouse_sensitivity;
        self.pitch_delta += -dy * self.mouse_sensitivity; // Usually invert Y
    }

    /// Update camera each frame. `dt` is elapsed time (seconds).
    pub fn update(&mut self, dt: f32) {
        // 1) Apply yaw/pitch deltas
        self.yaw += self.yaw_delta;
        self.pitch += self.pitch_delta;
        self.yaw_delta = 0.0;
        self.pitch_delta = 0.0;

        // Clamp pitch to avoid flipping upside down
        let max_pitch = std::f32::consts::FRAC_PI_2 - 0.001;
        if self.pitch > max_pitch {
            self.pitch = max_pitch;
        }
        if self.pitch < -max_pitch {
            self.pitch = -max_pitch;
        }

        // 2) Compute forward/right vectors from yaw/pitch
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();

        let forward = vec3(cos_pitch * sin_yaw, sin_pitch, cos_pitch * cos_yaw);
        let world_up = Vec3::Y;
        let right = forward.cross(world_up).normalize();

        // 3) Handle movement
        let mut velocity = Vec3::ZERO;
        if self.forward_pressed {
            velocity += forward;
        }
        if self.backward_pressed {
            velocity -= forward;
        }
        if self.right_pressed {
            velocity += right;
        }
        if self.left_pressed {
            velocity -= right;
        }
        if self.up_pressed {
            velocity += world_up;
        }
        if self.down_pressed {
            velocity -= world_up;
        }

        if velocity.length_squared() > 0.0 {
            velocity = velocity.normalize() * self.speed * dt;
        }
        self.position += velocity;
    }

    /// Build the combined view-projection matrix
    pub fn build_vp(&self) -> Mat4 {
        let view = self.build_view_matrix();
        let proj = Mat4::perspective_rh_gl(self.fovy, self.aspect, self.znear, self.zfar);
        proj * view
    }

    /// If the window size changes, update aspect ratio
    pub fn set_aspect(&mut self, aspect: f32) {
        self.aspect = aspect;
    }

    /// Build just the view (camera) matrix
    fn build_view_matrix(&self) -> Mat4 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();

        let forward = vec3(cos_pitch * sin_yaw, sin_pitch, cos_pitch * cos_yaw).normalize();

        let global_up = Vec3::Y;
        let right = forward.cross(global_up).normalize();
        let up = right.cross(forward).normalize();

        Mat4::look_at_rh(self.position, self.position + forward, up)
    }
}

impl Camera for FpsCamera {
    fn build_view_projection_matrix(&self) -> Mat4 {
        self.build_vp()
    }
}
