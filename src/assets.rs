use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_hanabi::prelude::*;
use bevy_kira_audio::AudioSource;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum MyStates {
    #[default]
    AssetLoading,
    AssetPreparing,
    Next,
}

#[derive(Resource, AssetCollection)]
pub struct GameAssets {
    #[asset(path = "stones.png")]
    #[asset(image(sampler(filter = linear, wrap = repeat)))]
    pub stones: Handle<Image>,

    #[asset(path = "mossy_stones.png")]
    #[asset(image(sampler(filter = linear, wrap = repeat)))]
    pub mossy_stones: Handle<Image>,

    #[asset(path = "outside_grass.jpg")]
    #[asset(image(sampler(filter = linear, wrap = repeat)))]
    pub outside_grass: Handle<Image>,

    #[asset(path = "lava.jpg")]
    #[asset(image(sampler(filter = linear, wrap = repeat)))]
    pub lava: Handle<Image>,

    #[asset(path = "stairs.glb#Mesh0/Primitive0")]
    pub stairs: Handle<Mesh>,

    #[asset(path = "wineglass.glb#Mesh0/Primitive0")]
    pub wineglass: Handle<Mesh>,

    #[asset(path = "wineglass.glb#Material0")]
    pub wineglass_material: Handle<StandardMaterial>,

    #[asset(path = "trophy.glb#Mesh0/Primitive0")]
    pub trophy: Handle<Mesh>,

    #[asset(path = "trophy.glb#Material0")]
    pub trophy_material: Handle<StandardMaterial>,

    #[asset(path = "bong.glb#Mesh0/Primitive0")]
    pub bong: Handle<Mesh>,

    #[asset(path = "bong.glb#Material0")]
    pub bong_material: Handle<StandardMaterial>,

    #[asset(path = "sword.glb#Scene0")]
    pub sword: Handle<Scene>,

    #[asset(path = "torch.glb#Scene0")]
    pub torch: Handle<Scene>,

    #[asset(path = "player.glb#Scene0")]
    pub player: Handle<Scene>,

    #[asset(
        paths(
            "player.glb#Animation0",
            "player.glb#Animation1",
            "player.glb#Animation2",
            "player.glb#Animation3",
            "player.glb#Animation4",
            "player.glb#Animation5",
            "player.glb#Animation6",
            "player.glb#Animation7",
            "player.glb#Animation8",
        ),
        collection(typed)
    )]
    pub player_clips: Vec<Handle<AnimationClip>>,

    pub fire: Handle<EffectAsset>,
    pub void: Handle<EffectAsset>,
    pub golden_pickup: Handle<EffectAsset>,

    #[asset(path = "fall.ogg")]
    pub fall: Handle<AudioSource>,

    #[asset(path = "pickup.mp3")]
    pub pickup: Handle<AudioSource>,

    #[asset(path = "bones-snap.mp3")]
    pub death: Handle<AudioSource>,
}

pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<MyStates>()
            .add_loading_state(
                LoadingState::new(MyStates::AssetLoading)
                    .continue_to_state(MyStates::AssetPreparing)
                    .load_collection::<GameAssets>(),
            )
            .add_systems(OnEnter(MyStates::AssetPreparing), prepare_assets);
    }
}

fn prepare_assets(
    mut assets: ResMut<GameAssets>,
    mut effects: ResMut<Assets<EffectAsset>>,
    mut state: ResMut<NextState<MyStates>>,
) {
    assets.fire = create_fire_effect(&mut effects);
    assets.void = create_void_effect(&mut effects);
    assets.golden_pickup = create_golden_pickup_effect(&mut effects);

    state.set(MyStates::Next);
}

