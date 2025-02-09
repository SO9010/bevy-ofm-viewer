use std::{default, thread};

// Thank you for the example: https://github.com/StarArawn/bevy_ecs_tilemap/blob/main/examples/chunking.rs
use bevy::{prelude::*, utils::{HashMap, HashSet}};
use bevy_ecs_tilemap::{map::{TilemapGridSize, TilemapId, TilemapTexture, TilemapTileSize}, tiles::{TileBundle, TilePos, TileStorage}, TilemapBundle, TilemapPlugin};
use crossbeam_channel::{bounded, Receiver, Sender};

use crate::{coord_offset, geo_to_tile, ofm_api::{buffer_to_bevy_image, get_ofm_data}, world_mercator_to_lat_lon, STARTING_LONG_LAT, TILE_QUALITY};

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
            .insert_resource(ZoomManager::default())
            .add_systems(Update, (spawn_chunks_around_camera, spawn_to_needed_chunks))
            .add_systems(Update, (despawn_outofrange_chunks, detect_zoom_level))
            .add_systems(FixedUpdate, read_map_receiver);
    }
}

#[derive(Debug, Resource, Clone)]
pub struct ZoomManager {
    pub zoom_level: u32,
    pub last_zoom_level: u32,
    pub last_projection_level: f32,
    pub tile_size: f32
}

impl Default for ZoomManager {
    fn default() -> Self {
        Self {
            zoom_level: 14,
            last_zoom_level: 0,
            last_projection_level: 0.0,
            tile_size: TILE_QUALITY as f32
        }
    }
}

#[derive(Debug, Resource)]
pub struct ChunkManager {
    pub spawned_chunks: HashSet<IVec2>,
    pub to_spawn_chunks: HashMap<IVec2, Vec<u8>>, // Store raw image data
    pub offset: Vec2,
    pub update: bool, // Store raw image data
}

impl Default for ChunkManager {
    fn default() -> Self {
        Self {
            spawned_chunks: HashSet::default(),
            to_spawn_chunks: HashMap::default(),
            offset: Vec2::new(0., 0.),
            update: true,
        }
    }
}

// This causes an annoying move which must be fixed, then it will work a lot better, then we can start working on other features.
/*
This is esseitnally the same place. Look at how mmuch it moves
I think this comes from the fact that Im putting it into a grid, and the grid may be displased. Infact the lat and long are not accurate.
Im a little bit lost here. I think that this issue comes from the fact that the tiles snap onto the grid.
We probably want to add a section which is where the offset comes from.
2025-02-08T21:21:32.503268Z  INFO bevy_ofm_viewer: (52.188956676958675, 0.14935694976164401)
2025-02-08T21:21:36.106325Z  INFO bevy_ofm_viewer: (52.18216577373214, 0.16036131199363995)
2025-02-08T21:21:41.694419Z  INFO bevy_ofm_viewer: (52.17539586891242, 0.1713836405313208)

52.17654007829139, 0.14520868302901419
52.18081628177534, 0.14514385107853697

*/
fn detect_zoom_level(
    mut chunk_manager: ResMut<ChunkManager>,
    mut zoom_manager: ResMut<ZoomManager>,
    mut ortho_projection_query: Query<&mut OrthographicProjection, With<Camera>>,
    chunk_query: Query<(Entity, &TileMarker)>,
    mut commands: Commands,
) {
    if let Ok(mut projection) = ortho_projection_query.get_single_mut() {
        if projection.scale != zoom_manager.last_projection_level {
            zoom_manager.last_projection_level = projection.scale;
            if projection.scale > 2.0 && projection.scale != 0. && zoom_manager.zoom_level > 3 {
                zoom_manager.last_zoom_level = zoom_manager.zoom_level;
                zoom_manager.zoom_level -= 1;
                despawn_all_chunks(commands, chunk_query);
                chunk_manager.spawned_chunks.clear();
                chunk_manager.to_spawn_chunks.clear();
                projection.scale = 1.0;
            } else if projection.scale < 0.5 && projection.scale != 0. && zoom_manager.zoom_level < 14 {
                zoom_manager.last_zoom_level = zoom_manager.zoom_level;
                zoom_manager.zoom_level += 1;
                despawn_all_chunks(commands, chunk_query);
                chunk_manager.spawned_chunks.clear();
                chunk_manager.to_spawn_chunks.clear();
                projection.scale = 1.0;
            }
            chunk_manager.update = true;
        }
    }
}

#[derive(Resource, Deref)]
pub struct ChunkReceiver(Receiver<(IVec2, Vec<u8>)>); // Use Vec<u8> for raw image data

#[derive(Resource, Deref)]
pub struct ChunkSender(Sender<(IVec2, Vec<u8>)>);

#[derive(Component)]
pub struct TileMarker;

fn spawn_chunk(
    commands: &mut Commands,
    tile: Handle<Image>,
    chunk_pos: IVec2,
    tile_size: f32,
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
        .insert(TileMarker)
        .id();
    commands.entity(tilemap_entity).add_child(tile_entity);
    tile_storage.set(&tile_pos, tile_entity);

    let transform = Transform::from_translation(Vec3::new(
        chunk_pos.x as f32 * CHUNK_SIZE.x as f32 * tile_size,
        chunk_pos.y as f32 * CHUNK_SIZE.y as f32 * tile_size,
        0.0,
    ));

    commands.entity(tilemap_entity).insert(TilemapBundle {
        grid_size: TilemapGridSize::from(TilemapTileSize { x: tile_size, y: tile_size }),
        size: CHUNK_SIZE.into(),
        storage: tile_storage,
        texture: TilemapTexture::Single(tile),
        tile_size: TilemapTileSize { x: tile_size, y: tile_size },
        transform,
        ..Default::default()
    });
}

