use std::collections::VecDeque;

use bevy::{
    camera::primitives::Aabb,
    platform::collections::{HashMap, HashSet},
    prelude::*,
    transform::TransformSystems,
};

#[derive(Component, Default, Deref, DerefMut, Clone, Debug, Reflect)]
pub struct MergedAabb(pub Aabb);

pub struct MergedAabbPlugin;

impl Plugin for MergedAabbPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<MergedAabb>();
        app.add_observer(on_aabb_added);
        app.add_systems(
            PostUpdate,
            update_merged_aabbs.after(TransformSystems::Propagate),
        );
    }
}

/// When an entity receives an `Aabb`, give it a `MergedAabb` and propagate
/// `MergedAabb` up through every ancestor so that parent bounding boxes
/// always encompass their children.
fn on_aabb_added(
    on: On<Add, Aabb>,
    mut commands: Commands,
    parents: Query<&ChildOf>,
    has_merged: Query<(), With<MergedAabb>>,
) {
    let entity = on.event_target();
    if has_merged.get(entity).is_err() {
        commands.entity(entity).insert(MergedAabb::default());
    }

    let mut current = entity;
    while let Ok(child_of) = parents.get(current) {
        let parent = child_of.parent();
        if has_merged.get(parent).is_ok() {
            break;
        }
        commands.entity(parent).insert(MergedAabb::default());
        current = parent;
    }
}

/// Recompute every `MergedAabb` once per frame, processing leaves before
/// parents (Kahn's algorithm) so each node only needs to look at its direct
/// children instead of walking the full descendant tree.
fn update_merged_aabbs(
    aabb_query: Query<(&GlobalTransform, &Aabb)>,
    children_query: Query<&Children>,
    parent_query: Query<&ChildOf>,
    targets: Query<Entity, With<MergedAabb>>,
    mut merged_aabbs: Query<&mut MergedAabb>,
) {
    let target_set: HashSet<Entity> = targets.iter().collect();
    if target_set.is_empty() {
        return;
    }

    // For each target, count how many of its direct children are also targets
    // (in-degree in the dependency graph) and record its target-parent if any.
    let mut in_degree: HashMap<Entity, u32> = HashMap::with_capacity(target_set.len());
    let mut parent_map: HashMap<Entity, Entity> = HashMap::new();

    for entity in target_set.iter().copied() {
        let child_count = children_query
            .get(entity)
            .map(|children| {
                children
                    .iter()
                    .filter(|c| target_set.contains(c))
                    .count() as u32
            })
            .unwrap_or(0);
        in_degree.insert(entity, child_count);

        if let Ok(child_of) = parent_query.get(entity) {
            let parent = child_of.parent();
            if target_set.contains(&parent) {
                parent_map.insert(entity, parent);
            }
        }
    }

    // Seed the queue with leaves (no children that are targets).
    let mut queue: VecDeque<Entity> = in_degree
        .iter()
        .filter(|&(_, &deg)| deg == 0)
        .map(|(&e, _)| e)
        .collect();

    let mut results: HashMap<Entity, Aabb> = HashMap::with_capacity(target_set.len());

    while let Some(entity) = queue.pop_front() {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        let mut has_aabb = false;

        // Own Aabb, transformed to world space.
        if let Ok((global_transform, aabb)) = aabb_query.get(entity) {
            has_aabb = true;
            expand_world_aabb(global_transform, aabb, &mut min, &mut max);
        }

        // Merge direct children's already-computed MergedAabbs.
        if let Ok(children) = children_query.get(entity) {
            for child in children.iter() {
                if let Some(child_aabb) = results.get(&child) {
                    has_aabb = true;
                    let c: Vec3 = child_aabb.center.into();
                    let h: Vec3 = child_aabb.half_extents.into();
                    min = min.min(c - h);
                    max = max.max(c + h);
                }
            }
        }

        if has_aabb {
            results.insert(
                entity,
                Aabb {
                    center: ((min + max) * 0.5).into(),
                    half_extents: ((max - min) * 0.5).into(),
                },
            );
        }

        if let Some(&parent) = parent_map.get(&entity) {
            let deg = in_degree.get_mut(&parent).unwrap();
            *deg -= 1;
            if *deg == 0 {
                queue.push_back(parent);
            }
        }
    }

    for (entity, aabb) in results {
        if let Ok(mut merged) = merged_aabbs.get_mut(entity) {
            merged.0 = aabb;
        }
    }
}

fn expand_world_aabb(
    global_transform: &GlobalTransform,
    aabb: &Aabb,
    min: &mut Vec3,
    max: &mut Vec3,
) {
    let center: Vec3 = aabb.center.into();
    let half: Vec3 = aabb.half_extents.into();
    for x in [-1.0f32, 1.0] {
        for y in [-1.0f32, 1.0] {
            for z in [-1.0f32, 1.0] {
                let local_corner = center + half * Vec3::new(x, y, z);
                let world_corner = global_transform.transform_point(local_corner);
                *min = min.min(world_corner);
                *max = max.max(world_corner);
            }
        }
    }
}
