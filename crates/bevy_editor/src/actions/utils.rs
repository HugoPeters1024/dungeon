use bevy::prelude::*;

pub fn world_position_to_local(world: &World, entity: Entity, world_position: Vec3) -> Vec3 {
    if let Some(child_of) = world.get::<ChildOf>(entity) {
        let parent = child_of.parent();
        if let Some(parent_global) = world.get::<GlobalTransform>(parent) {
            parent_global
                .affine()
                .inverse()
                .transform_point3(world_position)
        } else {
            world_position
        }
    } else {
        world_position
    }
}

pub fn world_scale_to_local(world: &World, entity: Entity, world_scale: Vec3) -> Vec3 {
    if let Some(child_of) = world.get::<ChildOf>(entity) {
        let parent = child_of.parent();
        if let Some(parent_global) = world.get::<GlobalTransform>(parent) {
            parent_global
                .affine()
                .inverse()
                .to_scale_rotation_translation()
                .0
                * world_scale
        } else {
            world_scale
        }
    } else {
        world_scale
    }
}
