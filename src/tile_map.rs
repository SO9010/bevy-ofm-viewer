use std::thread;

// Thank you for the example: https://github.com/StarArawn/bevy_ecs_tilemap/blob/main/examples/chunking.rs
use bevy::{prelude::*, utils::{HashMap, HashSet}};
use bevy_ecs_tilemap::{map::{TilemapId, TilemapTexture, TilemapTileSize}, tiles::{TileBundle, TilePos, TileStorage}, TilemapBundle, TilemapPlugin};
use crossbeam_channel::{bounded, Receiver, Sender};

use crate::{geo_to_tile, level_to_tile_width, ofm_api::{buffer_to_bevy_image, get_ofm_data, get_ofm_image}, world_mercator_to_lat_lon, STARTING_LONG_LAT, STARTING_ZOOM, TILE_QUALITY};

//const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 2446.0, y: 2446.0};
const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: TILE_QUALITY as f32, y: TILE_QUALITY as f32};
// For this example, don't choose too large a chunk size.
const CHUNK_SIZE: UVec2 = UVec2 { x: 1, y: 1 };

pub struct TileMapPlugin;

impl Plugin for TileMapPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx): (Sender<(IVec2, Vec<u8>)>, Receiver<(IVec2, Vec<u8>)>) = bounded(10);
        app.insert_resource(ChunkReceiver(rx))  // Store receiver globally
            .insert_resource(ChunkSender(tx))
            .add_plugins(TilemapPlugin)
            .insert_resource(ChunkManager::default())
            .add_systems(Update, (spawn_chunks_around_camera, spawn_to_needed_chunks))
            .add_systems(Update, (despawn_outofrange_chunks))
            .add_systems(FixedUpdate, (read_map_receiver));
    }
}

#[derive(Default, Debug, Resource)]
struct ChunkManager {
    pub spawned_chunks: HashSet<IVec2>,
    pub to_spawn_chunks: HashMap<IVec2, Vec<u8>>, // Store raw image data
}

#[derive(Resource, Deref)]
pub struct ChunkReceiver(Receiver<(IVec2, Vec<u8>)>); // Use Vec<u8> for raw image data

#[derive(Resource, Deref)]
pub struct ChunkSender(Sender<(IVec2, Vec<u8>)>);

fn spawn_chunk(
    commands: &mut Commands,
    tile: Handle<Image>,
    chunk_pos: IVec2,
) {
    let tilemap_entity = commands.spawn_empty().id();
    let mut tile_storage = TileStorage::empty(CHUNK_SIZE.into());

    let tile_pos = TilePos { x: 0, y: 0 };
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
        texture: TilemapTexture::Single(tile),
        tile_size: TILE_SIZE,
        transform,
        ..Default::default()
    });
}

fn camera_pos_to_chunk_pos(camera_pos: &Vec2) -> IVec2 {
    let chunk_size = Vec2::new(
        CHUNK_SIZE.x as f32 * TILE_SIZE.x,
        CHUNK_SIZE.y as f32 * TILE_SIZE.y,
    );
    let camera_pos = Vec2::new(camera_pos.x, camera_pos.y) / chunk_size;
    camera_pos.floor().as_ivec2()
}


fn chunk_pos_to_world_pos(chunk_pos: IVec2) -> Vec2 {
    let chunk_size = Vec2::new(
        CHUNK_SIZE.x as f32 * TILE_SIZE.x,
        CHUNK_SIZE.y as f32 * TILE_SIZE.y,
    );
    Vec2::new(
        chunk_pos.x as f32 * chunk_size.x,
        chunk_pos.y as f32 * chunk_size.y,
    )
}

fn spawn_chunks_around_camera(
    camera_query: Query<&Transform, With<Camera>>,
    chunk_sender: Res<ChunkSender>,  // Use the stored sender
    chunk_manager: ResMut<ChunkManager>,
) {
    for transform in camera_query.iter() {
        let camera_chunk_pos = camera_pos_to_chunk_pos(&transform.translation.xy());
        let range = 2;

        for y in (camera_chunk_pos.y - range)..=(camera_chunk_pos.y + range) {
            for x in (camera_chunk_pos.x - range)..=(camera_chunk_pos.x + range) {
                let chunk_pos = IVec2::new(x, y);
                if !chunk_manager.spawned_chunks.contains(&chunk_pos) {
                    let tx = chunk_sender.clone(); // Clone existing sender
                    thread::spawn(move || {
                        let world_pos = chunk_pos_to_world_pos(chunk_pos);
                        let position = world_mercator_to_lat_lon(world_pos.x.into(), world_pos.y.into(), STARTING_LONG_LAT);
                        let tile_coords = geo_to_tile(position.1, position.0, STARTING_ZOOM);
                        info!("powinno byc ok: {:?}", position);
                        info!("go to tile coords: {:?}", tile_coords);
                        let tile_key = (tile_coords.0 as i64, tile_coords.1 as i64, STARTING_ZOOM);

                        let tile_image = get_ofm_data(tile_key.0 as u64, tile_key.1 as u64, tile_key.2 as u64, TILE_SIZE.x as u32);
                        if let Err(e) = tx.send((chunk_pos, tile_image)) {
                            eprintln!("Failed to send chunk data: {:?}", e);
                        } else {
                        }
                    });
                }
            }
        }
    }
}

fn read_map_receiver(
    map_receiver: Res<ChunkReceiver>,
    mut chunk_manager: ResMut<ChunkManager>,
) {
    let mut new_chunks = Vec::new();

    while let Ok((chunk_pos, raw_image_data)) = map_receiver.try_recv() {
        if !chunk_manager.to_spawn_chunks.contains_key(&chunk_pos) {
            new_chunks.push((chunk_pos, raw_image_data));
        }
    }

    for (pos, data) in new_chunks {
        chunk_manager.to_spawn_chunks.insert(pos, data);
    }
}

fn spawn_to_needed_chunks(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut chunk_manager: ResMut<ChunkManager>,
) {
    let to_spawn_chunks: Vec<(IVec2, Vec<u8>)> = chunk_manager.to_spawn_chunks.iter().map(|(pos, data)| (*pos, data.clone())).collect();
    for (chunk_pos, raw_image_data) in to_spawn_chunks {
        let tile_handle = images.add(buffer_to_bevy_image(raw_image_data, TILE_SIZE.x as u32));
        spawn_chunk(&mut commands, tile_handle, chunk_pos);
        chunk_manager.spawned_chunks.insert(chunk_pos);
    }
    chunk_manager.to_spawn_chunks.clear();
}

fn despawn_outofrange_chunks(
    mut commands: Commands,
    camera_query: Query<&Transform, With<Camera>>,
    chunks_query: Query<(Entity, &Transform)>,
    mut chunk_manager: ResMut<ChunkManager>,
) {
    for camera_transform in camera_query.iter() {
        for (entity, chunk_transform) in chunks_query.iter() {
            let chunk_pos = chunk_transform.translation.xy();
            let distance = camera_transform.translation.xy().distance(chunk_pos);
            if distance > 256. * 10.{
                // info!("Despawning chunk at {:?}", chunk_pos);
                let x = (chunk_pos.x / (CHUNK_SIZE.x as f32 * TILE_SIZE.x)).floor() as i32;
                let y = (chunk_pos.y / (CHUNK_SIZE.y as f32 * TILE_SIZE.y)).floor() as i32;
                chunk_manager.spawned_chunks.remove(&IVec2::new(x, y));
                commands.entity(entity).despawn_recursive();
            }
        }
    }
}