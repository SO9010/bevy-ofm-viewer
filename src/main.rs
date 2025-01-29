use bevy::app::*;
use bevy::prelude::*;
use bevy_pancam::PanCamPlugin;
use bevy_prototype_lyon::prelude::*;
use ofm_reader::testeee;

pub mod ofm_reader;
pub mod tile;

fn main() {
    App::new()
    .add_plugins((DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "OFM Viewer".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    }), ShapePlugin, PanCamPlugin))
    .add_systems(Update, testeee)
    .insert_resource(ClearColor(Color::from(Srgba { red: 0.1, green: 0.1, blue: 0.1, alpha: 1.0 })))
    .run();
}
