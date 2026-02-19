use bevy::{
    gltf::{GltfMesh, GltfNode},
    prelude::*,
};
use rstar::RTree;

use crate::assets::GameAssets;

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

/// Pre-computed placement info for fitting a mesh into a unit cube face.
struct FittedMesh {
    scale: f32,
    thickness: f32,
    center: Vec3,
}

impl FittedMesh {
    fn from_mesh(mesh: Option<&Mesh>) -> Self {
        let Some(mesh) = mesh else {
            return Self { scale: 1.0, thickness: 0.0, center: Vec3::ZERO };
        };
        let bounds = mesh_bounds(mesh);
        let dominant = bounds.extents.into_iter().fold(0.0f32, f32::max);
        let scale = if dominant > 0.0 { 1.0 / dominant } else { 1.0 };
        let thickness = bounds.extents.into_iter().fold(f32::MAX, f32::min) * scale;
        Self { scale, thickness, center: bounds.center }
    }

    fn edge_offset(&self) -> f32 {
        0.5 - self.thickness / 2.0
    }

    /// Translation that places the mesh at a cube face.
    /// `face_dir` is the outward direction of the face (e.g. -Y for floor),
    /// `rotation` is the rotation applied to the mesh.
    fn face_translation(&self, face_dir: Vec3, rotation: Quat) -> Vec3 {
        let center_correction = rotation * (self.center * self.scale);
        face_dir.normalize() * self.edge_offset() - center_correction
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
            let fitted = FittedMesh::from_mesh(meshes.get(&primitive.mesh));
            let offset = fitted.face_translation(Vec3::NEG_Y, Quat::IDENTITY);
            let mut child = parent.spawn((
                Mesh3d(primitive.mesh.clone()),
                Transform::from_translation(offset).with_scale(Vec3::splat(fitted.scale)),
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
    let fitted_wall = FittedMesh::from_mesh(meshes.get(&wall_primitive.mesh));

    let mut wall_entities = [Entity::PLACEHOLDER; 4];
    for (i, &(_neighbour_idx, dir, rotation_y)) in WALL_FACES.iter().enumerate() {
        let rotation = Quat::from_rotation_y(rotation_y);
        let offset = fitted_wall.face_translation(dir, rotation);
        let wall = commands
            .spawn((
                Mesh3d(wall_primitive.mesh.clone()),
                MeshMaterial3d(wall_primitive.material.clone().unwrap()),
                Transform::from_translation(offset)
                    .with_rotation(rotation)
                    .with_scale(Vec3::splat(fitted_wall.scale)),
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::PrimitiveTopology;

    fn make_box_mesh(min: [f32; 3], max: [f32; 3]) -> Mesh {
        let positions = vec![min, max, [max[0], min[1], min[2]], [min[0], max[1], max[2]]];
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh
    }

    #[test]
    fn mesh_bounds_centered_box() {
        let mesh = make_box_mesh([-1.0, -2.0, -3.0], [1.0, 2.0, 3.0]);
        let bounds = mesh_bounds(&mesh);
        assert_eq!(bounds.extents, [2.0, 4.0, 6.0]);
        assert!(bounds.center.abs_diff_eq(Vec3::ZERO, 1e-6));
    }

    #[test]
    fn mesh_bounds_off_center_box() {
        let mesh = make_box_mesh([1.0, 2.0, 3.0], [3.0, 6.0, 9.0]);
        let bounds = mesh_bounds(&mesh);
        assert_eq!(bounds.extents, [2.0, 4.0, 6.0]);
        assert!(bounds.center.abs_diff_eq(Vec3::new(2.0, 4.0, 6.0), 1e-6));
    }

    #[test]
    fn mesh_bounds_no_positions_returns_defaults() {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD,
        );
        let bounds = mesh_bounds(&mesh);
        assert_eq!(bounds.extents, [1.0, 1.0, 1.0]);
        assert_eq!(bounds.center, Vec3::ZERO);
    }

    #[test]
    fn mesh_bounds_thin_wall() {
        let mesh = make_box_mesh([-5.0, -5.0, -0.1], [5.0, 5.0, 0.1]);
        let bounds = mesh_bounds(&mesh);
        let thickness = bounds.extents.into_iter().fold(f32::MAX, f32::min);
        let dominant = bounds.extents.into_iter().fold(0.0f32, f32::max);
        let scale = 1.0 / dominant;
        assert!((thickness * scale - 0.02).abs() < 1e-6);
    }

    #[test]
    fn wall_faces_neighbour_indices_are_valid() {
        for &(idx, _, _) in &WALL_FACES {
            assert!(idx < 27, "neighbour index {idx} out of 3x3x3 range");
        }
    }

    /// The 3x3x3 neighbour grid is indexed as x * 9 + y * 3 + z with
    /// x,y,z each in -1..=1 mapped to 0..=2. Verify the WALL_FACES
    /// indices match the expected cardinal directions.
    #[test]
    fn wall_faces_neighbour_indices_match_directions() {
        let idx =
            |x: i32, y: i32, z: i32| -> usize { ((x + 1) * 9 + (y + 1) * 3 + (z + 1)) as usize };
        assert_eq!(WALL_FACES[0].0, idx(-1, 0, 0), "-X face");
        assert_eq!(WALL_FACES[1].0, idx(1, 0, 0), "+X face");
        assert_eq!(WALL_FACES[2].0, idx(0, 0, -1), "-Z face");
        assert_eq!(WALL_FACES[3].0, idx(0, 0, 1), "+Z face");
    }

    #[test]
    fn wall_faces_directions_are_unit_length_scaled() {
        for &(_, dir, _) in &WALL_FACES {
            assert!(
                (dir.length() - 0.5).abs() < 1e-6,
                "direction {dir} should have length 0.5"
            );
        }
    }

    #[test]
    fn grid_wall_default_has_no_neighbours() {
        let gw = GridWall::default();
        assert!(gw.neighbours.iter().all(|&n| !n));
    }

    #[test]
    fn update_walls_hides_wall_when_neighbour_present() {
        let mut app = App::new();
        app.add_systems(Update, update_walls);

        let wall_entities: [Entity; 4] =
            std::array::from_fn(|_| app.world_mut().spawn(Visibility::Visible).id());

        let mut gw = GridWall::default();
        gw.neighbours[WALL_FACES[0].0] = true;
        gw.neighbours[WALL_FACES[2].0] = true;

        app.world_mut().spawn((
            gw,
            GridMeshes {
                walls: wall_entities,
            },
        ));
        app.update();

        let vis = |e: Entity| *app.world().get::<Visibility>(e).unwrap();
        assert_eq!(
            vis(wall_entities[0]),
            Visibility::Hidden,
            "-X wall should be hidden"
        );
        assert_eq!(
            vis(wall_entities[1]),
            Visibility::Visible,
            "+X wall should be visible"
        );
        assert_eq!(
            vis(wall_entities[2]),
            Visibility::Hidden,
            "-Z wall should be hidden"
        );
        assert_eq!(
            vis(wall_entities[3]),
            Visibility::Visible,
            "+Z wall should be visible"
        );
    }

    #[test]
    fn update_walls_shows_all_when_no_neighbours() {
        let mut app = App::new();
        app.add_systems(Update, update_walls);

        let wall_entities: [Entity; 4] =
            std::array::from_fn(|_| app.world_mut().spawn(Visibility::Hidden).id());

        let gw = GridWall::default();
        app.world_mut().spawn((
            gw,
            GridMeshes {
                walls: wall_entities,
            },
        ));
        app.update();

        for (i, &e) in wall_entities.iter().enumerate() {
            assert_eq!(
                *app.world().get::<Visibility>(e).unwrap(),
                Visibility::Visible,
                "wall {i} should be visible with no neighbours"
            );
        }
    }

    #[test]
    fn fitted_mesh_edge_offset_accounts_for_thickness() {
        let mesh = make_box_mesh([-5.0, -4.0, -0.25], [5.0, 4.0, 0.25]);
        let fitted = FittedMesh::from_mesh(Some(&mesh));
        let edge = fitted.edge_offset();
        assert!(edge < 0.5, "offset should be less than 0.5 for non-zero thickness");
        assert!(edge > 0.0, "offset should be positive");
        assert!((edge - (0.5 - 0.05 / 2.0)).abs() < 1e-6);
    }

    #[test]
    fn fitted_mesh_none_returns_defaults() {
        let fitted = FittedMesh::from_mesh(None);
        assert_eq!(fitted.scale, 1.0);
        assert_eq!(fitted.thickness, 0.0);
        assert_eq!(fitted.center, Vec3::ZERO);
        assert!((fitted.edge_offset() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn face_translation_corrects_off_center_mesh() {
        let mesh = make_box_mesh([0.0, 0.0, 0.0], [10.0, 8.0, 0.5]);
        let fitted = FittedMesh::from_mesh(Some(&mesh));

        let translation = fitted.face_translation(Vec3::new(0.0, 0.0, 0.5), Quat::IDENTITY);
        let expected_edge = fitted.edge_offset();
        let expected_center = fitted.center * fitted.scale;
        let expected = Vec3::new(0.0, 0.0, 1.0) * expected_edge - expected_center;
        assert!(
            translation.abs_diff_eq(expected, 1e-6),
            "got {translation}, expected {expected}"
        );
    }

    #[test]
    fn face_translation_rotated_wall() {
        let mesh = make_box_mesh([-5.0, -4.0, -0.1], [5.0, 4.0, 0.1]);
        let fitted = FittedMesh::from_mesh(Some(&mesh));

        let rot = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let t1 = fitted.face_translation(Vec3::NEG_X, rot);
        let t2 = fitted.face_translation(Vec3::X, Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2));
        assert!(
            (t1.x.abs() - t2.x.abs()).abs() < 1e-5,
            "symmetric walls should have symmetric X offsets"
        );
    }

    #[test]
    fn floor_pushed_to_negative_y_edge() {
        let mesh = make_box_mesh([-5.0, -0.1, -5.0], [5.0, 0.1, 5.0]);
        let fitted = FittedMesh::from_mesh(Some(&mesh));
        let translation = fitted.face_translation(Vec3::NEG_Y, Quat::IDENTITY);

        let expected_y = -fitted.edge_offset() - fitted.center.y * fitted.scale;
        assert!(
            (translation.y - expected_y).abs() < 1e-6,
            "floor y={}, expected {expected_y}", translation.y
        );
        assert!(translation.y < 0.0, "floor should be below center");
    }

    #[test]
    fn floor_and_wall_use_same_logic() {
        let wall_mesh = make_box_mesh([-5.0, -4.0, -0.1], [5.0, 4.0, 0.1]);
        let floor_mesh = make_box_mesh([-5.0, -0.1, -5.0], [5.0, 0.1, 5.0]);

        let wall_fitted = FittedMesh::from_mesh(Some(&wall_mesh));
        let floor_fitted = FittedMesh::from_mesh(Some(&floor_mesh));

        assert!(
            (wall_fitted.edge_offset() - floor_fitted.edge_offset()).abs() < 1e-6,
            "same-thickness meshes should produce the same edge offset"
        );
    }
}
