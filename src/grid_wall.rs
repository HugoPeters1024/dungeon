use bevy::{
    gltf::{GltfMesh, GltfNode},
    prelude::*,
};
use rstar::RTree;

use crate::assets::{GameAssets, MyStates};

struct MeshBounds {
    extents: [f32; 3],
    center: Vec3,
}

fn mesh_bounds(mesh: &Mesh) -> MeshBounds {
    let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        return MeshBounds {
            extents: [1.0; 3],
            center: Vec3::ZERO,
        };
    };
    let positions = positions.as_float3().unwrap();
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for pos in positions {
        for i in 0..3 {
            min[i] = min[i].min(pos[i]);
            max[i] = max[i].max(pos[i]);
        }
    }
    MeshBounds {
        extents: [max[0] - min[0], max[1] - min[1], max[2] - min[2]],
        center: Vec3::new(
            (min[0] + max[0]) / 2.0,
            (min[1] + max[1]) / 2.0,
            (min[2] + max[2]) / 2.0,
        ),
    }
}

#[derive(Component, Reflect)]
pub struct GridMeshes {
    walls: [Entity; 4],
}

impl Default for GridMeshes {
    fn default() -> Self {
        Self {
            walls: [Entity::PLACEHOLDER; 4],
        }
    }
}

#[derive(Component, Default, Reflect)]
#[require(GridMeshes, Transform, InheritedVisibility)]
pub struct GridWall {
    neighbours: [bool; 27],
}

pub struct GridWallPlugin;

impl Plugin for GridWallPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_grid_wall_added);
        app.add_systems(
            Update,
            (grid_wall_sync, update_walls).run_if(resource_exists::<GameAssets>),
        );
    }
}

fn on_grid_wall_added(
    on: On<Add, GridWall>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    meshes: Res<Assets<Mesh>>,
    mut grid_meshes_q: Query<&mut GridMeshes>,
) {
    let entity = on.event_target();
    let gltf = gltf_assets.get(&assets.castle_test).unwrap();

    // Spawn floor
    let node_handle = gltf
        .named_nodes
        .get("Ceiling and Beams Tiler, 9 x 9.000")
        .unwrap();
    let node = gltf_nodes.get(node_handle).unwrap();
    let gltf_mesh = gltf_meshes.get(node.mesh.as_ref().unwrap()).unwrap();

    commands.entity(entity).with_children(|parent| {
        for primitive in &gltf_mesh.primitives {
            let bounds = meshes.get(&primitive.mesh).map(mesh_bounds);
            let scale = bounds
                .as_ref()
                .map(|b| {
                    let dominant = b.extents.into_iter().fold(0.0f32, f32::max);
                    if dominant > 0.0 { 1.0 / dominant } else { 1.0 }
                })
                .unwrap_or(1.0);
            let center_correction = bounds
                .as_ref()
                .map(|b| b.center * scale)
                .unwrap_or(Vec3::ZERO);
            let mut child = parent.spawn((
                Mesh3d(primitive.mesh.clone()),
                Transform::from_translation(-center_correction)
                    .with_scale(Vec3::splat(scale)),
            ));
            if let Some(material) = primitive.material.as_ref() {
                child.insert(MeshMaterial3d(material.clone()));
            }
        }
    });

    // Spawn 4 walls (initially visible)
    let wall_mesh_handle = gltf.named_meshes.get("Wall").unwrap();
    let wall_gltf_mesh = gltf_meshes.get(wall_mesh_handle).unwrap();
    let wall_primitive = &wall_gltf_mesh.primitives[0];
    let wall_mesh = meshes.get(&wall_primitive.mesh);
    let wall_bounds = wall_mesh.map(mesh_bounds);
    let wall_scale = wall_bounds
        .as_ref()
        .map(|b| {
            let dominant = b.extents.into_iter().fold(0.0f32, f32::max);
            if dominant > 0.0 { 1.0 / dominant } else { 1.0 }
        })
        .unwrap_or(1.0);
    let wall_thickness = wall_bounds
        .as_ref()
        .map(|b| b.extents.into_iter().fold(f32::MAX, f32::min) * wall_scale)
        .unwrap_or(0.0);
    let wall_center = wall_bounds
        .as_ref()
        .map(|b| b.center)
        .unwrap_or(Vec3::ZERO);
    let wall_offset = 0.5 - wall_thickness / 2.0;

    let mut wall_entities = [Entity::PLACEHOLDER; 4];
    for (i, &(_neighbour_idx, dir, rotation_y)) in WALL_FACES.iter().enumerate() {
        let rotation = Quat::from_rotation_y(rotation_y);
        let center_correction = rotation * (wall_center * wall_scale);
        let offset = dir.normalize() * wall_offset - center_correction;
        let wall = commands
            .spawn((
                Mesh3d(wall_primitive.mesh.clone()),
                MeshMaterial3d(wall_primitive.material.clone().unwrap()),
                Transform::from_translation(offset)
                    .with_rotation(rotation)
                    .with_scale(Vec3::splat(wall_scale)),
                ChildOf(entity),
            ))
            .id();
        wall_entities[i] = wall;
    }

    if let Ok(mut grid_meshes) = grid_meshes_q.get_mut(entity) {
        grid_meshes.walls = wall_entities;
    }
}

fn grid_wall_sync(
    mut q: Query<(&GlobalTransform, &mut GridWall)>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyY) {
        return;
    }
    info!("grid_wall_sync");
    let rtree = RTree::bulk_load(q.iter().map(|x| x.0.translation().to_array()).collect());

    const EPS: f32 = 0.2;
    for (gt, mut gw) in q.iter_mut() {
        let mut idx = 0;
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let (x, y, z) = (x as f32, y as f32, z as f32);
                    let target = gt.translation() + Vec3::new(x, y, z);
                    let env = rstar::AABB::from_corners(
                        [target.x - EPS, target.y - EPS, target.z - EPS],
                        [target.x + EPS, target.y + EPS, target.z + EPS],
                    );
                    gw.neighbours[idx] = rtree.locate_in_envelope(&env).next().is_some();
                    idx += 1;
                }
            }
        }
    }
}

const WALL_FACES: [(usize, Vec3, f32); 4] = [
    (4, Vec3::new(-0.5, 0.0, 0.0), std::f32::consts::FRAC_PI_2), // -X
    (22, Vec3::new(0.5, 0.0, 0.0), -std::f32::consts::FRAC_PI_2), // +X
    (12, Vec3::new(0.0, 0.0, -0.5), std::f32::consts::PI),       // -Z
    (14, Vec3::new(0.0, 0.0, 0.5), 0.0),                         // +Z
];

fn update_walls(
    q: Query<(&GridWall, &GridMeshes), Changed<GridWall>>,
    mut visibility_q: Query<&mut Visibility>,
) {
    for (grid_wall, grid_meshes) in q.iter() {
        for (i, &(neighbour_idx, _, _)) in WALL_FACES.iter().enumerate() {
            let has_neighbour = grid_wall.neighbours[neighbour_idx];
            if let Ok(mut vis) = visibility_q.get_mut(grid_meshes.walls[i]) {
                *vis = if has_neighbour {
                    Visibility::Hidden
                } else {
                    Visibility::Visible
                };
            }
        }
    }
}
