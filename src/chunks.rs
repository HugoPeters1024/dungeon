use avian3d::prelude::*;
use bevy::{math::Affine2, mesh::Indices, platform::collections::HashMap, prelude::*};
use noise::{NoiseFn, Perlin};

use crate::assets::{GameAssets, MyStates};

#[derive(Component)]
pub struct ChunkObserver;

#[derive(Resource, Default, Deref, DerefMut)]
pub struct ChunkIndex(HashMap<IVec2, Entity>);

pub struct ChunksPlugin;

const FLOOR_SIZE: i32 = 8;

/// Controls how many terrain chunks are kept around the player.
/// `spawn_radius` of 2 means a 5x5 square (from -2..=2 in x/y).
#[derive(Resource, Debug, Clone, Copy)]
pub struct ChunkRenderSettings {
    pub spawn_radius: i32,
    /// Chunks beyond this radius will be despawned to avoid unbounded growth.
    /// Kept slightly larger than spawn_radius to reduce pop-in when moving quickly.
    pub despawn_radius: i32,
}

impl Default for ChunkRenderSettings {
    fn default() -> Self {
        Self {
            // 3 => 7x7 chunks visible around the player
            spawn_radius: 3,
            // keep extra margin to reduce pop-in when moving quickly (11x11 max kept)
            despawn_radius: 5,
        }
    }
}

impl Plugin for ChunksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChunkIndex>();
        app.init_resource::<ChunkRenderSettings>();
        app.add_systems(Update, update_chunk_index.run_if(in_state(MyStates::Next)));
    }
}

fn update_chunk_index(
    mut commands: Commands,
    q: Single<(&GlobalTransform, &ChunkObserver)>,
    index: Res<ChunkIndex>,
    settings: Res<ChunkRenderSettings>,
) {
    let (gt, _) = *q;

    let loc = gt.translation().xz().as_ivec2() / IVec2::splat(FLOOR_SIZE);
    for y in -settings.spawn_radius..=settings.spawn_radius {
        for x in -settings.spawn_radius..=settings.spawn_radius {
            let key = loc + IVec2::new(x, y);
            if !index.contains_key(&key) {
                commands.run_system_cached_with(spawn_chunk, key);
            }
        }
    }

    // Despawn chunks that are too far away to keep memory/meshes bounded.
    if settings.despawn_radius >= 0 {
        let mut to_remove: Vec<IVec2> = Vec::new();
        for (key, entity) in index.iter() {
            let d = *key - loc;
            if d.x.abs() > settings.despawn_radius || d.y.abs() > settings.despawn_radius {
                commands.entity(*entity).despawn();
                to_remove.push(*key);
            }
        }
        if !to_remove.is_empty() {
            commands.run_system_cached_with(remove_chunks_from_index, to_remove);
        }
    }
}

fn remove_chunks_from_index(In(keys): In<Vec<IVec2>>, mut index: ResMut<ChunkIndex>) {
    for k in keys {
        index.remove(&k);
    }
}

fn spawn_chunk(
    In(offset): In<IVec2>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<GameAssets>,
    mut index: ResMut<ChunkIndex>,
) {
    // base - heightfield floor
    const FLOOR_RESOLUTION: usize = 100;
    let (heightfield_mesh, heights) = generate_heightfield_mesh(offset, FLOOR_RESOLUTION);
    let heightfield_handle = meshes.add(heightfield_mesh);

    let entity = commands
        .spawn((
            Mesh3d(heightfield_handle),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture: Some(assets.outside_grass.clone()),
                uv_transform: Affine2::from_scale(Vec2::new(10.0, 10.0)),
                perceptual_roughness: 1.0,
                ..default()
            })),
            Transform::from_xyz(
                (offset.x * FLOOR_SIZE) as f32,
                0.0,
                (offset.y * FLOOR_SIZE) as f32,
            ),
            RigidBody::Static,
            Collider::heightfield(
                heights,
                Vec3::new(FLOOR_SIZE as f32, 1.0, FLOOR_SIZE as f32),
            ),
        ))
        .id();

    index.insert(offset, entity);
}

struct LayeredPerlin {
    layers: Vec<Perlin>,
    // how fast the frequency should increase at each layer (sane = 2.0)
    lacunarity: f64,
    // how much the influence should diminish at each layer [0 1]
    persistance: f64,
}

impl LayeredPerlin {
    fn new(num_layers: u32) -> Self {
        LayeredPerlin {
            layers: (0u32..num_layers).map(Perlin::new).collect(),
            lacunarity: 2.0,
            persistance: 0.5,
        }
    }

    fn get(&self, x: f64, z: f64) -> f64 {
        let mut frequency = 1.0;
        let mut amplitude = 1.0;
        let mut acc = 0.0;

        for layer in self.layers.iter() {
            acc += layer.get([x * frequency, z * frequency]) * amplitude;
            frequency *= self.lacunarity;
            amplitude *= self.persistance;
        }

        acc
    }
}