/// Create a fire particle effect
fn create_fire_effect(effects: &mut ResMut<Assets<EffectAsset>>) -> Handle<EffectAsset> {
    // More realistic fire color gradient:
    // - White/yellow hot core at base (intense heat)
    // - Orange/yellow in the middle (main flame)
    // - Red-orange as it cools
    // - Dark red/black smoke as it fades
    let mut color_gradient = bevy_hanabi::Gradient::new();
    color_gradient.add_key(0.0, Vec4::new(1.0, 1.0, 0.9, 1.0)); // White-hot core
    color_gradient.add_key(0.15, Vec4::new(1.0, 0.95, 0.5, 1.0)); // Bright yellow
    color_gradient.add_key(0.35, Vec4::new(1.0, 0.7, 0.2, 0.7)); // Orange-yellow
    color_gradient.add_key(0.55, Vec4::new(1.0, 0.4, 0.1, 0.55)); // Orange
    color_gradient.add_key(0.75, Vec4::new(0.8, 0.2, 0.05, 0.3)); // Red-orange
    color_gradient.add_key(0.9, Vec4::new(0.4, 0.1, 0.0, 0.1)); // Dark red
    color_gradient.add_key(1.0, Vec4::new(0.1, 0.05, 0.0, 0.1)); // Black smoke, fully transparent

    // Realistic size gradient: particles expand as hot air rises, then shrink as they cool
    let mut size_gradient = bevy_hanabi::Gradient::new();
    size_gradient.add_key(0.0, Vec3::splat(0.03)); // Small at base
    size_gradient.add_key(0.2, Vec3::splat(0.06)); // Growing
    size_gradient.add_key(0.5, Vec3::splat(0.12)); // Peak size (expanding hot air)
    size_gradient.add_key(0.8, Vec3::splat(0.08)); // Shrinking as cooling
    size_gradient.add_key(1.0, Vec3::splat(0.02)); // Very small at end

    let writer = ExprWriter::new();

    // Initialize particles with random position in a small circle
    let age = writer.lit(0.).expr();
    let init_age = SetAttributeModifier::new(Attribute::AGE, age);

    // More varied lifetime for natural fire behavior
    let lifetime = writer.lit(0.6).uniform(writer.lit(0.4)).expr();
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, lifetime);

    // Spawn particles in a small circle at the base (fire source)
    let init_pos = SetPositionCircleModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        axis: writer.lit(Vec3::Y).expr(),
        radius: writer.lit(0.06).expr(), // Slightly larger spawn area
        dimension: ShapeDimension::Surface,
    };

    // More realistic initial velocity:
    // - Strong upward component (hot air rises)
    // - More horizontal variation for flickering/turbulence
    // - Variable upward speed for natural variation
    let random_x = writer.lit(-0.4).uniform(writer.lit(0.4)); // More horizontal turbulence
    let random_z = writer.lit(-0.4).uniform(writer.lit(0.4));
    let upward_speed = writer.lit(2.0).uniform(writer.lit(1.0)); // Variable upward speed
    let velocity = random_x.vec3(upward_speed, random_z);
    let init_vel = SetAttributeModifier::new(Attribute::VELOCITY, velocity.expr());

    // Add upward acceleration (buoyancy) - hot air accelerates upward
    let accel = writer.lit(Vec3::new(0.0, 2.3, 0.0)).expr();
    let update_accel = AccelModifier::new(accel);

    // Add drag to simulate air resistance (less drag = particles rise higher)
    let drag = writer.lit(0.6).expr(); // Reduced drag for more buoyant effect
    let update_drag = LinearDragModifier::new(drag);

    effects.add(
        EffectAsset::new(
            32768,
            SpawnerSettings::rate(80.0.into()), // Higher spawn rate for denser fire
            writer.finish(),
        )
        .with_name("fire")
        .init(init_pos)
        .init(init_vel)
        .init(init_age)
        .init(init_lifetime)
        .update(update_accel) // Buoyancy
        .update(update_drag) // Air resistance
        .render(ColorOverLifetimeModifier {
            gradient: color_gradient,
            blend: ColorBlendMode::Modulate, // Modulate blending (Additive not available in this version)
            mask: ColorBlendMask::RGBA,
        })
        .render(SizeOverLifetimeModifier {
            gradient: size_gradient,
            screen_space_size: false,
        })
        .render(OrientModifier {
            mode: OrientMode::FaceCameraPosition,
            rotation: None,
        }),
    )
}

/// Create a void-like background particle effect with slow-moving white particles
fn create_void_effect(effects: &mut ResMut<Assets<EffectAsset>>) -> Handle<EffectAsset> {
    let mut color_gradient = bevy_hanabi::Gradient::new();
    color_gradient.add_key(0.0, Vec4::new(1.0, 1.0, 1.0, 0.0)); // Start invisible (fade in)
    color_gradient.add_key(0.1, Vec4::new(1.0, 1.0, 1.0, 0.15)); // Fade in quickly
    color_gradient.add_key(0.3, Vec4::new(1.0, 1.0, 1.0, 0.3)); // Full visibility
    color_gradient.add_key(0.7, Vec4::new(1.0, 1.0, 1.0, 0.2)); // Start fading out
    color_gradient.add_key(1.0, Vec4::new(1.0, 1.0, 1.0, 0.0)); // Fade out completely

    let mut size_gradient = bevy_hanabi::Gradient::new();
    size_gradient.add_key(0.0, Vec3::splat(0.08)); // Small particles
    size_gradient.add_key(1.0, Vec3::splat(0.16)); // Slightly larger over time

    let writer = ExprWriter::new();

    // Initialize particles with random position in a large volume (background area)
    let age = writer.lit(0.).expr();
    let init_age = SetAttributeModifier::new(Attribute::AGE, age);

    // Long lifetime for slow-moving particles
    let lifetime = writer.lit(8.0).uniform(writer.lit(12.0)).expr();
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, lifetime);

    // Spawn particles in a large volume around the scene
    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(), // Center relative to particle effect transform
        radius: writer.lit(125.0).expr(),
        dimension: ShapeDimension::Volume,
    };

    // Random velocity in all directions (void-like drift) - increased velocity
    const VEL: f32 = 3.0;
    let random_x = writer.lit(-VEL).uniform(writer.lit(VEL));
    let random_y = writer.lit(-VEL).uniform(writer.lit(VEL));
    let random_z = writer.lit(-VEL).uniform(writer.lit(VEL));
    let velocity = random_x.vec3(random_y, random_z);
    let init_vel = SetAttributeModifier::new(Attribute::VELOCITY, velocity.expr());

    let drag = writer.lit(0.1).expr();
    let update_drag = LinearDragModifier::new(drag);

    effects.add(
        EffectAsset::new(
            32768,                              // Increased particle capacity for more particles
            SpawnerSettings::rate(60.0.into()), // Higher spawn rate for more particles
            writer.finish(),
        )
        .with_name("void")
        .init(init_pos)
        .init(init_vel)
        .init(init_age)
        .init(init_lifetime)
        .update(update_drag)
        .render(ColorOverLifetimeModifier {
            gradient: color_gradient,
            blend: ColorBlendMode::Modulate, // Modulate blending for void particles
            mask: ColorBlendMask::RGBA,
        })
        .render(SizeOverLifetimeModifier {
            gradient: size_gradient,
            screen_space_size: false,
        }),
    )
}

