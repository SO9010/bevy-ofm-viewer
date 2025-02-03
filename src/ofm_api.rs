use std::{clone, fs, path::Path};

use bevy::{asset::{Assets, RenderAssetUsages}, ecs::system::{Commands, ResMut, Resource}, image::Image, log::info, math::{Rect, UVec2, Vec2, Vec3}, render::render_resource::{Extent3d, TextureDimension, TextureFormat}, sprite::Sprite, transform::components::Transform};
use font_kit::{family_name::FamilyName, properties::{Properties, Weight}, source::SystemSource};
use geo::Translate;
use mvt_reader::Reader;
use raqote::{AntialiasMode, DrawOptions, DrawTarget, PathBuilder, Point, SolidSource, Source, StrokeStyle};
use rstar::{RTree, RTreeObject, AABB};

use crate::{lat_lon_to_world_mercator, level_to_tile_width, tile::Coord, world_mercator_to_lat_lon, STARTING_LONG_LAT};

#[derive(Resource, Clone)]
pub struct OfmTiles {
    pub tiles: RTree<Tile>,
    pub tiles_to_render: Vec<Tile>,
}

#[derive(Clone)]
pub struct Tile {
    pub name: String,
    pub image: Image,
    pub tile_location: Coord,
    pub zoom: i32,
} 

impl PartialEq for Tile {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.zoom == other.zoom && self.tile_location.lat == other.tile_location.lat && self.tile_location.long == other.tile_location.long
    }
}
impl RTreeObject for Tile {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(
            [self.tile_location.long as f64, self.tile_location.lat as f64],
            [
                self.tile_location.long as f64 + level_to_tile_width(self.zoom) as f64,
                self.tile_location.lat as f64 + level_to_tile_width(self.zoom) as f64,
            ],
        )
    }
}
impl Tile {
    pub fn new(name: String, image: Image, tile_location: Coord, tile_location_in_game: UVec2, zoom: i32) -> Self {
        Self {
            name,
            image,
            tile_location,
            zoom,
        }
    }
}

/*
5-01-31T01:38:37.820064Z  INFO bevy_ofm_viewer::ofm_api: convertied: Coord { lat: 52.17394, long: 0.1538086 } to (0.0, 0.0)
2025-01-31T01:38:37.827485Z  INFO bevy_ofm_viewer::ofm_api: convertied: Coord { lat: 52.19591, long: 0.1538086 } to (0.0, 2445.9849)
2025-01-31T01:38:37.834549Z  INFO bevy_ofm_viewer::ofm_api: convertied: Coord { lat: 52.17394, long: 0.17578125 } to (2445.9849, 0.0)
2025-01-31T01:38:37.841368Z  INFO bevy_ofm_viewer::ofm_api: convertied: Coord { lat: 52.19591, long: 0.17578125 } to (2445.9849, 2445.9849)

025-01-31T01:38:42.310090Z  INFO bevy_ofm_viewer::ofm_api: convertied: Coord { lat: 52.16402, long: 0.15291189 } to (-99.82099, -1104.0903)
2025-01-31T01:38:42.318214Z  INFO bevy_ofm_viewer::ofm_api: convertied: Coord { lat: 52.185993, long: 0.15291189 } to (-99.82099, 1341.8945)
2025-01-31T01:38:42.325527Z  INFO bevy_ofm_viewer::ofm_api: convertied: Coord { lat: 52.16402, long: 0.17488454 } to (2346.1638, -1104.0903)
2025-01-31T01:38:42.333455Z  INFO bevy_ofm_viewer::ofm_api: convertied: Coord { lat: 52.185993, long: 0.17488454 } to (2346.1638, 1341.8945)

*/

pub fn tile_width_meters(zoom: u32) -> f64 {
    let earth_circumference_meters = 40075016.686;
    let num_tiles = 2_u32.pow(zoom) as f64;
    earth_circumference_meters / num_tiles
}

pub fn display_ofm_tile(
    mut overpass_settings: ResMut<OfmTiles>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
) {
    for tile in overpass_settings.tiles_to_render.iter() {
        let image_handle = images.add(tile.image.clone());
        let coords = world_mercator_to_lat_lon(tile.tile_location.lat.into(), tile.tile_location.long.into(), STARTING_LONG_LAT);
        let mut img = Sprite::from_image(image_handle);
        img.custom_size = Some(Vec2::new(2080., 2080.));
        commands.spawn((
            img,
            Transform::from_xyz(coords.0 as f32, coords.1 as f32, 0.),
        ));
    }
    overpass_settings.tiles_to_render.clear();
}

pub fn get_ofm_data(x: u64, y: u64, zoom: u64, tile_size: u32) -> Image {
    let data = send_ofm_request(x, y, zoom);
    let image = ofm_to_image(data, tile_size, x, y, zoom);
    return image;
}

