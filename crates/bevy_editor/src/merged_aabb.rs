use bevy::{camera::primitives::Aabb, prelude::*, transform::TransformSystems};

#[derive(Component, Deref, DerefMut, Clone, Debug, Reflect)]
pub struct MergedAabb(pub Aabb);

pub struct MergedAabbPlugin;

impl Plugin for MergedAabbPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<MergedAabb>();
        app.add_systems(
            PostUpdate,
            update_merged_aabbs.after(TransformSystems::Propagate),
        );
    }
}

fn update_merged_aabbs(
    mut commands: Commands,
    aabb_query: Query<(&GlobalTransform, &Aabb)>,
    children_query: Query<&Children>,
    roots: Query<Entity, Without<ChildOf>>,
) {
    for entity in roots.iter() {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        let mut has_aabb = false;

        merge_aabb(entity, &aabb_query, &mut has_aabb, &mut min, &mut max);
        for descendant in children_query.iter_descendants(entity) {
            merge_aabb(descendant, &aabb_query, &mut has_aabb, &mut min, &mut max);
        }

        if has_aabb {
            let center = ((min + max) * 0.5).into();
            let half_extents = ((max - min) * 0.5).into();
            commands.entity(entity).insert(MergedAabb(Aabb {
                center,
                half_extents,
            }));
        }
    }
}

fn merge_aabb(
    entity: Entity,
    query: &Query<(&GlobalTransform, &Aabb)>,
    has_aabb: &mut bool,
    min: &mut Vec3,
    max: &mut Vec3,
) {
    if let Ok((global_transform, aabb)) = query.get(entity) {
        *has_aabb = true;
        let center: Vec3 = aabb.center.into();
        let half: Vec3 = aabb.half_extents.into();
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    let local_corner = center + half * Vec3::new(x, y, z);
                    let world_corner = global_transform.transform_point(local_corner);
                    *min = min.min(world_corner);
                    *max = max.max(world_corner);
                }
            }
        }
    }
}
