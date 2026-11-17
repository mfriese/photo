//! Read-only EXIF GPS adapter. No external command, media writes or network access.
use crate::domain::{geo::Coordinates, tags::LibraryFile};
use photosuite_application::photo_location::CoordinateSource;
use std::{
    fs,
    io::BufReader,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
pub struct ExifCoordinates;
fn coordinate(value: &exif::Value, reference: &exif::Value, latitude: bool) -> Result<f64, String> {
    let exif::Value::Rational(parts) = value else {
        return Err("GPS-Koordinaten haben ein ungültiges Zahlenformat".into());
    };
    if parts.len() != 3 || parts.iter().any(|n| n.denom == 0) {
        return Err("GPS-Koordinaten sind unvollständig oder ungültig".into());
    }
    let values: Vec<_> = parts.iter().map(exif::Rational::to_f64).collect();
    if values[1] >= 60. || values[2] >= 60. {
        return Err("GPS-Minuten oder -Sekunden sind ungültig".into());
    }
    let exif::Value::Ascii(reference) = reference else {
        return Err("GPS-Himmelsrichtung fehlt oder ist ungültig".into());
    };
    if reference.len() != 1 {
        return Err("GPS-Himmelsrichtung ist ungültig".into());
    }
    let sign = match (latitude, reference[0].as_slice()) {
        (true, b"N") | (false, b"E") => 1.,
        (true, b"S") | (false, b"W") => -1.,
        _ => return Err("GPS-Himmelsrichtung ist ungültig".into()),
    };
    Ok(sign * (values[0] + values[1] / 60. + values[2] / 3600.))
}
fn fields(exif: &exif::Exif) -> Result<Option<Coordinates>, String> {
    let tags = [
        exif::Tag::GPSLatitude,
        exif::Tag::GPSLatitudeRef,
        exif::Tag::GPSLongitude,
        exif::Tag::GPSLongitudeRef,
    ];
    let values: Vec<_> = tags
        .iter()
        .map(|tag| exif.get_field(*tag, exif::In::PRIMARY))
        .collect();
    if values.iter().all(|field| field.is_none()) {
        return Ok(None);
    }
    if values.iter().any(|field| field.is_none()) {
        return Err("Das Bild enthält unvollständige GPS-Koordinaten".into());
    }
    let latitude = coordinate(&values[0].unwrap().value, &values[1].unwrap().value, true)?;
    let longitude = coordinate(&values[2].unwrap().value, &values[3].unwrap().value, false)?;
    Coordinates::new(latitude, longitude).map(Some)
}
impl CoordinateSource for ExifCoordinates {
    fn coordinates(
        &self,
        root: &Path,
        photo: &LibraryFile,
        stop: &AtomicBool,
    ) -> Result<Option<Coordinates>, String> {
        if stop.load(Ordering::Relaxed) {
            return Err("Ortssuche abgebrochen".into());
        }
        if !crate::domain::Media::from_path(photo.as_str().into(), String::new())
            .is_some_and(|m| m.is_photo())
        {
            return Err("Ortssuche benötigt ein Foto".into());
        }
        let path = crate::metadata_fs::library_path(root, photo.as_str(), false)?;
        if !crate::metadata_fs::check_file(&path)? {
            return Err("Foto ist aktuell nicht erreichbar".into());
        }
        let file = fs::File::open(path).map_err(|e| e.to_string())?;
        let exif = match exif::Reader::new().read_from_container(&mut BufReader::new(file)) {
            Ok(value) => value,
            Err(exif::Error::NotFound(_)) => return Ok(None),
            Err(error) => {
                return Err(format!(
                    "GPS-Metadaten konnten nicht gelesen werden: {error}"
                ));
            }
        };
        if stop.load(Ordering::Relaxed) {
            return Err("Ortssuche abgebrochen".into());
        }
        fields(&exif)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn field(tag: exif::Tag, value: exif::Value) -> exif::Field {
        exif::Field {
            tag,
            ifd_num: exif::In::PRIMARY,
            value,
        }
    }
    fn dms(degrees: u32, minutes: u32, seconds: u32) -> exif::Value {
        exif::Value::Rational(vec![
            (degrees, 1).into(),
            (minutes, 1).into(),
            (seconds, 1).into(),
        ])
    }
    pub(crate) fn jpeg(path: &Path, fields: Vec<exif::Field>) {
        let mut image = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut image)
            .encode(&[120, 80, 30], 1, 1, image::ExtendedColorType::Rgb8)
            .unwrap();
        if fields.is_empty() {
            fs::write(path, image).unwrap();
            return;
        }
        let mut writer = exif::experimental::Writer::new();
        for field in &fields {
            writer.push_field(field);
        }
        let mut tiff = std::io::Cursor::new(Vec::new());
        writer.write(&mut tiff, false).unwrap();
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend(tiff.into_inner());
        let mut result = image[..2].to_vec();
        result.extend([0xff, 0xe1]);
        result.extend(((payload.len() + 2) as u16).to_be_bytes());
        result.extend(payload);
        result.extend(&image[2..]);
        fs::write(path, result).unwrap();
    }
    #[test]
    fn real_jpeg_gps_handles_missing_zero_hemispheres_and_invalid_values_without_writes() {
        let root = tempfile::tempdir().unwrap();
        let photo = LibraryFile::new("photo.jpg".into()).unwrap();
        let path = root.path().join("photo.jpg");
        let stop = AtomicBool::new(false);
        jpeg(&path, vec![]);
        assert!(
            ExifCoordinates
                .coordinates(root.path(), &photo, &stop)
                .unwrap()
                .is_none()
        );
        let gps = |latitude, reference| {
            vec![
                field(exif::Tag::GPSLatitude, latitude),
                field(
                    exif::Tag::GPSLatitudeRef,
                    exif::Value::Ascii(vec![reference]),
                ),
                field(exif::Tag::GPSLongitude, dms(122, 20, 0)),
                field(
                    exif::Tag::GPSLongitudeRef,
                    exif::Value::Ascii(vec![b"W".to_vec()]),
                ),
            ]
        };
        jpeg(&path, gps(dms(47, 30, 0), b"S".to_vec()));
        let before = fs::read(&path).unwrap();
        let point = ExifCoordinates
            .coordinates(root.path(), &photo, &stop)
            .unwrap()
            .unwrap();
        assert_eq!(point.latitude(), -47.5);
        assert!((point.longitude() + 122.333333).abs() < 0.000001);
        assert_eq!(fs::read(&path).unwrap(), before);
        for fields in [
            vec![field(exif::Tag::GPSLatitude, dms(47, 0, 0))],
            gps(dms(100, 0, 0), b"N".to_vec()),
            gps(dms(10, 60, 0), b"N".to_vec()),
            gps(dms(10, 0, 0), b"Q".to_vec()),
            gps(
                exif::Value::Rational(vec![(1, 0).into(), (0, 1).into(), (0, 1).into()]),
                b"N".to_vec(),
            ),
        ] {
            jpeg(&path, fields);
            assert!(
                ExifCoordinates
                    .coordinates(root.path(), &photo, &stop)
                    .is_err()
            );
        }
        jpeg(
            &path,
            vec![
                field(exif::Tag::GPSLatitude, dms(0, 0, 0)),
                field(
                    exif::Tag::GPSLatitudeRef,
                    exif::Value::Ascii(vec![b"N".to_vec()]),
                ),
                field(exif::Tag::GPSLongitude, dms(0, 0, 0)),
                field(
                    exif::Tag::GPSLongitudeRef,
                    exif::Value::Ascii(vec![b"E".to_vec()]),
                ),
            ],
        );
        let point = ExifCoordinates
            .coordinates(root.path(), &photo, &stop)
            .unwrap()
            .unwrap();
        assert_eq!(point, Coordinates::new(0., 0.).unwrap());
    }
    #[cfg(unix)]
    #[test]
    fn gps_reader_rejects_symbolic_sources() {
        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("outside.jpg");
        jpeg(&outside, vec![]);
        std::os::unix::fs::symlink(&outside, root.path().join("photo.jpg")).unwrap();
        assert!(
            ExifCoordinates
                .coordinates(
                    root.path(),
                    &LibraryFile::new("photo.jpg".into()).unwrap(),
                    &AtomicBool::new(false)
                )
                .is_err()
        );
    }
}
