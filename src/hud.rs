use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::assets::MyStates;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Vitals>()
            .add_systems(OnEnter(MyStates::Next), spawn_hud)
            .add_systems(
                Update,
                update_hud_from_vitals.run_if(in_state(MyStates::Next)),
            );
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct Vitals {
    pub health: f32,
    pub max_health: f32,
    pub mana: f32,
    pub max_mana: f32,
}

impl Default for Vitals {
    fn default() -> Self {
        Self {
            health: 85.0,
            max_health: 100.0,
            mana: 42.0,
            max_mana: 100.0,
        }
    }
}

#[derive(Component)]
struct HudRoot;

#[derive(Component, Clone, Copy)]
enum OrbKind {
    Health,
    Mana,
}

#[derive(Component)]
struct OrbFill(OrbKind);

#[derive(Component)]
struct OrbText(OrbKind);

#[derive(Resource, Default)]
struct HudImages {
    hp_fill: Handle<Image>,
    mp_fill: Handle<Image>,
    frame: Handle<Image>,
    gloss: Handle<Image>,
}

fn spawn_hud(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let hud_images = HudImages {
        frame: images.add(make_orb_frame_image(256)),
        gloss: images.add(make_orb_gloss_image(256)),
        hp_fill: images.add(make_orb_fill_image(256, Color::srgb(0.78, 0.08, 0.12))),
        mp_fill: images.add(make_orb_fill_image(256, Color::srgb(0.10, 0.30, 0.86))),
    };
    commands.insert_resource(hud_images);

    // Root overlay (non-interactive).
    let root = commands
        .spawn((
            HudRoot,
            Name::new("HUD Root"),
            GlobalZIndex(10),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
        ))
        .id();

    let hp_orb = spawn_orb(&mut commands, OrbKind::Health, Some(22.0), None);
    let mp_orb = spawn_orb(&mut commands, OrbKind::Mana, None, Some(22.0));

    commands.entity(root).add_child(hp_orb);
    commands.entity(root).add_child(mp_orb);
}

fn spawn_orb(
    commands: &mut Commands,
    kind: OrbKind,
    left: Option<f32>,
    right: Option<f32>,
) -> Entity {
    let orb_size = 148.0;
    let pad = 18.0;

    let mut node = Node {
        width: Val::Px(orb_size),
        height: Val::Px(orb_size),
        position_type: PositionType::Absolute,
        bottom: Val::Px(pad),
        ..default()
    };
    if let Some(x) = left {
        node.left = Val::Px(x);
    }
    if let Some(x) = right {
        node.right = Val::Px(x);
    }

    let outer = commands
        .spawn((
            Name::new(match kind {
                OrbKind::Health => "Health Orb",
                OrbKind::Mana => "Mana Orb",
            }),
            node,
        ))
        .id();

    // Clip container for fill image.
    let clip = commands
        .spawn((
            Name::new("Orb Clip"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderRadius::all(Val::Px(999.0)),
        ))
        .id();

    let fill = commands
        .spawn((
            OrbFill(kind),
            Name::new("Orb Fill"),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                height: Val::Percent(60.0),
                ..default()
            },
            ImageNode::default(),
        ))
        .id();

    // Frame and gloss are full-size images on top.
    let frame = commands
        .spawn((
            Name::new("Orb Frame"),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode::default(),
        ))
        .id();

    let gloss = commands
        .spawn((
            Name::new("Orb Gloss"),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode::default(),
        ))
        .id();

    // Clean number, no labels.
    let text = commands
        .spawn((
            OrbText(kind),
            Name::new("Orb Value Text"),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            children![(
                Text::new(""),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgba(0.97, 0.95, 0.90, 0.88)),
            )],
        ))
        .id();

    commands.entity(outer).add_child(clip);
    commands.entity(clip).add_child(fill);
    commands.entity(outer).add_child(frame);
    commands.entity(outer).add_child(gloss);
    commands.entity(outer).add_child(text);

    outer
}

