use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_hanabi::prelude::*;

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
        ),
        collection(typed)
    )]
    pub player_animations: Vec<Handle<AnimationClip>>,

    pub fire: Handle<EffectAsset>,
    pub void: Handle<EffectAsset>,
}

#[derive(Resource)]
pub struct CharacterAnimations {
    pub graph: Handle<AnimationGraph>,
    pub root: AnimationNodeIndex,
    pub running: AnimationNodeIndex,
    pub defeated: AnimationNodeIndex,
    pub right_strafe: AnimationNodeIndex,
    pub left_strafe: AnimationNodeIndex,
    pub a180: AnimationNodeIndex,
    pub jump: AnimationNodeIndex,
    pub falling_landing: AnimationNodeIndex,
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
    mut commands: Commands,
    mut assets: ResMut<GameAssets>,
    mut effects: ResMut<Assets<EffectAsset>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut state: ResMut<NextState<MyStates>>,
) {
    assets.fire = create_fire_effect(&mut effects);
    assets.void = create_void_effect(&mut effects);

    let mut graph = AnimationGraph::new();
    let defeated = graph.add_clip(assets.player_animations[0].clone(), 1.0, graph.root);
    let running = graph.add_clip(assets.player_animations[1].clone(), 1.0, graph.root);
    let right_strafe = graph.add_clip(assets.player_animations[2].clone(), 1.0, graph.root);
    let left_strafe = graph.add_clip(assets.player_animations[3].clone(), 1.0, graph.root);
    let a180 = graph.add_clip(assets.player_animations[4].clone(), 1.0, graph.root);
    let jump = graph.add_clip(assets.player_animations[5].clone(), 1.0, graph.root);
    let falling_landing = graph.add_clip(assets.player_animations[6].clone(), 1.0, graph.root);
    let graph_handle = graphs.add(graph.clone());

    commands.insert_resource(CharacterAnimations {
        graph: graph_handle,
        root: graph.root,
        running,
        defeated,
        right_strafe,
        left_strafe,
        a180,
        jump,
        falling_landing,
    });

    state.set(MyStates::Next);
}

/// Create a fire particle effect
fn create_fire_effect(effects: &mut ResMut<Assets<EffectAsset>>) -> Handle<EffectAsset> {
    let mut color_gradient = bevy_hanabi::Gradient::new();
    color_gradient.add_key(0.0, Vec4::new(1.0, 1.0, 0.0, 1.0)); // Yellow
    color_gradient.add_key(0.3, Vec4::new(1.0, 0.5, 0.0, 1.0)); // Orange
    color_gradient.add_key(0.6, Vec4::new(1.0, 0.2, 0.0, 0.8)); // Red-orange
    color_gradient.add_key(1.0, Vec4::new(0.3, 0.0, 0.0, 0.0)); // Dark red, fading out

    let mut size_gradient = bevy_hanabi::Gradient::new();
    size_gradient.add_key(0.0, Vec3::splat(0.05));
    size_gradient.add_key(0.5, Vec3::splat(0.08));
    size_gradient.add_key(1.0, Vec3::splat(0.02));

    let writer = ExprWriter::new();

    // Initialize particles with random position in a small circle
    let age = writer.lit(0.).expr();
    let init_age = SetAttributeModifier::new(Attribute::AGE, age);

    let lifetime = writer.lit(0.8).uniform(writer.lit(1.2)).expr();
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, lifetime);

    // Spawn particles in a small circle at the base
    let init_pos = SetPositionCircleModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        axis: writer.lit(Vec3::Y).expr(),
        radius: writer.lit(0.05).expr(),
        dimension: ShapeDimension::Surface,
    };

    // Initial upward velocity with small random horizontal offset
    // Create a mostly upward direction with slight randomness in X and Z
    let random_x = writer.lit(-0.2).uniform(writer.lit(0.2));
    let random_z = writer.lit(-0.2).uniform(writer.lit(0.2));
    let upward_speed = writer.lit(1.5).uniform(writer.lit(0.5));
    // Velocity is mostly upward with small horizontal variation
    let velocity = random_x.vec3(upward_speed, random_z);
    let init_vel = SetAttributeModifier::new(Attribute::VELOCITY, velocity.expr());

    // Add drag to make particles slow down - reduced for higher rise
    let drag = writer.lit(0.8).expr();
    let update_drag = LinearDragModifier::new(drag);

    effects.add(
        EffectAsset::new(32768, SpawnerSettings::rate(50.0.into()), writer.finish())
            .with_name("fire")
            .init(init_pos)
            .init(init_vel)
            .init(init_age)
            .init(init_lifetime)
            .update(update_drag)
            .render(ColorOverLifetimeModifier {
                gradient: color_gradient,
                blend: ColorBlendMode::Modulate,
                mask: ColorBlendMask::RGBA,
            })
            .render(SizeOverLifetimeModifier {
                gradient: size_gradient,
                screen_space_size: false,
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