fn camera_pos_to_chunk_pos(camera_pos: &Vec2, tile_size: f32) -> IVec2 {
    let chunk_size = Vec2::new(
        CHUNK_SIZE.x as f32 * tile_size,
        CHUNK_SIZE.y as f32 * tile_size,
    );
    let camera_pos = Vec2::new(camera_pos.x, camera_pos.y) / chunk_size;
    camera_pos.floor().as_ivec2()
}


fn chunk_pos_to_world_pos(chunk_pos: IVec2, tile_size: f32) -> Vec2 {
    let chunk_size = Vec2::new(
        CHUNK_SIZE.x as f32 * tile_size,
        CHUNK_SIZE.y as f32 * tile_size,
    );
    Vec2::new(
        chunk_pos.x as f32 * chunk_size.x,
        chunk_pos.y as f32 * chunk_size.y,
    )
}

fn spawn_chunks_around_camera(
    camera_query: Query<&Transform, With<Camera>>,
    chunk_sender: Res<ChunkSender>,  // Use the stored sender
    mut chunk_manager: ResMut<ChunkManager>,
    zoom_manager: Res<ZoomManager>,
) {
    if chunk_manager.update {
        chunk_manager.update = false;
        for transform in camera_query.iter() {
            let camera_chunk_pos = camera_pos_to_chunk_pos(&transform.translation.xy(), zoom_manager.tile_size);
            let range = 2;

            for y in (camera_chunk_pos.y - range)..=(camera_chunk_pos.y + range) {
                for x in (camera_chunk_pos.x - range)..=(camera_chunk_pos.x + range) {
                    let chunk_pos = IVec2::new(x, y);
                    if !chunk_manager.spawned_chunks.contains(&chunk_pos) {
                        let tx = chunk_sender.clone(); // Clone existing sender
                        let zoom_manager = zoom_manager.clone();
                        let world_pos = chunk_pos_to_world_pos(chunk_pos, zoom_manager.tile_size);
                        let position = world_mercator_to_lat_lon(world_pos.x.into(), world_pos.y.into(), STARTING_LONG_LAT.into(), zoom_manager.zoom_level.into(), zoom_manager.tile_size.into(), Vec2::new(0., 0.));
                        let offset = coord_offset(position.1, position.0, zoom_manager.zoom_level);

                        thread::spawn(move || {
                            let tile_coords = geo_to_tile(position.1, position.0, zoom_manager.zoom_level);
                            let tile_key = (tile_coords.0 as i64, tile_coords.1 as i64, zoom_manager.zoom_level);
    
                            let tile_image = get_ofm_data(tile_key.0 as u64, tile_key.1 as u64, tile_key.2 as u64, zoom_manager.tile_size as u32);
                            if let Err(e) = tx.send((chunk_pos, tile_image)) {
                                eprintln!("Failed to send chunk data: {:?}", e);
                            }
                        });

                        chunk_manager.offset = Vec2::new(offset.0 as f32, offset.1 as f32);
                        chunk_manager.spawned_chunks.insert(chunk_pos);
                    }
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
    mut images: ResMut<Assets<Image>>,
    mut chunk_manager: ResMut<ChunkManager>,
    zoom_manager: Res<ZoomManager>,
) {
    let to_spawn_chunks: Vec<(IVec2, Vec<u8>)> = chunk_manager.to_spawn_chunks.iter().map(|(pos, data)| (*pos, data.clone())).collect();
    for (chunk_pos, raw_image_data) in to_spawn_chunks {
        let tile_handle = images.add(buffer_to_bevy_image(raw_image_data, zoom_manager.tile_size as u32));
        spawn_chunk(&mut commands, tile_handle, chunk_pos, zoom_manager.tile_size);
        chunk_manager.spawned_chunks.insert(chunk_pos);
    }
    chunk_manager.to_spawn_chunks.clear();
}

fn despawn_outofrange_chunks(
    mut commands: Commands,
    camera_query: Query<&Transform, With<Camera>>,
    chunks_query: Query<(Entity, &Transform, &TileMarker)>,
    mut chunk_manager: ResMut<ChunkManager>,
    zoom_manager: Res<ZoomManager>,
) {
    for camera_transform in camera_query.iter() {
        for (entity, chunk_transform, _) in chunks_query.iter() {
            let chunk_pos = chunk_transform.translation.xy();
            let distance = camera_transform.translation.xy().distance(chunk_pos);
            if distance > zoom_manager.tile_size * 10.{
                let x = (chunk_pos.x / (CHUNK_SIZE.x as f32 * zoom_manager.tile_size)).floor() as i32;
                let y = (chunk_pos.y / (CHUNK_SIZE.y as f32 * zoom_manager.tile_size)).floor() as i32;
                chunk_manager.spawned_chunks.remove(&IVec2::new(x, y));
                commands.entity(entity).despawn_recursive();
            }
        }
    }
}

fn despawn_all_chunks(
    mut commands: Commands,
    chunks_query: Query<(Entity, &TileMarker)>,
) {
    for (entity, _) in chunks_query.iter() {
            commands.entity(entity).despawn_recursive();
    }
}