fn update_hud_from_vitals(
    vitals: Res<Vitals>,
    hud_images: Res<HudImages>,
    mut fills: Query<(&OrbFill, &mut Node, &mut ImageNode)>,
    texts: Query<(&OrbText, &Children)>,
    mut inner_text: Query<&mut Text>,
    mut frames: Query<(&Name, &mut ImageNode), Without<OrbFill>>,
) {
    let hp_frac = (vitals.health / vitals.max_health).clamp(0.0, 1.0);
    let mp_frac = (vitals.mana / vitals.max_mana).clamp(0.0, 1.0);

    // Apply textures (frame/gloss) once (cheap guard: check if image handle is default).
    for (name, mut img) in frames.iter_mut() {
        if img.image != Handle::<Image>::default() {
            continue;
        }
        if name.as_str().contains("Frame") {
            img.image = hud_images.frame.clone();
        } else if name.as_str().contains("Gloss") {
            img.image = hud_images.gloss.clone();
        }
    }

    for (fill, mut node, mut img) in fills.iter_mut() {
        let (frac, tex) = match fill.0 {
            OrbKind::Health => (hp_frac, hud_images.hp_fill.clone()),
            OrbKind::Mana => (mp_frac, hud_images.mp_fill.clone()),
        };
        node.height = Val::Percent(frac * 100.0);
        img.image = tex;
    }

    for (label, children) in texts.iter() {
        let value = match label.0 {
            OrbKind::Health => vitals.health,
            OrbKind::Mana => vitals.mana,
        };
        for child in children.iter() {
            if let Ok(mut t) = inner_text.get_mut(child) {
                *t = Text::new(format!("{:.0}", value.max(0.0)));
            }
        }
    }
}

fn make_orb_frame_image(size: u32) -> Image {
    let mut data = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 * 0.5;
    let r_outer = c - 2.0;
    let r_inner = c - 18.0;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let dx = fx - c;
            let dy = fy - c;
            let d = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;

            let in_ring = d <= r_outer && d >= r_inner;
            if !in_ring {
                continue;
            }

            // Metallic gradient.
            let t = ((d - r_inner) / (r_outer - r_inner)).clamp(0.0, 1.0);
            let ang = dy.atan2(dx);
            let sheen = ((ang * 2.0).sin() * 0.12 + 0.12).clamp(0.0, 0.22);
            let base = 0.10 + (1.0 - t) * 0.08 + sheen;
            let r = (base * 255.0) as u8;
            let g = ((base * 0.78) * 255.0) as u8;
            let b = ((base * 0.55) * 255.0) as u8;

            data[idx] = r;
            data[idx + 1] = g;
            data[idx + 2] = b;
            data[idx + 3] = 255;
        }
    }

    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
}

fn make_orb_gloss_image(size: u32) -> Image {
    let mut data = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 * 0.5;
    let r = c - 20.0;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let dx = fx - c;
            let dy = fy - c;
            let d = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;
            if d > r {
                continue;
            }

            // Gloss highlight biased to top-left.
            let hx = (fx / size as f32 - 0.20).clamp(0.0, 1.0);
            let hy = (fy / size as f32 - 0.10).clamp(0.0, 1.0);
            let h = (1.0 - (hx * hx + hy * hy).sqrt()).clamp(0.0, 1.0);
            let alpha = (h * 0.10).min(0.10);

            data[idx] = 255;
            data[idx + 1] = 255;
            data[idx + 2] = 255;
            data[idx + 3] = (alpha * 255.0) as u8;
        }
    }

    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
}

fn make_orb_fill_image(size: u32, base: Color) -> Image {
    let mut data = vec![0u8; (size * size * 4) as usize];
    let c = size as f32 * 0.5;
    let r = c - 20.0;

    let [br, bg, bb, _] = base.to_srgba().to_f32_array();

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let dx = fx - c;
            let dy = fy - c;
            let d = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;
            if d > r {
                continue;
            }

            let edge = (1.0 - (d / r)).clamp(0.0, 1.0);
            let v = (fy / size as f32).clamp(0.0, 1.0);
            // Use wrapping math to avoid debug overflow panics.
            let hash = x
                .wrapping_mul(1103515245)
                .wrapping_add(y.wrapping_mul(12345));
            let swirl = (((hash & 0xff) as f32) / 255.0) * 0.06 - 0.03;
            let bright = (0.55 + edge * 0.50 + (1.0 - v) * 0.18 + swirl).clamp(0.0, 1.0);

            data[idx] = (br * bright * 255.0) as u8;
            data[idx + 1] = (bg * bright * 255.0) as u8;
            data[idx + 2] = (bb * bright * 255.0) as u8;
            data[idx + 3] = 255;
        }
    }

    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
}
