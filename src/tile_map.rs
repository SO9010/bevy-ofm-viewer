// Thank you for the example: https://github.com/StarArawn/bevy_ecs_tilemap/blob/main/examples/chunking.rs
use bevy::{asset::RenderAssetUsages, prelude::*, render::render_resource::{Extent3d, TextureDimension, TextureFormat}, utils::HashSet, window::PrimaryWindow};
use bevy_ecs_tilemap::{map::{TilemapId, TilemapTexture, TilemapTileSize}, tiles::{TileBundle, TilePos, TileStorage}, TilemapBundle, TilemapPlugin};

use crate::{geo_to_tile, level_to_tile_width, ofm_api::{get_ofm_data, OfmTiles}, tile::{self, Coord}, tile_to_geo, world_degreese_to_world_mercator, world_mercator_to_lat_lon, STARTING_LONG_LAT, STARTING_ZOOM};


const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 2446.0, y: 2446.0};
// For this example, don't choose too large a chunk size.
const CHUNK_SIZE: UVec2 = UVec2 { x: 1, y: 1 };
// Render chunk sizes are set to 4 render chunks per user specified chunk.
const RENDER_CHUNK_SIZE: UVec2 = UVec2 {
    x: CHUNK_SIZE.x * 1,
    y: CHUNK_SIZE.y * 1,
};

pub struct TileMapPlugin;

impl Plugin for TileMapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TilemapPlugin)
            .insert_resource(ChunkManager::default())
            .add_systems(Update, spawn_chunks_around_camera)
            .add_systems(Update, (despawn_outofrange_chunks, handle_keyboard));
    }
}

#[derive(Default, Debug, Resource)]
struct ChunkManager {
    pub spawned_chunks: HashSet<IVec2>,
    pub refresh_chunks: bool,
}


pub fn handle_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut chunk_manager: ResMut<ChunkManager>,
    query: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    primary_window_query: Query<&Window, With<PrimaryWindow>>,
) {
    let (camera, camera_transform) = query.single();
    let window: &Window = primary_window_query.single();
    if keys.pressed(KeyCode::KeyU) {
        chunk_manager.refresh_chunks = true;
    }
}

fn spawn_chunk(
    commands: &mut Commands,
    tile: Handle<Image>,
    chunk_pos: IVec2,
) {
    let tilemap_entity = commands.spawn_empty().id();
    let mut tile_storage = TileStorage::empty(CHUNK_SIZE.into());

    // TODO: the duplocates come from here where it is trying to chunk the data! 
    // Spawn the elements of the tilemap.
    let tile_pos = TilePos { x:  0, y:  0 };
    let tile_entity = commands
    .spawn(TileBundle {
        position: tile_pos,
        tilemap_id: TilemapId(tilemap_entity),
        ..Default::default()
    })
    .id();
    commands.entity(tilemap_entity).add_child(tile_entity);
    tile_storage.set(&tile_pos, tile_entity);

    let transform = Transform::from_translation(Vec3::new(
        chunk_pos.x as f32 * CHUNK_SIZE.x as f32 * TILE_SIZE.x,
        chunk_pos.y as f32 * CHUNK_SIZE.y as f32 * TILE_SIZE.y,
        0.0,
    ));
    
    commands.entity(tilemap_entity).insert(TilemapBundle {
        grid_size: TILE_SIZE.into(),
        size: CHUNK_SIZE.into(),
        storage: tile_storage,
        texture: TilemapTexture::Single(tile), // Ensure the texture is applied to the tilemap
        tile_size: TILE_SIZE,
        transform,
        render_settings: bevy_ecs_tilemap::map::TilemapRenderSettings {
            render_chunk_size: RENDER_CHUNK_SIZE,
            ..Default::default()
        },
        ..Default::default()
    });
}

fn camera_pos_to_chunk_pos(camera_pos: &Vec2) -> IVec2 {
    let chunk_size = Vec2::new(
        CHUNK_SIZE.x as f32 * TILE_SIZE.x as f32,
        CHUNK_SIZE.y as f32 * TILE_SIZE.y as f32,
    );
    let camera_pos = Vec2::new(camera_pos.x as f32, camera_pos.y as f32) / chunk_size;
    camera_pos.floor().as_ivec2()
}


fn chunk_pos_to_world_pos(chunk_pos: IVec2) -> Vec2 {
    let chunk_size = Vec2::new(
        CHUNK_SIZE.x as f32 * TILE_SIZE.x as f32,
        CHUNK_SIZE.y as f32 * TILE_SIZE.y as f32,
    );
    Vec2::new(
        chunk_pos.x as f32 * chunk_size.x,
        chunk_pos.y as f32 * chunk_size.y,
    )
}

fn spawn_chunks_around_camera(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    camera_query: Query<&Transform, With<Camera>>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut ofm_tiles: ResMut<OfmTiles>,
) {
    for transform in camera_query.iter() {
        let camera_chunk_pos = camera_pos_to_chunk_pos(&transform.translation.xy());
        let range = 1; // Adjust this range to limit the number of chunks being loaded

        for y in (camera_chunk_pos.y - range)..=(camera_chunk_pos.y + range) {
            for x in (camera_chunk_pos.x - range)..=(camera_chunk_pos.x + range) {
                let chunk_pos = IVec2::new(x, y);
                if !chunk_manager.spawned_chunks.contains(&chunk_pos) {
                    let world_pos = chunk_pos_to_world_pos(chunk_pos);
                    let position = world_mercator_to_lat_lon(world_pos.x.into(), world_pos.y.into(), STARTING_LONG_LAT);
                    let tile_coords = geo_to_tile(position.1, position.0, STARTING_ZOOM.into());
                    let tile_key = (
                        tile_coords.0 as i64,
                        tile_coords.1 as i64,
                        STARTING_ZOOM
                    );

                    chunk_manager.spawned_chunks.insert(chunk_pos);
                    let tile_image = get_ofm_data(tile_key.0 as u64, tile_key.1 as u64, tile_key.2 as u64, TILE_SIZE.x as u32);
                    let tile_handle = asset_server.add(tile_image); // Load the image from file path
                    spawn_chunk(&mut commands, tile_handle, chunk_pos);
                }
            }
        }
    }
}

fn despawn_outofrange_chunks(
    
    mut commands: Commands,
    camera_query: Query<&GlobalTransform, With<Camera>>,
    chunks_query: Query<(Entity, &Transform)>,
    mut chunk_manager: ResMut<ChunkManager>,
) {
    for camera_transform in camera_query.iter() {
        for (entity, chunk_transform) in chunks_query.iter() {
            let chunk_pos = camera_transform.translation().xy();
            let distance = camera_transform.translation().xy().distance(chunk_pos);
            let threshold = 1000.0; // Increase this threshold to prevent chunks from being despawned too aggressively
            if distance > threshold {
                let x = (chunk_pos.x / (CHUNK_SIZE.x as f32 * TILE_SIZE.x)).floor() as i32;
                let y = (chunk_pos.y / (CHUNK_SIZE.y as f32 * TILE_SIZE.y)).floor() as i32;
                chunk_manager.spawned_chunks.remove(&IVec2::new(x, y));
                commands.entity(entity).despawn_recursive();
            }
        }
    }
}