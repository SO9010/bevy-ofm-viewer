use bevy::{prelude::*, window::PrimaryWindow, winit::WinitSettings};
use bevy_ofm_viewer::camera::handle_mouse;
use bevy_pancam::PanCamPlugin;
use camera::{camera_middle_to_lat_long, setup_camera};
use debug::DebugPlugin;
use api::OfmTiles;
use rstar::RTree;
use tile::{world_mercator_to_lat_lon, Coord};
use tile_map::{ChunkManager, Location, TileMapPlugin, ZoomManager};

pub mod api;
pub mod tile;
pub mod tile_map;
pub mod debug;
pub mod camera;

pub const STARTING_LONG_LAT: Coord = Coord::new(0.011, 0.011);
pub const STARTING_DISPLACEMENT: Coord = Coord::new(52.207_59, 0.186_745_48);
// This can be changed, it changes the size of each tile too.
pub const TILE_QUALITY: i32 = 256;

fn main() {
    App::new()
    .add_plugins((DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "OFM Viewer".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    }), PanCamPlugin, TileMapPlugin))
    .insert_resource(WinitSettings::default())
    .add_systems(Startup, setup_camera)
    .add_systems(Update, handle_mouse)
    .insert_resource(Location::default())
    .add_plugins(DebugPlugin)
    .insert_resource(OfmTiles {
        tiles: RTree::new(),
        tiles_to_render: Vec::new(),
    })
    .insert_resource(ClearColor(Color::from(Srgba { red: 0.1, green: 0.1, blue: 0.1, alpha: 1.0 })))
    .run();
}