/// Generate a heightfield mesh and height data using Perlin noise
/// Returns (mesh, heights) where heights is a 2D array for the collider
fn generate_heightfield_mesh(offset: IVec2, resolution: usize) -> (Mesh, Vec<Vec<f32>>) {
    let perlin = LayeredPerlin::new(8);
    let noise_scale = 0.02;
    let height_scale = 6.0;

    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    let mut heights = Vec::new(); // Store heights for collider

    // Generate vertices and heights in x-outer, z-inner order to match heightfield collider
    for x in 0..=resolution {
        let mut height_column = Vec::new();
        for z in 0..=resolution {
            let x_pos = (x as f32 / resolution as f32 - 0.5) * FLOOR_SIZE as f32;
            let z_pos = (z as f32 / resolution as f32 - 0.5) * FLOOR_SIZE as f32;

            // Sample Perlin noise for height
            let height = perlin.get(
                ((offset.x * FLOOR_SIZE) as f64 + x_pos as f64) * noise_scale,
                ((offset.y * FLOOR_SIZE) as f64 + z_pos as f64) * noise_scale,
            ) as f32
                * height_scale;

            positions.push([x_pos, height, z_pos]);
            uvs.push([x as f32 / resolution as f32, z as f32 / resolution as f32]);
            height_column.push(height);
        }
        heights.push(height_column);
    }

    // Generate indices for triangles (indexed mesh)
    // Each quad is made of 2 triangles
    // Vertex layout: column-major order, x-outer loop, z-inner loop
    // For a grid of resolution x resolution quads, we have (resolution+1) x (resolution+1) vertices
    for x in 0..resolution {
        for z in 0..resolution {
            // Calculate vertex indices for the quad corners
            // With x-outer, z-inner: index = x * (resolution + 1) + z
            let top_left = (x * (resolution + 1) + z) as u32;
            let top_right = (x * (resolution + 1) + z + 1) as u32;
            let bottom_left = ((x + 1) * (resolution + 1) + z) as u32;
            let bottom_right = ((x + 1) * (resolution + 1) + z + 1) as u32;

            indices.push(bottom_left);
            indices.push(top_left);
            indices.push(top_right);

            indices.push(bottom_left);
            indices.push(top_right);
            indices.push(bottom_right);
        }
    }

    // Create mesh using the builder pattern
    let mut mesh = Mesh::new(
        bevy::render::render_resource::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::MAIN_WORLD | bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    // Set indices to create an indexed mesh (reuses vertices for better performance)
    mesh.insert_indices(Indices::U32(indices));
    mesh = mesh.with_computed_smooth_normals();

    (mesh, heights)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_layered_perlin_generates_ppm() {
        const IMAGE_SIZE: usize = 256;
        let layered_perlin = LayeredPerlin::new(12);

        let mut pixels = Vec::with_capacity(IMAGE_SIZE * IMAGE_SIZE * 3);

        // Sample the noise across the range [0, FLOOR_SIZE] for both x and z
        for y in 0..IMAGE_SIZE {
            for x in 0..IMAGE_SIZE {
                // Map pixel coordinates to world coordinates in range [0, FLOOR_SIZE]
                let world_x = (x as f64 / IMAGE_SIZE as f64) * FLOOR_SIZE as f64;
                let world_z = (y as f64 / IMAGE_SIZE as f64) * FLOOR_SIZE as f64;

                // Sample the noise (returns value roughly in range [-1, 1])
                let noise_value = layered_perlin.get(world_x, world_z);

                // Normalize to [0, 1] range, then scale to [0, 255]
                let normalized = ((noise_value + 1.0) / 2.0).clamp(0.0, 1.0);
                let grayscale = (normalized * 255.0) as u8;

                // Write RGB (all same for grayscale)
                pixels.push(grayscale);
                pixels.push(grayscale);
                pixels.push(grayscale);
            }
        }

        // Write PPM file
        let mut file = File::create("layered_perlin_noise.ppm").expect("Failed to create PPM file");

        // Write PPM header (P3 = ASCII format)
        writeln!(file, "P3").expect("Failed to write header");
        writeln!(file, "{IMAGE_SIZE} {IMAGE_SIZE}").expect("Failed to write dimensions");
        writeln!(file, "255").expect("Failed to write max color value");

        // Write pixel data
        for (i, &value) in pixels.iter().enumerate() {
            if i.is_multiple_of(3) && i > 0 {
                writeln!(file).expect("Failed to write newline");
            }
            write!(file, "{value} ").expect("Failed to write pixel value");
        }

        println!("Generated layered_perlin_noise.ppm (256x256)");
    }
}
