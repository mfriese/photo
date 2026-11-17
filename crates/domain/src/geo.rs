//! Geographic values and spherical distances, independent of EXIF, SQLite and presentation.
const EARTH_RADIUS_METERS: f64 = 6_371_008.8;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coordinates {
    latitude: f64,
    longitude: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct SearchBox {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}
impl Coordinates {
    pub fn new(latitude: f64, longitude: f64) -> Result<Self, String> {
        if !latitude.is_finite()
            || !longitude.is_finite()
            || !(-90. ..=90.).contains(&latitude)
            || !(-180. ..=180.).contains(&longitude)
        {
            return Err("GPS-Koordinaten liegen außerhalb des gültigen Bereichs".into());
        }
        Ok(Self {
            latitude,
            longitude,
        })
    }
    pub fn latitude(self) -> f64 {
        self.latitude
    }
    pub fn longitude(self) -> f64 {
        self.longitude
    }
    pub fn distance_meters(self, other: Self) -> f64 {
        let delta_lat = (other.latitude - self.latitude).to_radians();
        let delta_lon = (other.longitude - self.longitude).to_radians();
        let a = (delta_lat / 2.).sin().powi(2)
            + self.latitude.to_radians().cos()
                * other.latitude.to_radians().cos()
                * (delta_lon / 2.).sin().powi(2);
        2. * EARTH_RADIUS_METERS * a.clamp(0., 1.).sqrt().asin()
    }
    /// Conservative boxes covering a spherical cap, split at the date line.
    pub fn search_boxes(self, radius_meters: f64) -> Result<Vec<SearchBox>, String> {
        if !radius_meters.is_finite() || radius_meters <= 0. {
            return Err("Suchradius muss positiv und endlich sein".into());
        }
        let angle = (radius_meters / EARTH_RADIUS_METERS).min(std::f64::consts::PI);
        let latitude = self.latitude.to_radians();
        let south = (latitude - angle)
            .max(-std::f64::consts::FRAC_PI_2)
            .to_degrees();
        let north = (latitude + angle)
            .min(std::f64::consts::FRAC_PI_2)
            .to_degrees();
        if latitude - angle <= -std::f64::consts::FRAC_PI_2
            || latitude + angle >= std::f64::consts::FRAC_PI_2
        {
            return Ok(vec![SearchBox {
                west: -180.,
                south,
                east: 180.,
                north,
            }]);
        }
        let span = (angle.sin() / latitude.cos())
            .clamp(-1., 1.)
            .asin()
            .to_degrees()
            + 1e-9;
        let west = self.longitude - span;
        let east = self.longitude + span;
        let south = (south - 1e-9).max(-90.);
        let north = (north + 1e-9).min(90.);
        if west < -180. {
            Ok(vec![
                SearchBox {
                    west: -180.,
                    south,
                    east,
                    north,
                },
                SearchBox {
                    west: west + 360.,
                    south,
                    east: 180.,
                    north,
                },
            ])
        } else if east > 180. {
            Ok(vec![
                SearchBox {
                    west,
                    south,
                    east: 180.,
                    north,
                },
                SearchBox {
                    west: -180.,
                    south,
                    east: east - 360.,
                    north,
                },
            ])
        } else {
            Ok(vec![SearchBox {
                west,
                south,
                east,
                north,
            }])
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coordinates_distances_and_search_caps_handle_zero_dateline_and_poles() {
        assert!(Coordinates::new(f64::NAN, 0.).is_err());
        assert!(Coordinates::new(91., 0.).is_err());
        assert!(Coordinates::new(0., 181.).is_err());
        assert_eq!(
            Coordinates::new(0., 0.)
                .unwrap()
                .distance_meters(Coordinates::new(0., 0.).unwrap()),
            0.
        );
        let a = Coordinates::new(0., 179.9).unwrap();
        let b = Coordinates::new(0., -179.9).unwrap();
        assert!((a.distance_meters(b) - 22_239.).abs() < 2.);
        for center in [
            a,
            Coordinates::new(89.9, 140.).unwrap(),
            Coordinates::new(-89.9, -50.).unwrap(),
            Coordinates::new(52., 13.).unwrap(),
        ] {
            for radius in [50_000., 1_000_000., 15_000_000., 21_000_000.] {
                let boxes = center.search_boxes(radius).unwrap();
                for latitude in (-90..=90).step_by(10) {
                    for longitude in (-180..=180).step_by(10) {
                        let point = Coordinates::new(latitude as f64, longitude as f64).unwrap();
                        if center.distance_meters(point) <= radius {
                            assert!(boxes.iter().any(|b| point.longitude >= b.west
                                && point.longitude <= b.east
                                && point.latitude >= b.south
                                && point.latitude <= b.north));
                        }
                    }
                }
            }
        }
    }
}