fn send_ofm_request(x: u64, y: u64, zoom: u64) -> Vec<u8> {
    let cache_dir = "cache";
    let cache_file = format!("{}/{}_{}_{}.pbf", cache_dir, zoom, x, y);

    // Check if the file exists in the cache
    if Path::new(&cache_file).exists() {
        return fs::read(&cache_file).expect("Failed to read cache file");
    }

    // If not in cache, fetch from the network
    let url = "https://tiles.openfreemap.org/planet/20250122_001001_pt";
    let mut status = 429;
    while status == 429 {
        if let Ok(response) = ureq::get(format!("{}/{}/{}/{}.pbf", url, zoom, x, y).as_str()).call() {
            if response.status() == 200 {
                status = 200;
                let mut reader = response.into_reader();
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes).expect("Failed to read bytes from response");

                // Save to cache
                fs::create_dir_all(cache_dir).expect("Failed to create cache directory");
                fs::write(&cache_file, &bytes).expect("Failed to write cache file");

                return bytes;
            } else if response.status() == 429 {
                std::thread::sleep(std::time::Duration::from_secs(5));
            } else {
                status = 0;
            }
        }
    }
    vec![]
}

/// This converts it to an image which is as many meters as the tile width
fn ofm_to_image(data: Vec<u8>, size: u32, x: u64, y: u64, zoom: u64) -> Image {
    // Create an OsmPbfReader
    let tile = Reader::new(data).unwrap();

    let mut dt = DrawTarget::new(size as i32, size as i32);
    let mut pb = PathBuilder::new();

    let scale: f32 = 1.675;
    // Iterate over layers and features]
    let layer_names = tile.get_layer_names().unwrap();
    for (i, title) in layer_names.into_iter().enumerate() {
        for (layer_no, features) in tile.get_features(i).iter().enumerate() {
            for feature in features {
                match &feature.geometry {
                    geo::Geometry::Point(point) 
                        => {
                            pb.move_to(point.x()/scale, point.y()/scale);
                            pb.line_to(point.x()/scale + 1.0, point.y()/scale + 1.0);
                            pb.line_to(point.x()/scale + 1.0, point.y()/scale);
                            pb.line_to(point.x()/scale, point.y()/scale + 1.0)
                        },
                    geo::Geometry::Line(line) 
                        => {
                            pb.move_to(line.start.x/scale, line.start.y/scale);
                            pb.line_to(line.end.x/scale, line.end.y/scale);
                        },
                    geo::Geometry::LineString(line_string) 
                        => {
                            for (j, line) in line_string.lines().enumerate() {
                                if j == 0 {
                                    pb.move_to(line.start.x/scale, line.start.y/scale);
                                    pb.line_to(line.end.x/scale, line.end.y/scale);
                                } else {
                                    pb.line_to(line.start.x/scale, line.start.y/scale);
                                    pb.line_to(line.end.x/scale, line.end.y/scale);
                                }
                            }
                        },
                    geo::Geometry::Polygon(polygon) 
                        => {
                            for (j, line) in polygon.exterior().0.iter().enumerate() {
                                if j == 0 {
                                    pb.move_to(line.x/scale, line.y/scale);
                                    pb.line_to(line.x/scale, line.y/scale);
                                } else {
                                    pb.line_to(line.x/scale, line.y/scale);
                                    pb.line_to(line.x/scale, line.y/scale);
                                }
                            }
                        },
                    geo::Geometry::MultiPolygon(multi_polygon)
                        => {
                            for polygon in multi_polygon {
                                for (j, line) in polygon.exterior().0.iter().enumerate() {
                                    if j == 0 {
                                        pb.move_to(line.x/scale, line.y/scale);
                                        pb.line_to(line.x/scale, line.y/scale);
                                    } else {
                                        pb.line_to(line.x/scale, line.y/scale);
                                        pb.line_to(line.x/scale, line.y/scale);
                                    }
                                }
                            }
                        },
                    geo::Geometry::MultiPoint(multi_point) 
                        => {
                            for point in multi_point {
                                pb.move_to(point.x()/scale, point.y()/scale);
                                pb.line_to(point.x()/scale + 1.0, point.y()/scale + 1.0);
                                pb.line_to(point.x()/scale + 1.0, point.y()/scale);
                                pb.line_to(point.x()/scale, point.y()/scale + 1.0)};
                        },
                    geo::Geometry::MultiLineString(multi_line_string) 
                        => {
                            for line_string in multi_line_string {
                                for (j, line) in line_string.lines().enumerate() {
                                    if j == 0 {
                                        pb.move_to(line.start.x/scale, line.start.y/scale);
                                        pb.line_to(line.end.x/scale, line.end.y/scale);
                                    } else {
                                        pb.line_to(line.start.x/scale, line.start.y/scale);
                                        pb.line_to(line.end.x/scale, line.end.y/scale);
                                    }
                                }
                            }
                        },
                    geo::Geometry::GeometryCollection(geometry_collection) => {
                        println!("GeometryCollection: {:?}", geometry_collection);
                    },
                    geo::Geometry::Rect(rect) => {
                        println!("Rect: {:?}", rect);
                    },
                    geo::Geometry::Triangle(triangle) => {
                        println!("Triangle: {:?}", triangle);
                    },
                }
            }
        }
    }
    
    let path = pb.finish();

    let stroke_style = StrokeStyle {
        cap: raqote::LineCap::Round,
        join: raqote::LineJoin::Round,
        width: 2.5,
        miter_limit: 10.0,
        dash_array: vec![],
        dash_offset: 0.0,
    };

    dt.stroke(
        &path,
    &Source::Solid(SolidSource {
            r: 0xff,
            g: 0xff,
            b: 0xff,
            a: 0xff,
        }),        
        
        &stroke_style,
        &DrawOptions {
            antialias: AntialiasMode::Gray,
            ..Default::default()
        },
    );

    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        dt.get_data_u8().to_vec(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}