use bevy::math::Vec2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord {
    pub lat: f32,
    pub long: f32,
}

impl Coord {
    pub const fn new(lat: f32, long: f32) -> Self {
        Self {
            lat,
            long,
        }
    }

    pub fn to_vec2(&self) -> Vec2 {
        Vec2::new(self.lat, self.long)
    }

    pub fn to_tile_coords(&self, zoom: u32) -> (u32, u32) {
        let x = ((self.long + 180.0) / 360.0 * (2_i32.pow(zoom) as f32)).floor() as i32;
        let y = ((1.0 - (self.lat.to_radians().tan() + 1.0 / self.lat.to_radians().cos()).ln() / std::f32::consts::PI) / 2.0 * (2_i32.pow(zoom) as f32)).floor() as i32;
        (x as u32, y as u32)
    }
}

pub fn tile_to_coords(x: i32, y: i32, zoom: u32) -> Coord {
    let n = 2_i32.pow(zoom) as f32;
    let lon = x as f32 / n * 360.0 - 180.0;
    let lat_rad = (std::f32::consts::PI * (1.0 - 2.0 * y as f32 / n)).sinh().atan();
    let lat = lat_rad.to_degrees();
    Coord::new(lat, lon)
}