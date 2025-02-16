use bevy::{prelude::*, winit::WinitSettings};
use bevy_pancam::PanCamPlugin;
use rstar::RTree;
use tile::Coord;

pub mod camera;
pub mod debug;
pub mod api;
pub mod tile;
pub mod tile_map;

pub const STARTING_LONG_LAT: Coord = Coord::new(0.011, 0.011);
pub const STARTING_DISPLACEMENT: Coord = Coord::new(52.207_59, 0.186_745_48);
// This can be changed, it changes the size of each tile too.
pub const TILE_QUALITY: i32 = 256;

pub struct OsmViewerPlugin;

impl Plugin for OsmViewerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((PanCamPlugin, tile_map::TileMapPlugin))
            .insert_resource(WinitSettings::default())
            .add_systems(Startup, camera::setup_camera)
            .insert_resource(tile_map::Location::default())
            .add_plugins(debug::DebugPlugin)
            .insert_resource(api::OfmTiles {
                tiles: RTree::new(),
                tiles_to_render: Vec::new(),
            })
            .insert_resource(ClearColor(Color::from(Srgba { red: 0.1, green: 0.1, blue: 0.1, alpha: 1.0 })));
    }
}