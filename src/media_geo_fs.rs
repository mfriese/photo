//! Derived per-photo GPS/location rows. No YAML or original media writes.
use crate::{
    domain::{geo::Coordinates, tags::LibraryFile},
    library_db::{self, Database},
};
use photosuite_application::{
    geodata::Place,
    media_geo::{Fingerprint, Position, Repository, ResolvedPlace, Snapshot},
    photo_location::CoordinateSource,
};
use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::UNIX_EPOCH,
};
pub struct SqlitePhotoGeo {
    pub database: Arc<Database>,
    pub coordinates: Arc<dyn CoordinateSource>,
}
fn version(root: &Path, photo: &LibraryFile) -> Result<Fingerprint, String> {
    let path = crate::metadata_fs::library_path(root, photo.as_str(), false)?;
    if !crate::metadata_fs::check_file(&path)? {
        return Err("Foto ist nicht erreichbar".into());
    }
    let m = fs::metadata(path).map_err(|e| e.to_string())?;
    let modified = m.modified().map_err(|e| e.to_string())?;
    let modified = match modified.duration_since(UNIX_EPOCH) {
        Ok(t) => format!("{}.{:09}", t.as_secs(), t.subsec_nanos()),
        Err(e) => format!(
            "-{}.{:09}",
            e.duration().as_secs(),
            e.duration().subsec_nanos()
        ),
    };
    Ok(Fingerprint {
        bytes: m.len(),
        modified,
    })
}
fn cancelled(stop: &AtomicBool) -> Result<(), String> {
    if stop.load(Ordering::Relaxed) {
        Err("GPS-Prüfung abgebrochen".into())
    } else {
        Ok(())
    }
}
impl Repository for SqlitePhotoGeo {
    fn inspect(
        &self,
        root: &Path,
        photo: &LibraryFile,
        stop: &AtomicBool,
    ) -> Result<Snapshot, String> {
        cancelled(stop)?;
        let fingerprint = version(root, photo)?;
        let cached=self.database.transaction(root,|tx|{
   library_db::optional(tx.query_row("SELECT status,latitude,longitude,error,place_id,place_name,place_ascii_name,place_latitude,place_longitude,country_code,admin1_code,distance_meters,dataset_sha256 FROM media_geo WHERE media_path=?1 AND source_bytes=?2 AND source_modified=?3",rusqlite::params![photo.as_str(),fingerprint.bytes,fingerprint.modified],|r|{
    let status:String=r.get(0)?;let point=match status.as_str(){"missing"=>Position::Missing,"invalid"=>Position::Invalid(r.get(3)?),"available"=>Position::Present(Coordinates::new(r.get(1)?,r.get(2)?).map_err(library_db::invalid)?),_=>return Err(library_db::invalid("Ungültiger GPS-Cachestatus"))};
    let resolved=if let Some(id)=r.get::<_,Option<i64>>(4)? {Some(ResolvedPlace {place:Place {id,name:r.get(5)?,ascii_name:r.get(6)?,latitude:r.get(7)?,longitude:r.get(8)?,country_code:r.get(9)?,admin1_code:r.get(10)?},distance_meters:r.get(11)?,dataset_sha256:r.get(12)?})}else{None};
    Ok(Snapshot {fingerprint:fingerprint.clone(),position:point,resolved})
   }))
  })?;
        if let Some(cached) = cached
            && !matches!(cached.position, Position::Invalid(_))
        {
            return Ok(cached);
        }
        let position = match self.coordinates.coordinates(root, photo, stop) {
            Ok(Some(point)) => Position::Present(point),
            Ok(None) => Position::Missing,
            Err(error) => {
                cancelled(stop)?;
                Position::Invalid(error)
            }
        };
        cancelled(stop)?;
        if version(root, photo)? != fingerprint {
            return Err("Foto während der GPS-Prüfung geändert; erneut versuchen".into());
        }
        self.database.transaction(root,|tx|{
   library_db::media(tx,photo)?;
   let (status,latitude,longitude,error)=match &position{Position::Missing=>("missing",None,None,String::new()),Position::Invalid(error)=>("invalid",None,None,error.chars().take(2048).collect()),Position::Present(point)=>("available",Some(point.latitude()),Some(point.longitude()),String::new())};
   tx.execute("INSERT INTO media_geo(media_path,source_bytes,source_modified,status,latitude,longitude,error) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(media_path) DO UPDATE SET source_bytes=excluded.source_bytes,source_modified=excluded.source_modified,status=excluded.status,latitude=excluded.latitude,longitude=excluded.longitude,error=excluded.error,place_id=NULL,place_name=NULL,place_ascii_name=NULL,place_latitude=NULL,place_longitude=NULL,country_code=NULL,admin1_code=NULL,distance_meters=NULL,dataset_sha256=NULL WHERE media_geo.source_bytes!=excluded.source_bytes OR media_geo.source_modified!=excluded.source_modified OR media_geo.status!=excluded.status OR media_geo.error!=excluded.error OR media_geo.latitude IS NOT excluded.latitude OR media_geo.longitude IS NOT excluded.longitude",rusqlite::params![photo.as_str(),fingerprint.bytes,fingerprint.modified,status,latitude,longitude,error])?;
   tx.execute("UPDATE index_state SET change_sequence=change_sequence+1 WHERE id=1",[])?;Ok(())
  })?;
        Ok(Snapshot {
            fingerprint,
            position,
            resolved: None,
        })
    }
    fn save_place(
        &self,
        root: &Path,
        photo: &LibraryFile,
        expected: &Fingerprint,
        resolved: &ResolvedPlace,
        stop: &AtomicBool,
    ) -> Result<(), String> {
        cancelled(stop)?;
        if version(root, photo)? != *expected {
            return Err("Foto während der Ortssuche geändert; erneut öffnen".into());
        }
        if !resolved.distance_meters.is_finite()
            || resolved.distance_meters < 0.
            || resolved.dataset_sha256.len() != 64
        {
            return Err("Ungültiges Ortsergebnis".into());
        }
        self.database.transaction(root,|tx|{
   let place=&resolved.place;
   let changed=tx.execute("UPDATE media_geo SET place_id=?1,place_name=?2,place_ascii_name=?3,place_latitude=?4,place_longitude=?5,country_code=?6,admin1_code=?7,distance_meters=?8,dataset_sha256=?9 WHERE media_path=?10 AND source_bytes=?11 AND source_modified=?12 AND status='available'",rusqlite::params![place.id,place.name,place.ascii_name,place.latitude,place.longitude,place.country_code,place.admin1_code,resolved.distance_meters,resolved.dataset_sha256,photo.as_str(),expected.bytes,expected.modified])?;
   if changed!=1{return Err(library_db::invalid("GPS-Cache wurde geändert; Ortssuche wiederholen"));}
   tx.execute("UPDATE index_state SET change_sequence=change_sequence+1 WHERE id=1",[])?;Ok(())
  })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    struct Source {
        calls: AtomicUsize,
    }
    impl CoordinateSource for Source {
        fn coordinates(
            &self,
            _: &Path,
            _: &LibraryFile,
            _: &AtomicBool,
        ) -> Result<Option<Coordinates>, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Some(Coordinates::new(52.52, 13.41).unwrap()))
        }
    }
    #[test]
    fn geo_cache_has_foreign_key_is_searchable_recalculates_after_loss_and_never_writes_yaml() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("photo.jpg"), b"original").unwrap();
        let photo = LibraryFile::new("photo.jpg".into()).unwrap();
        let source = Arc::new(Source {
            calls: AtomicUsize::new(0),
        });
        let database = Arc::new(Database);
        let repo = SqlitePhotoGeo {
            database: database.clone(),
            coordinates: source.clone(),
        };
        let stop = AtomicBool::new(false);
        let snapshot = repo.inspect(root.path(), &photo, &stop).unwrap();
        assert!(matches!(snapshot.position, Position::Present(_)));
        repo.inspect(root.path(), &photo, &stop).unwrap();
        assert_eq!(source.calls.load(Ordering::SeqCst), 1);
        let resolved = ResolvedPlace {
            place: Place {
                id: 2950159,
                name: "Berlin".into(),
                ascii_name: "Berlin".into(),
                latitude: 52.52,
                longitude: 13.41,
                country_code: "DE".into(),
                admin1_code: "16".into(),
            },
            distance_meters: 0.,
            dataset_sha256: "a".repeat(64),
        };
        repo.save_place(root.path(), &photo, &snapshot.fingerprint, &resolved, &stop)
            .unwrap();
        assert_eq!(
            repo.inspect(root.path(), &photo, &stop)
                .unwrap()
                .resolved
                .unwrap()
                .place
                .name,
            "Berlin"
        );
        database.transaction(root.path(),|tx|{let count:i64=tx.query_row("SELECT count(*) FROM geo_overview WHERE country_code='DE' AND place_name='Berlin'",[],|r|r.get(0))?;assert_eq!(count,1);let target:String=tx.query_row("SELECT \"table\" FROM pragma_foreign_key_list('media_geo')",[],|r|r.get(0))?;assert_eq!(target,"media");Ok(())}).unwrap();
        assert!(!root.path().join(photo.metadata_path().unwrap()).exists());
        assert_eq!(
            fs::read(root.path().join("photo.jpg")).unwrap(),
            b"original"
        );
        fs::write(root.path().join("photo.jpg"), b"changed original metadata").unwrap();
        assert!(
            repo.inspect(root.path(), &photo, &stop)
                .unwrap()
                .resolved
                .is_none()
        );
        assert_eq!(source.calls.load(Ordering::SeqCst), 2);
        assert!(
            repo.save_place(root.path(), &photo, &snapshot.fingerprint, &resolved, &stop)
                .is_err()
        );
        fs::remove_file(root.path().join(crate::library_db::FILE)).unwrap();
        repo.inspect(root.path(), &photo, &stop).unwrap();
        assert_eq!(source.calls.load(Ordering::SeqCst), 3);
        assert!(!root.path().join(photo.metadata_path().unwrap()).exists());
    }
}