/// Create a golden flakes/stars particle effect for item pickups
fn create_golden_pickup_effect(effects: &mut ResMut<Assets<EffectAsset>>) -> Handle<EffectAsset> {
    // Golden color gradient: bright gold to yellow, fading out smoothly to 0 opacity
    let mut color_gradient = bevy_hanabi::Gradient::new();
    color_gradient.add_key(0.0, Vec4::new(1.0, 0.85, 0.0, 0.7)); // Bright gold - full opacity
    color_gradient.add_key(0.2, Vec4::new(1.0, 0.9, 0.3, 0.6)); // Golden yellow - start fading
    color_gradient.add_key(0.4, Vec4::new(1.0, 0.95, 0.5, 0.3)); // Light yellow - continue fading
    color_gradient.add_key(0.6, Vec4::new(1.0, 0.97, 0.6, 0.2)); // Continue fading
    color_gradient.add_key(0.8, Vec4::new(1.0, 0.98, 0.7, 0.1)); // Almost transparent
    color_gradient.add_key(1.0, Vec4::new(1.0, 1.0, 0.9, 0.0)); // Fully transparent at end

    // Size gradient: particles start small, grow slightly, then shrink
    let mut size_gradient = bevy_hanabi::Gradient::new();
    size_gradient.add_key(0.0, Vec3::splat(0.020)); // Small flakes (2x smaller)
    size_gradient.add_key(0.2, Vec3::splat(0.025)); // Grow (2x smaller)
    size_gradient.add_key(0.5, Vec3::splat(0.03)); // Peak size (2x smaller)
    size_gradient.add_key(0.8, Vec3::splat(0.02)); // Shrink (2x smaller)
    size_gradient.add_key(1.0, Vec3::splat(0.00)); // Very small at end (2x smaller)

    let writer = ExprWriter::new();

    // Initialize particles
    let age = writer.lit(0.).expr();
    let init_age = SetAttributeModifier::new(Attribute::AGE, age);

    // Longer lifetime for slow fade-out effect
    let lifetime = writer.lit(1.0).uniform(writer.lit(2.0)).expr();
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, lifetime);

    // Spawn particles in a sphere around the pickup location
    let init_pos = SetPositionSphereModifier {
        center: writer.lit(0.2).mul(writer.rand(VectorType::VEC3F)).expr(),
        radius: writer.lit(0.25).expr(),
        dimension: ShapeDimension::Surface,
    };

    // Velocity: particles move mostly upward with slight horizontal spread
    let random_x = writer.lit(-0.5).uniform(writer.lit(0.5)); // Small horizontal spread
    let random_y = writer.lit(2.0).uniform(writer.lit(4.0)); // Strong upward movement
    let random_z = writer.lit(-0.5).uniform(writer.lit(0.5)); // Small horizontal spread
    let velocity = random_x.vec3(random_y, random_z);
    let init_vel = SetAttributeModifier::new(Attribute::VELOCITY, velocity.expr());

    // Reduced gravity so particles move mostly upward
    let accel = writer.lit(Vec3::new(0.0, -0.3, 0.0)).expr();
    let update_accel = AccelModifier::new(accel);

    // Drag to slow particles down
    let drag = writer.lit(1.2).expr();
    let update_drag = LinearDragModifier::new(drag);

    effects.add(
        EffectAsset::new(
            512,                                // Smaller capacity for fewer particles
            SpawnerSettings::once(20.0.into()), // Fewer particles (30)
            writer.finish(),
        )
        .with_name("golden_pickup")
        .init(init_pos)
        .init(init_vel)
        .init(init_age)
        .init(init_lifetime)
        .update(update_accel) // Gravity
        .update(update_drag) // Air resistance
        .render(ColorOverLifetimeModifier {
            gradient: color_gradient,
            blend: ColorBlendMode::Modulate,
            mask: ColorBlendMask::RGBA,
        })
        .render(SizeOverLifetimeModifier {
            gradient: size_gradient,
            screen_space_size: false,
        })
        .render(OrientModifier {
            mode: OrientMode::FaceCameraPosition,
            rotation: None,
        }),
    )
}
