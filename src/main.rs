use bevy::{prelude::*, window::PrimaryWindow, winit::{UpdateMode, WinitSettings}};
use bevy_pancam::PanCamPlugin;
use camera::{camera_middle_to_lat_long, setup_camera};
use debug::DebugPlugin;
use ofm_api::OfmTiles;
use rstar::RTree;
use tile::{world_mercator_to_lat_lon, Coord};
use tile_map::{ChunkManager, Location, TileMapPlugin, ZoomManager};
use ui::MapUIPlugin;

pub mod ofm_api;
pub mod tile;
pub mod tile_map;
pub mod debug;
pub mod camera;
pub mod ui;

pub const STARTING_LONG_LAT: Coord = Coord::new(0.011, 0.011);
pub const STARTING_DISPLACEMENT: Coord = Coord::new(52.207_59, 0.186_745_48);
// This can be changed, it changes the size of each tile too.
pub const TILE_QUALITY: i32 = 256;

fn main() {
    App::new()
    .add_plugins((DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Map Viewer".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    }),PanCamPlugin, TileMapPlugin,))
    .insert_resource(WinitSettings {
        unfocused_mode: UpdateMode::Reactive {
            wait: std::time::Duration::from_secs(1),
            react_to_device_events: true,
            react_to_user_events: true,
            react_to_window_events: true,
        },
        ..Default::default()
    })
    .add_systems(Startup, setup_camera)
    .add_systems(Update, handle_mouse)
    .insert_resource(Location::default())
    .add_plugins((DebugPlugin, MapUIPlugin))
    .insert_resource(OfmTiles {
        tiles: RTree::new(),
        tiles_to_render: Vec::new(),
    })
    .insert_resource(ClearColor(Color::from(Srgba { red: 0.1, green: 0.1, blue: 0.1, alpha: 1.0 })))
    .run();
}

pub fn handle_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    q_windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    zoom_manager: Res<ZoomManager>,
    mut location_manager: ResMut<Location>,
    mut chunk_manager: ResMut<ChunkManager>,
) {
    let (camera, camera_transform) = camera.single();
    if buttons.pressed(MouseButton::Left) {
        if let Some(position) = q_windows.single().cursor_position() {
            /*
            let world_pos = camera.viewport_to_world_2d(camera_transform, position).unwrap();
            let long_lat = world_mercator_to_lat_lon(world_pos.x as f64, world_pos.y as f64, chunk_manager.refrence_long_lat, zoom_manager.zoom_level, zoom_manager.tile_size);
            let closest_tile = long_lat.to_tile_coords(zoom_manager.zoom_level).to_lat_long();
            info!("{:?}", closest_tile);
            */

            let world_pos = camera.viewport_to_world_2d(camera_transform, position).unwrap();
            info!("{:?}", world_mercator_to_lat_lon(world_pos.x.into(), world_pos.y.into(), chunk_manager.refrence_long_lat, zoom_manager.zoom_level, zoom_manager.tile_size));
        }
    }   
    if buttons.pressed(MouseButton::Middle){
        chunk_manager.update = true;
    }
    if buttons.just_released(MouseButton::Middle) {
        let movement = camera_middle_to_lat_long(camera_transform, zoom_manager.zoom_level, zoom_manager.tile_size, chunk_manager.refrence_long_lat);
        if movement != location_manager.location {
            location_manager.location = movement;
            chunk_manager.update = true;
        }
    }
}
