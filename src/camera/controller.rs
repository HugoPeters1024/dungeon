use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::window::CursorOptions;

/// Component for third-person camera controller
#[derive(Component)]
pub struct ThirdPersonCamera {
    /// Horizontal rotation (yaw) in radians
    pub yaw: f32,
    /// Vertical rotation (pitch) in radians
    pub pitch: f32,
    /// Target distance from player
    pub target_distance: f32,
    /// Current smoothed distance from player
    pub current_distance: f32,
    /// Height offset from player position
    pub height_offset: f32,
    /// Horizontal mouse sensitivity
    pub mouse_sensitivity_horizontal: f32,
    /// Vertical mouse sensitivity
    pub mouse_sensitivity_vertical: f32,
    /// Camera follow speed (higher = faster, more responsive)
    pub follow_speed: f32,
    /// Camera rotation smoothing speed
    pub rotation_smoothing: f32,
    /// Distance smoothing speed
    pub distance_smoothing: f32,
    /// Minimum distance from player
    pub min_distance: f32,
    /// Maximum distance from player
    pub max_distance: f32,
    /// Minimum pitch angle (looking down)
    pub min_pitch: f32,
    /// Maximum pitch angle (looking up)
    pub max_pitch: f32,
    /// Collision detection radius
    pub collision_radius: f32,
    /// Whether to enable collision detection
    pub enable_collision: bool,
}

impl Default for ThirdPersonCamera {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: -0.5, // Look slightly down
            target_distance: 3.5,
            current_distance: 3.5,
            height_offset: 2.0,
            mouse_sensitivity_horizontal: 0.003, // Increased for snappier feel
            mouse_sensitivity_vertical: 0.003,
            follow_speed: 12.0,       // Faster follow for more responsive feel
            rotation_smoothing: 90.0, // High value for very subtle smoothing - almost instant but smooth
            distance_smoothing: 6.0,
            min_distance: 1.0,
            max_distance: 8.0,
            min_pitch: -std::f32::consts::FRAC_PI_2 + 0.15,
            max_pitch: std::f32::consts::FRAC_PI_2 - 0.15,
            collision_radius: 0.3,
            enable_collision: true,
        }
    }
}

/// Handle mouse input for camera rotation
pub fn handle_mouse_look(
    mut cursor_options: Single<&mut CursorOptions>,
    mut camera_query: Query<&mut ThirdPersonCamera>,
    mut cursor_events: MessageReader<bevy::input::mouse::MouseMotion>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
) {
    let Ok(mut camera) = camera_query.single_mut() else {
        return;
    };

    // Collect mouse delta from events
    let mut delta = Vec2::ZERO;
    for event in cursor_events.read() {
        delta += event.delta;
    }

    // Lock cursor for better camera control
    if mouse.just_pressed(MouseButton::Left) && !keyboard.pressed(KeyCode::ControlRight) {
        cursor_options.grab_mode = bevy::window::CursorGrabMode::Locked;
        cursor_options.visible = false;
    }

    if keyboard.just_pressed(KeyCode::Escape) {
        cursor_options.grab_mode = bevy::window::CursorGrabMode::None;
        cursor_options.visible = true;
    }

    // Update camera rotation when cursor is locked
    if cursor_options.grab_mode == bevy::window::CursorGrabMode::Locked {
        camera.yaw -= delta.x * camera.mouse_sensitivity_horizontal;
        camera.pitch += delta.y * camera.mouse_sensitivity_vertical;

        // Clamp pitch to prevent flipping
        camera.pitch = camera.pitch.clamp(camera.min_pitch, camera.max_pitch);
    }
}

/// Update camera position with smooth interpolation and collision detection
pub fn update_camera_position(
    mut camera_query: Query<(&mut Transform, &mut ThirdPersonCamera)>,
    player_query: Query<
        (&Transform, &LinearVelocity),
        (
            With<bevy_tnua::prelude::TnuaController>,
            Without<ThirdPersonCamera>,
        ),
    >,
    time: Res<Time>,
) {
    let Ok((mut camera_transform, mut camera)) = camera_query.single_mut() else {
        return;
    };

    let Ok((player_transform, player_velocity)) = player_query.single() else {
        return;
    };

    let delta_time = time.delta_secs();

    // Calculate player position and velocity
    let player_pos = player_transform.translation;
    let player_speed = player_velocity.0.length();

    // Adjust target distance based on player speed (zoom out slightly when moving fast)
    // This creates a dynamic feel similar to Elden Ring
    let base_distance = 3.5;
    let speed_factor = (player_speed * 0.25).min(1.0);
    let dynamic_distance = base_distance + speed_factor * 0.3;
    camera.target_distance = dynamic_distance.clamp(camera.min_distance, camera.max_distance);

    // Smooth distance interpolation with exponential smoothing
    camera.current_distance = camera.current_distance.lerp(
        camera.target_distance,
        1.0 - (-delta_time * camera.distance_smoothing).exp(),
    );

    // Calculate desired camera position in spherical coordinates
    let horizontal_distance = camera.current_distance * camera.pitch.cos();
    let vertical_offset = camera.height_offset + camera.current_distance * camera.pitch.sin();

    let camera_offset = Vec3::new(
        camera.yaw.sin() * horizontal_distance,
        vertical_offset,
        camera.yaw.cos() * horizontal_distance,
    );

    let desired_camera_pos = player_pos + camera_offset;

    // For now, use desired position (collision detection can be added later with RayCaster component)
    let final_camera_pos = desired_camera_pos;

    // Smooth camera position interpolation (spring-like behavior)
    // Elden Ring-style camera lag: camera follows player smoothly but with slight delay
    let current_pos = camera_transform.translation;
    let target_pos = final_camera_pos;

    // Use exponential smoothing for smooth camera movement (like Elden Ring)
    // Higher follow speed = more responsive, lower = more cinematic lag
    let smoothing_factor = 1.0 - (-delta_time * camera.follow_speed).exp();
    let smoothed_pos = current_pos.lerp(target_pos, smoothing_factor);

    camera_transform.translation = smoothed_pos;

    // Calculate look target (slightly above player center for better framing)
    let look_target = player_pos + Vec3::Y * 1.2;

    // Very subtle rotation smoothing - fast enough to feel instant but smooths micro-jitters
    let target_rotation = Transform::from_translation(smoothed_pos)
        .looking_at(look_target, Vec3::Y)
        .rotation;

    // High smoothing factor makes it nearly instant but still smooth
    let rotation_smoothing_factor = 1.0 - (-delta_time * camera.rotation_smoothing).exp();
    camera_transform.rotation = camera_transform
        .rotation
        .slerp(target_rotation, rotation_smoothing_factor);
}
