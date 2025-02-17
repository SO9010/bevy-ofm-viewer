use bevy::{app::*, ecs::{query::With, system::{Query, ResMut}}, render::camera::{Camera, OrthographicProjection}, transform::components::Transform};
use bevy_egui::{egui::{self, RichText}, EguiContexts, EguiPlugin};

use crate::tile_map::{change_zoom_level, ChunkManager, Location, ZoomManager};

pub struct MapUIPlugin;

impl Plugin for MapUIPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
        .add_systems(Update, ui_example);

    }
}


fn ui_example(
    mut contexts: EguiContexts,
    mut chunk_manager: ResMut<ChunkManager>,
    mut zoom_manager: ResMut<ZoomManager>,
    mut ortho_projection_query: Query<&mut OrthographicProjection, With<Camera>>,
    mut camera_query: Query<&mut Transform, With<Camera>>,
    location_manager: ResMut<Location>,
) {
    let ctx = contexts.ctx_mut();

    // Button 1: Positioned at (10, 10)
    egui::Area::new("button_1".into())
        .fixed_pos(egui::pos2(10.0, 10.0)) // Absolute position
        .show(ctx, |ui| {
            if ui.button(RichText::new(" + ").size(15.)).clicked() {
                if zoom_manager.zoom_level > 1 {
                    let zoom = zoom_manager.zoom_level + 1;
                    change_zoom_level(&mut chunk_manager, &mut zoom_manager, &location_manager, &mut camera_query, &mut ortho_projection_query, zoom);
                }
            }
        });

    // Button 2: Positioned at (10, 30)
    egui::Area::new("button_2".into())
        .fixed_pos(egui::pos2(10.0, 30.0)) // Absolute position
        .show(ctx, |ui| {
            if ui.button(RichText::new(" - ").size(20.)).clicked() {
                if zoom_manager.zoom_level < 19 {
                    let zoom = zoom_manager.zoom_level - 1;
                    change_zoom_level(&mut chunk_manager, &mut zoom_manager, &location_manager, &mut camera_query, &mut ortho_projection_query, zoom);
                }
            }
        });
}
