use std::{fs::File, io::Read};

use bevy::{image, log::info, math::Vec2};

struct tile {
    name: String,
    image: image::Image,
    world_location: Vec2,
    tile_location: Vec2,
    zoom: i32,
} 

impl tile {
    pub fn new(name: String, image: image::Image, world_location: Vec2, tile_location: Vec2, zoom: i32) -> Self {
        Self {
            name,
            image,
            world_location,
            tile_location,
            zoom,
        }
    }
}

pub fn testeee() {
    let path = "5397.pbf";
    let mut file = File::open(path).expect("Failed to open PBF file");

    // Create an OsmPbfReader
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).expect("Failed to read file");
    // Decode the MVT tile
    let tile = Reader::new(buf).unwrap();

    
    // Iterate over layers and features
    for i in 0..tile.get_layer_names().iter().len() {
        for features in tile.get_features(i) {
            for feature in features {
                info!("Feature: {:?}", feature.geometry);
            } 
        }
    }
    // Ok(())
}
