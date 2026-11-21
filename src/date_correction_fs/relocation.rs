//! Exclusive, verified media transfer with a durable rollback journal. No user-library migrations.
use super::*;
use crate::media_records_fs::Record;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
};
const PREFIX: &str = ".photosuite-relocation-";
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    size: u64,
    modified: String,
    device: Option<u64>,
    inode: Option<u64>,
}
impl Identity {
    fn read(file: &File) -> Result<Self, String> {
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        #[cfg(unix)]
        let (device, inode) = {
            use std::os::unix::fs::MetadataExt;
            (Some(metadata.dev()), Some(metadata.ino()))
        };
        #[cfg(not(unix))]
        let (device, inode) = (None, None);
        Ok(Self {
            size: metadata.len(),
            modified: format!("{:?}", metadata.modified().map_err(|e| e.to_string())?),
            device,
            inode,
        })
    }
    fn same_object(&self, other: &Self) -> bool {
        self.device == other.device && self.inode == other.inode
    }
    fn unchanged(&self, other: &Self) -> bool {
        self.same_object(other) && self.size == other.size && self.modified == other.modified
    }
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    version: u32,
    source: String,
    target: String,
    old_yaml: String,
    new_yaml: String,
    previous: Option<String>,
    next: String,
    source_identity: Identity,
    target_identity: Option<Identity>,
    ready: bool,
    committed: bool,
}
fn path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    crate::metadata_fs::library_path(root, relative, false)
}
fn collision(relative: &str) -> String {
    format!(
        "Kollision: {relative} ist bereits vorhanden. Nichts geändert. Bitte Datum oder Uhrzeit anpassen."
    )
}
fn exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}
fn open(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Keine reguläre Datei".into());
    }
    Ok(file)
}
fn equal(a: &File, b: &File) -> Result<bool, String> {
    let mut a = a.try_clone().map_err(|e| e.to_string())?;
    let mut b = b.try_clone().map_err(|e| e.to_string())?;
    a.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    b.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut left = [0_u8; 65536];
    let mut right = [0_u8; 65536];
    loop {
        let n = a.read(&mut left).map_err(|e| e.to_string())?;
        let m = b.read(&mut right).map_err(|e| e.to_string())?;
        if n != m || left[..n] != right[..m] {
            return Ok(false);
        }
        if n == 0 {
            return Ok(true);
        }
    }
}
fn copy(source: &File, target: &File) -> Result<(), String> {
    let metadata = source.metadata().map_err(|e| e.to_string())?;
    let mut source = source.try_clone().map_err(|e| e.to_string())?;
    let mut target = target.try_clone().map_err(|e| e.to_string())?;
    source.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    std::io::copy(&mut source, &mut target).map_err(|e| e.to_string())?;
    target
        .set_times(
            fs::FileTimes::new().set_modified(metadata.modified().map_err(|e| e.to_string())?),
        )
        .map_err(|e| e.to_string())?;
    target
        .set_permissions(metadata.permissions())
        .map_err(|e| e.to_string())?;
    target.sync_all().map_err(|e| e.to_string())?;
    if !equal(&source, &target)? {
        return Err("Dateikopie stimmt nicht bytegenau mit der Quelle überein".into());
    }
    Ok(())
}
fn create(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                collision(&path.display().to_string())
            } else {
                e.to_string()
            }
        })
}
fn relative(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map_err(|e| e.to_string())?
        .to_str()
        .map(str::to_owned)
        .ok_or("Nicht darstellbarer Bibliothekspfad".into())
}
struct JournalFile {
    path: PathBuf,
    lock_path: PathBuf,
    _lock: File,
    expected: Option<Vec<u8>>,
}
impl JournalFile {
    fn create(root: &Path) -> Result<Self, String> {
        let temporary = tempfile::Builder::new()
            .prefix(PREFIX)
            .suffix(".lock")
            .tempfile_in(root.join("yaml"))
            .map_err(|e| e.to_string())?;
        if !lock(temporary.as_file())? {
            return Err("Verschiebetransaktion bereits gesperrt".into());
        }
        let (lock_file, lock_path) = temporary.keep().map_err(|e| e.to_string())?;
        Ok(Self {
            path: lock_path.with_extension("yaml"),
            lock_path,
            _lock: lock_file,
            expected: None,
        })
    }
    fn save(&mut self, journal: &Journal) -> Result<(), String> {
        let bytes = serde_saphyr::to_string(journal)
            .map_err(|e| e.to_string())?
            .into_bytes();
        crate::metadata_fs::atomic_write_if_current(&self.path, &bytes, self.expected.as_deref())?;
        self.expected = Some(bytes);
        sync_directory(self.path.parent().ok_or("Journalordner fehlt")?)?;
        Ok(())
    }
    fn cleanup(&self) -> Result<(), String> {
        if exists(&self.path)? {
            let mut bytes = Vec::new();
            open(&self.path)?
                .take(64 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if self.expected.as_deref() != Some(bytes.as_slice()) {
                return Err("Wiederherstellungsdatei extern geändert; bleibt erhalten".into());
            }
            fs::remove_file(&self.path).map_err(|e| e.to_string())?;
        }
        if exists(&self.lock_path)? {
            if !Identity::read(&self._lock)?.same_object(&Identity::read(&open(&self.lock_path)?)?)
            {
                return Err("Journal-Sperrdatei extern ersetzt; bleibt erhalten".into());
            }
            fs::remove_file(&self.lock_path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
fn sync_directory(path: &Path) -> Result<(), String> {
    match File::open(path).and_then(|file| file.sync_all()) {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::Unsupported | std::io::ErrorKind::InvalidInput
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    }
}
fn validate(journal: &Journal) -> Result<(), String> {
    if journal.version != 1 {
        return Err("Unbekannte Verschiebe-Wiederherstellung".into());
    }
    let source = LibraryFile::new(journal.source.clone())?;
    let target = LibraryFile::new(journal.target.clone())?;
    let next = media_records_fs::parse(journal.next.as_bytes())?;
    let date = next
        .corrected_date()?
        .ok_or("Wiederherstellung ohne Zieldatum")?;
    if next.file != target.as_str()
        || photosuite_application::date_correction::relocation_target(&source, &date)? != target
        || journal.new_yaml != target.metadata_path()?
        || ![source.metadata_path()?, source.metadata_collision_path()?].contains(&journal.old_yaml)
    {
        return Err("Inkonsistente Verschiebe-Wiederherstellung; Dateien bleiben erhalten".into());
    }
    if let Some(previous) = &journal.previous
        && media_records_fs::parse(previous.as_bytes())?.file != source.as_str()
    {
        return Err("Wiederherstellung gehört zu einer anderen Quelldatei".into());
    }
    Ok(())
}
fn remove_yaml(path: &Path, expected: &[u8]) -> Result<(), String> {
    if media_records_fs::read(path)?.as_deref() != Some(expected) {
        return Err(format!(
            "Medien-YAML inzwischen geändert: {}",
            path.display()
        ));
    }
    fs::remove_file(path).map_err(|e| e.to_string())
}
fn restore_old(root: &Path, journal: &Journal) -> Result<(), String> {
    let old = path(root, &journal.old_yaml)?;
    let new = path(root, &journal.new_yaml)?;
    if old == new {
        let current = media_records_fs::read(&old)?;
        if current.as_deref() == journal.previous.as_deref().map(str::as_bytes) {
            return Ok(());
        }
        match &journal.previous {
            Some(previous) => crate::metadata_fs::atomic_write_if_current(
                &old,
                previous.as_bytes(),
                Some(journal.next.as_bytes()),
            ),
            None => remove_yaml(&old, journal.next.as_bytes()),
        }
    } else {
        if let Some(previous) = &journal.previous {
            let current = media_records_fs::read(&old)?;
            if current.as_deref() != Some(previous.as_bytes()) {
                crate::metadata_fs::atomic_write_if_current(&old, previous.as_bytes(), None)?;
            }
        }
        if media_records_fs::read(&new)?.is_some() {
            remove_yaml(&new, journal.next.as_bytes())?;
        }
        Ok(())
    }
}
fn remap(
    repo: &CaptureDates,
    root: &Path,
    from: &LibraryFile,
    to: &LibraryFile,
) -> Result<(), String> {
    repo.database.transaction(root,|tx| {
        let source:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM media WHERE path=?1)",[from.as_str()],|r|r.get(0))?;
        if !source {return Ok(());}
        let occupied:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM media WHERE path=?1)",[to.as_str()],|r|r.get(0))?;
        if occupied {return Err(library_db::invalid(collision(to.as_str())));}
        library_db::record_address(tx,to,root)?;
        for (table,column) in [("media_tags","media_path"),("media_geo","media_path"),("audio_durations","media_path"),("audio_comments","photo_path"),("transcription_jobs","source_path"),("transcription_jobs","record_path")] {
            tx.execute(&format!("UPDATE {table} SET {column}=?2 WHERE {column}=?1"),(from.as_str(),to.as_str()))?;
        }
        let overrides:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='capture_date_overrides')",[],|r|r.get(0))?;
        if overrides {tx.execute("UPDATE capture_date_overrides SET media_path=?2 WHERE media_path=?1",(from.as_str(),to.as_str()))?;}
        tx.execute("DELETE FROM media WHERE path=?1",[from.as_str()])?;
        tx.execute("UPDATE media SET reference_at=(SELECT reference_at FROM media WHERE path=?1) WHERE path IN (SELECT audio_path FROM audio_comments WHERE photo_path=?1)",[to.as_str()])?;
        tx.execute("UPDATE index_state SET tags_revision=tags_revision+1,comments_revision=comments_revision+1,transcription_revision=transcription_revision+1,change_sequence=change_sequence+1",[])?;
        Ok(())
    })
}
fn rollback(repo: &CaptureDates, root: &Path, journal: &Journal) -> Result<(), String> {
    let source = path(root, &journal.source)?;
    let target = path(root, &journal.target)?;
    if journal.target_identity.is_none() && exists(&source)? {
        return Ok(());
    }
    if !exists(&source)? {
        if !journal.ready {
            return Err(
                "Quelle fehlt; unvollständiges Ziel bleibt zur manuellen Prüfung erhalten".into(),
            );
        }
        let target_file = open(&target)?;
        let expected = journal
            .target_identity
            .as_ref()
            .ok_or("Zielidentität fehlt")?;
        if !expected.unchanged(&Identity::read(&target_file)?) {
            return Err(
                "Zieldatei inzwischen verändert; keine automatische Wiederherstellung".into(),
            );
        }
        crate::metadata_fs::library_path(root, &journal.source, true)?;
        let output = create(&source)?;
        copy(&target_file, &output)?;
    }
    restore_old(root, journal)?;
    let from = LibraryFile::new(journal.target.clone())?;
    let to = LibraryFile::new(journal.source.clone())?;
    if exists(&path(root, library_db::FILE)?)? {
        remap(repo, root, &from, &to)?;
        let correction = journal
            .previous
            .as_deref()
            .map(|bytes| {
                media_records_fs::parse(bytes.as_bytes()).and_then(|record| record.corrected_date())
            })
            .transpose()?
            .flatten();
        repo.database.transaction(root,|tx| {
            if let Some(date)=correction {super::project(tx,&to,&date,root)?;} else {
                let exists:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='capture_date_overrides')",[],|row|row.get(0))?;
                if exists {tx.execute("DELETE FROM capture_date_overrides WHERE media_path=?1",[to.as_str()])?;}
                tx.execute("UPDATE media SET reference_at=recorded_at WHERE path=?1",[to.as_str()])?;
                tx.execute("UPDATE media SET reference_at=(SELECT reference_at FROM media WHERE path=?1) WHERE path IN (SELECT audio_path FROM audio_comments WHERE photo_path=?1)",[to.as_str()])?;
            }Ok(())
        })?;
    }
    if exists(&target)? {
        let current = open(&target)?;
        let Some(expected) = journal.target_identity.as_ref() else {
            return Ok(());
        };
        if !expected.same_object(&Identity::read(&current)?) {
            return Err("Zielpfad wurde ersetzt; Datei bleibt erhalten".into());
        }
        if journal.ready && !equal(&open(&source)?, &current)? {
            return Err(
                "Quelle und Ziel unterscheiden sich; beide Dateien bleiben erhalten".into(),
            );
        }
        fs::remove_file(&target).map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[cfg(unix)]
fn lock(file: &File) -> Result<bool, String> {
    use std::os::fd::AsRawFd;
    // Only the metadata transaction's own descriptor is locked; crash recovery skips active work.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::WouldBlock {
        Ok(false)
    } else {
        Err(error.to_string())
    }
}
#[cfg(not(unix))]
fn lock(_file: &File) -> Result<bool, String> {
    Err("Sicheres Einsortieren wird auf diesem Betriebssystem noch nicht unterstützt".into())
}
fn clean_parents(root: &Path, file: &Path) {
    let mut parent = file.parent();
    while let Some(directory) = parent {
        if directory == root || !directory.starts_with(root) || fs::remove_dir(directory).is_err() {
            break;
        }
        parent = directory.parent();
    }
}
pub(super) fn recover(repo: &CaptureDates, root: &Path) -> Result<(), String> {
    let folder = crate::metadata_fs::library_path(root, "yaml/.probe", false)?
        .parent()
        .ok_or("YAML-Verzeichnis fehlt")?
        .to_owned();
    if !exists(&folder)? {
        return Ok(());
    }
    for entry in fs::read_dir(&folder).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(PREFIX) || !name.ends_with(".yaml") {
            continue;
        }
        let _guard = media_records_fs::lock_records()?;
        let journal_path = path(root, &format!("yaml/{name}"))?;
        let lock_path = journal_path.with_extension("lock");
        crate::metadata_fs::check_file(&lock_path)?;
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|e| e.to_string())?;
        if !lock(&lock_file)? {
            continue;
        }
        if !exists(&journal_path)? {
            let _ = fs::remove_file(&lock_path);
            continue;
        }
        let mut bytes = Vec::new();
        open(&journal_path)?
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let journal: Journal = crate::metadata_fs::parse_large_yaml(&bytes, 64 * 1024 * 1024)
            .map_err(|e| {
                format!("Wiederherstellungsdatei {name} beschädigt; Dateien bleiben erhalten: {e}")
            })?;
        validate(&journal)?;
        if !journal.committed {
            rollback(repo, root, &journal)
                .map_err(|e| format!("Einsortieren benötigt Wiederherstellung ({name}): {e}"))?;
        } else if !exists(&path(root, &journal.target)?)? {
            return Err(
                "Abgeschlossene Verschiebung: Zieldatei fehlt; Journal bleibt erhalten".into(),
            );
        }
        fs::remove_file(&journal_path).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(&lock_path);
        if journal.committed {
            clean_parents(root, &path(root, &journal.source)?);
        }
    }
    Ok(())
}
pub(super) fn relocate(
    repo: &CaptureDates,
    root: &Path,
    file: &LibraryFile,
    date: &CaptureTime,
    expected: Option<&CaptureTime>,
) -> Result<LibraryFile, String> {
    recover(repo, root)?;
    let target = photosuite_application::date_correction::relocation_target(file, date)?;
    if target == *file {
        repo.save(root, file, date, expected)?;
        return Ok(target);
    }
    run(repo, root, file, &target, date, expected, &|_| Ok(()))?;
    Ok(target)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Copied,
    Metadata,
    Indexed,
    SourceRemoved,
    Committed,
}
fn run(
    repo: &CaptureDates,
    root: &Path,
    file: &LibraryFile,
    target: &LibraryFile,
    date: &CaptureTime,
    expected: Option<&CaptureTime>,
    step: &dyn Fn(Stage) -> Result<(), String>,
) -> Result<(), String> {
    let _guard = media_records_fs::lock_records()?;
    let source = path(root, file.as_str())?;
    let destination = path(root, target.as_str())?;
    if exists(&destination)? {
        return Err(collision(target.as_str()));
    }
    let old_yaml = media_records_fs::path(root, file, false)?;
    let new_yaml = path(root, &target.metadata_path()?)?;
    if new_yaml != old_yaml && exists(&new_yaml)? {
        return Err(collision(&target.metadata_path()?));
    }
    let alternative = path(root, &target.metadata_collision_path()?)?;
    if alternative != old_yaml && exists(&alternative)? {
        return Err(collision(&target.metadata_collision_path()?));
    }
    if exists(&path(root, library_db::FILE)?)? {
        let occupied = repo.database.read(root, |db| {
            db.query_row(
                "SELECT EXISTS(SELECT 1 FROM media WHERE path=?1)",
                [target.as_str()],
                |row| row.get::<_, bool>(0),
            )
        })?;
        if occupied {
            return Err(collision(target.as_str()));
        }
    }
    let source_file = open(&source)?;
    let source_identity = Identity::read(&source_file)?;
    let previous = media_records_fs::read(&old_yaml)?;
    let mut record = previous
        .as_deref()
        .map(media_records_fs::parse)
        .transpose()?
        .unwrap_or_else(|| Record::new(file));
    if record.file != file.as_str()
        || record
            .corrected_date()?
            .as_ref()
            .map(CaptureTime::exif_value)
            != expected.map(CaptureTime::exif_value)
    {
        return Err("Medien-YAML inzwischen geändert; Dialog erneut öffnen".into());
    }
    record.file = target.as_str().into();
    record.capture_date = Some(date.exif_value());
    if let Some(value) = record.transcripts.remove(file.as_str()) {
        record.transcripts.insert(target.as_str().into(), value);
    }
    if let Some(value) = record.transcription_failures.remove(file.as_str()) {
        record
            .transcription_failures
            .insert(target.as_str().into(), value);
    }
    let next = serde_saphyr::to_string(&record).map_err(|e| e.to_string())?;
    media_records_fs::parse(next.as_bytes())?;
    // Probe deletion permission before publishing anything, while collisions have caused no writes.
    let probe = tempfile::Builder::new()
        .prefix(".photosuite-move-check-")
        .tempfile_in(source.parent().ok_or("Quellordner fehlt")?)
        .map_err(|e| e.to_string())?;
    drop(probe);
    crate::metadata_fs::library_path(root, "yaml/.probe", true)?;
    let mut journal_file = JournalFile::create(root)?;
    let mut journal = Journal {
        version: 1,
        source: file.as_str().into(),
        target: target.as_str().into(),
        old_yaml: relative(root, &old_yaml)?,
        new_yaml: relative(root, &new_yaml)?,
        previous: previous
            .map(String::from_utf8)
            .transpose()
            .map_err(|e| e.to_string())?,
        next,
        source_identity,
        target_identity: None,
        ready: false,
        committed: false,
    };
    if let Err(error) = journal_file.save(&journal) {
        let _ = journal_file.cleanup();
        return Err(error);
    }
    let operation = (|| {
        crate::metadata_fs::library_path(root, target.as_str(), true)?;
        let output = create(&destination)?;
        journal.target_identity = Some(Identity::read(&output)?);
        journal_file.save(&journal)?;
        if journal
            .target_identity
            .as_ref()
            .is_some_and(|target| target.device != journal.source_identity.device)
        {
            return Err("Quelle und Ziel liegen auf verschiedenen Dateisystemen; Einsortieren wird dort nicht unterstützt".into());
        }
        copy(&source_file, &output)?;
        if !journal
            .source_identity
            .unchanged(&Identity::read(&open(&source)?)?)
        {
            return Err("Quelldatei während des Kopierens geändert".into());
        }
        journal.target_identity = Some(Identity::read(&output)?);
        journal.ready = true;
        journal_file.save(&journal)?;
        step(Stage::Copied)?;
        crate::metadata_fs::library_path(root, &journal.new_yaml, true)?;
        let current = media_records_fs::read(&old_yaml)?;
        if current.as_deref() != journal.previous.as_deref().map(str::as_bytes) {
            return Err("Quell-YAML während des Einsortierens geändert".into());
        }
        crate::metadata_fs::atomic_write_if_current(
            &new_yaml,
            journal.next.as_bytes(),
            if old_yaml == new_yaml {
                journal.previous.as_deref().map(str::as_bytes)
            } else {
                None
            },
        )?;
        if old_yaml != new_yaml
            && let Some(previous) = &journal.previous
        {
            remove_yaml(&old_yaml, previous.as_bytes())?;
        }
        step(Stage::Metadata)?;
        // Ensure an original row exists for metadata without a previous index.
        repo.database
            .transaction(root, |tx| library_db::media(tx, file))?;
        remap(repo, root, file, target)?;
        repo.database
            .transaction(root, |tx| super::project(tx, target, date, root))?;
        step(Stage::Indexed)?;
        let current_target = Identity::read(&open(&destination)?)?;
        if !journal
            .source_identity
            .unchanged(&Identity::read(&open(&source)?)?)
            || !journal
                .target_identity
                .as_ref()
                .is_some_and(|expected| expected.unchanged(&current_target))
            || !equal(&source_file, &open(&destination)?)?
        {
            return Err("Quelle oder Ziel verändert; Quelle wird nicht gelöscht".into());
        }
        sync_directory(destination.parent().ok_or("Zielordner fehlt")?)?;
        sync_directory(new_yaml.parent().ok_or("YAML-Zielordner fehlt")?)?;
        let old_parent = old_yaml.parent().ok_or("YAML-Quellordner fehlt")?;
        if exists(old_parent)? {
            sync_directory(old_parent)?;
        }
        fs::remove_file(&source).map_err(|e| e.to_string())?;
        sync_directory(source.parent().ok_or("Quellordner fehlt")?)?;
        step(Stage::SourceRemoved)?;
        journal.committed = true;
        journal_file.save(&journal)?;
        step(Stage::Committed)?;
        Ok(())
    })();
    match operation {
        Ok(()) => {
            if let Err(error) = journal_file.cleanup() {
                eprintln!(
                    "Einsortieren abgeschlossen; Wiederherstellungsdatei bleibt erhalten: {error}"
                );
            }
            clean_parents(root, &source);
            Ok(())
        }
        Err(error) => match rollback(repo, root, &journal) {
            Ok(()) => {
                let _ = journal_file.cleanup();
                clean_parents(root, &destination);
                Err(error)
            }
            Err(restore) => Err(format!(
                "{error}. Wiederherstellung noch offen: {restore}. Quelle/Ziel und {} bleiben erhalten.",
                journal_file.path.display()
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::transcript::{SourceVersion, Transcript, TranscriptStatus};
    use photosuite_application::{
        library_index::Rebuilder,
        transcription::queue::{Candidate, QueueStore},
    };
    fn repository() -> CaptureDates {
        CaptureDates {
            database: Arc::new(Database),
            priority: Arc::default(),
        }
    }
    fn date() -> CaptureTime {
        CaptureTime::parse("1980:07:08 12:48:45", "Korrigiertes Mediendatum").unwrap()
    }
    fn fixture(root: &Path, photo: bool) -> LibraryFile {
        let file = LibraryFile::new(format!(
            "broken/img_20111111_124844.{}",
            if photo { "jpg" } else { "mp4" }
        ))
        .unwrap();
        let source = root.join(file.as_str());
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"complete original media bytes").unwrap();
        let mut record = Record::new(&file);
        record.tags = Some(["urlaub".into()].into());
        record.capture_date = Some("1979:06:07 12:48:44".into());
        if !photo {
            let store = crate::transcription_queue_fs::SqliteTranscriptionQueue::default();
            let version = store.version(root, &file).unwrap();
            let transcript = Transcript::new(
                version,
                TranscriptStatus::Complete,
                "tiny".into(),
                "de".into(),
                "Schöne Erinnerung\nzweite Zeile".into(),
                vec![],
            )
            .unwrap();
            record.transcripts.insert(
                file.as_str().into(),
                crate::transcript_codec::TranscriptDto::from_value(&transcript),
            );
        }
        media_records_fs::write(root, &file, &record, None).unwrap();
        crate::library_index_fs::SqliteRebuilder {
            database: Arc::new(Database),
            priority: Arc::default(),
        }
        .rebuild(root, &AtomicBool::new(false), &|_| {})
        .unwrap();
        file
    }
    fn expected() -> CaptureTime {
        CaptureTime::parse("1979:06:07 12:48:44", "Korrigiertes Mediendatum").unwrap()
    }
    fn tree(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
        walkdir::WalkDir::new(root)
            .into_iter()
            .map(Result::unwrap)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| {
                (
                    entry.path().strip_prefix(root).unwrap().into(),
                    fs::read(entry.path()).unwrap(),
                )
            })
            .collect()
    }
    #[test]
    fn video_transfer_preserves_bytes_mtime_tags_transcript_and_index_reconstructibility() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let repo = repository();
        let old = fixture(root, false);
        let original = fs::read(root.join(old.as_str())).unwrap();
        let modified = fs::metadata(root.join(old.as_str()))
            .unwrap()
            .modified()
            .unwrap();
        let store = crate::transcription_queue_fs::SqliteTranscriptionQueue::default();
        let version = store.version(root, &old).unwrap();
        let target = repo
            .relocate(root, &old, &date(), Some(&expected()))
            .unwrap();
        assert_eq!(target.as_str(), "1980/07/img_19800708_124845.mp4");
        assert!(!root.join("broken").exists());
        assert_eq!(fs::read(root.join(target.as_str())).unwrap(), original);
        assert_eq!(
            fs::metadata(root.join(target.as_str()))
                .unwrap()
                .modified()
                .unwrap(),
            modified
        );
        let record = media_records_fs::parse(
            &media_records_fs::read(&media_records_fs::path(root, &target, false).unwrap())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(record.file, target.as_str());
        assert_eq!(record.tags.unwrap(), ["urlaub".into()].into());
        assert_eq!(
            record.transcripts[target.as_str()]
                .value()
                .unwrap()
                .source(),
            &version
        );
        assert!(!record.transcripts.contains_key(old.as_str()));
        assert_eq!(store.version(root, &target).unwrap(), version);
        assert!(
            store
                .existing(
                    root,
                    &Candidate {
                        source: target.clone(),
                        record: target.clone()
                    }
                )
                .unwrap()
                .is_some()
        );
        assert!(!media_records_fs::path(root, &old, false).unwrap().exists());
        repo.database
            .read(root, |db| {
                let count: i64 = db.query_row(
                    "SELECT count(*) FROM media WHERE path=?1",
                    [old.as_str()],
                    |row| row.get(0),
                )?;
                assert_eq!(count, 0);
                let count: i64 = db.query_row(
                    "SELECT count(*) FROM media_tags WHERE media_path=?1 AND tag_name='urlaub'",
                    [target.as_str()],
                    |row| row.get(0),
                )?;
                assert_eq!(count, 1);
                Ok(())
            })
            .unwrap();
        fs::remove_file(root.join(library_db::FILE)).unwrap();
        crate::library_index_fs::SqliteRebuilder {
            database: repo.database.clone(),
            priority: Arc::default(),
        }
        .rebuild(root, &AtomicBool::new(false), &|_| {})
        .unwrap();
        assert_eq!(repo.load(root).unwrap()[&target].stamp(), date().stamp());
        assert!(
            store
                .existing(
                    root,
                    &Candidate {
                        source: target.clone(),
                        record: target
                    }
                )
                .unwrap()
                .is_some()
        );
    }
    #[test]
    fn destination_media_or_metadata_collision_changes_nothing_and_never_adds_a_suffix() {
        for metadata in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path();
            let repo = repository();
            let old = fixture(root, false);
            let target =
                photosuite_application::date_correction::relocation_target(&old, &date()).unwrap();
            let occupied = if metadata {
                root.join(target.metadata_path().unwrap())
            } else {
                root.join(target.as_str())
            };
            fs::create_dir_all(occupied.parent().unwrap()).unwrap();
            fs::write(&occupied, b"foreign content must remain").unwrap();
            let before = tree(root);
            let error = repo
                .relocate(root, &old, &date(), Some(&expected()))
                .unwrap_err();
            assert!(error.contains("Kollision"));
            assert_eq!(tree(root), before);
        }
    }
    #[test]
    fn same_metadata_address_is_updated_without_leaving_an_old_broken_reference() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let repo = repository();
        let old = fixture(root, false);
        let same = CaptureTime::parse("2011:11:11 12:48:44", "test").unwrap();
        let old_path = media_records_fs::path(root, &old, false).unwrap();
        let target = repo.relocate(root, &old, &same, Some(&expected())).unwrap();
        assert_eq!(
            media_records_fs::path(root, &target, false).unwrap(),
            old_path
        );
        let record = media_records_fs::parse(&fs::read(old_path).unwrap()).unwrap();
        assert_eq!(record.file, target.as_str());
        assert!(!record.transcripts.contains_key(old.as_str()));
    }
    #[test]
    fn all_intermediate_failures_restore_original_and_yaml_and_can_be_retried() {
        for failed in [
            Stage::Copied,
            Stage::Metadata,
            Stage::Indexed,
            Stage::SourceRemoved,
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path();
            let repo = repository();
            let old = fixture(root, false);
            let original = fs::read(root.join(old.as_str())).unwrap();
            let old_yaml = media_records_fs::path(root, &old, false).unwrap();
            let bytes = fs::read(&old_yaml).unwrap();
            let target =
                photosuite_application::date_correction::relocation_target(&old, &date()).unwrap();
            let error = run(
                &repo,
                root,
                &old,
                &target,
                &date(),
                Some(&expected()),
                &|stage| {
                    if stage == failed {
                        Err("simulated I/O failure".into())
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            assert!(error.contains("simulated"), "{error}");
            assert_eq!(fs::read(root.join(old.as_str())).unwrap(), original);
            assert_eq!(fs::read(&old_yaml).unwrap(), bytes);
            assert!(!root.join(target.as_str()).exists());
            assert_eq!(repo.load(root).unwrap()[&old].stamp(), expected().stamp());
            assert!(
                fs::read_dir(root.join("yaml"))
                    .unwrap()
                    .map(Result::unwrap)
                    .all(|entry| !entry.file_name().to_string_lossy().starts_with(PREFIX))
            );
            assert_eq!(
                repo.relocate(root, &old, &date(), Some(&expected()))
                    .unwrap(),
                target
            );
        }
    }
    #[test]
    fn photo_comments_keep_recording_time_and_follow_photo_after_transfer() {
        use photosuite_application::comments::CommentRepository;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let repo = repository();
        let old = LibraryFile::new("2011/11/img_20111111_124844.jpg".into()).unwrap();
        let source = root.join(old.as_str());
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"photo bytes unchanged").unwrap();
        let audio = "2026/audio/comments/com_20261121_120000.m4a";
        let audio_path = root.join(audio);
        fs::create_dir_all(audio_path.parent().unwrap()).unwrap();
        fs::write(&audio_path, b"audio bytes unchanged").unwrap();
        let mut record = Record::new(&old);
        record.tags = Some(["commented".into()].into());
        record
            .comments
            .insert("893d3899-9d2d-4241-8a3c-9f4226353b67".into(), audio.into());
        let transcript = Transcript::new(
            SourceVersion::new(21, "1.0".into()).unwrap(),
            TranscriptStatus::Complete,
            "tiny".into(),
            "de".into(),
            "Kommentar".into(),
            vec![],
        )
        .unwrap();
        record.transcripts.insert(
            audio.into(),
            crate::transcript_codec::TranscriptDto::from_value(&transcript),
        );
        media_records_fs::write(root, &old, &record, None).unwrap();
        crate::library_index_fs::SqliteRebuilder {
            database: repo.database.clone(),
            priority: Arc::default(),
        }
        .rebuild(root, &AtomicBool::new(false), &|_| {})
        .unwrap();
        repo.database.transaction(root,|tx| {tx.execute("INSERT INTO media_geo(media_path,source_bytes,source_modified,status,latitude,longitude) VALUES(?1,21,'1.0','available',52,13)",[old.as_str()])?;Ok(())}).unwrap();
        let target = repo.relocate(root, &old, &date(), None).unwrap();
        let comments = crate::comments_fs::SqliteComments {
            database: repo.database.clone(),
            priority: Arc::default(),
        };
        let photosuite_application::comments::CommentCache::Current(stored) =
            comments.read_cache(root).unwrap()
        else {
            panic!("cache missing")
        };
        let comment = stored.catalog.entries().values().next().unwrap();
        assert_eq!(comment.photo(), &target);
        assert_eq!(comment.reference_time().unwrap().stamp(), date().stamp());
        assert_eq!(comment.recorded_at().time().year(), "2026");
        assert_eq!(fs::read(&audio_path).unwrap(), b"audio bytes unchanged");
        assert_eq!(
            fs::read(root.join(target.as_str())).unwrap(),
            b"photo bytes unchanged"
        );
        let saved = media_records_fs::parse(
            &fs::read(media_records_fs::path(root, &target, false).unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(saved.transcripts[audio].value().unwrap(), transcript);
        repo.database
            .read(root, |db| {
                let (lat, lon): (f64, f64) = db.query_row(
                    "SELECT latitude,longitude FROM media_geo WHERE media_path=?1",
                    [target.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                assert_eq!((lat, lon), (52., 13.));
                Ok(())
            })
            .unwrap();
    }
}

#[cfg(test)]
mod crash_tests {
    use super::*;
    use photosuite_application::library_index::Rebuilder;
    #[test]
    #[ignore = "Only invoked by the isolated recovery parent with a temporary library"]
    fn relocation_crash_child() {
        let root = PathBuf::from(
            std::env::var_os("PHOTOSUITE_MOVE_TEST_ROOT").expect("parent fixture missing"),
        );
        let stage = std::env::var("PHOTOSUITE_MOVE_TEST_STAGE").unwrap();
        let old = LibraryFile::new("broken/img_20111111_124844.mp4".into()).unwrap();
        let date = CaptureTime::parse("1980:07:08 12:48:45", "test").unwrap();
        let target =
            photosuite_application::date_correction::relocation_target(&old, &date).unwrap();
        let expected = CaptureTime::parse("1979:06:07 12:48:44", "test").unwrap();
        let repo = CaptureDates {
            database: Arc::new(Database),
            priority: Arc::default(),
        };
        let _ = run(
            &repo,
            &root,
            &old,
            &target,
            &date,
            Some(&expected),
            &|current| {
                if format!("{current:?}") == stage {
                    std::process::exit(73);
                }
                Ok(())
            },
        );
        panic!("requested crash point was not reached");
    }
    #[test]
    fn interrupted_process_recovers_each_durable_boundary_without_losing_media_or_tags() {
        for stage in [
            "Copied",
            "Metadata",
            "Indexed",
            "SourceRemoved",
            "Committed",
        ] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path();
            let repo = CaptureDates {
                database: Arc::new(Database),
                priority: Arc::default(),
            };
            let old = LibraryFile::new("broken/img_20111111_124844.mp4".into()).unwrap();
            let source = root.join(old.as_str());
            fs::create_dir_all(source.parent().unwrap()).unwrap();
            fs::write(&source, b"crash-safe original bytes").unwrap();
            let mut record = Record::new(&old);
            record.tags = Some(["urlaub".into()].into());
            record.capture_date = Some("1979:06:07 12:48:44".into());
            media_records_fs::write(root, &old, &record, None).unwrap();
            crate::library_index_fs::SqliteRebuilder {
                database: repo.database.clone(),
                priority: Arc::default(),
            }
            .rebuild(root, &AtomicBool::new(false), &|_| {})
            .unwrap();
            let before = fs::read(media_records_fs::path(root, &old, false).unwrap()).unwrap();
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "date_correction_fs::relocation::crash_tests::relocation_crash_child",
                ])
                .env("PHOTOSUITE_MOVE_TEST_ROOT", root)
                .env("PHOTOSUITE_MOVE_TEST_STAGE", stage)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    panic!("crash fixture timed out");
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            };
            assert_eq!(status.code(), Some(73), "{stage}");
            repo.load(root).unwrap();
            let current = if stage == "Committed" {
                LibraryFile::new("1980/07/img_19800708_124845.mp4".into()).unwrap()
            } else {
                old.clone()
            };
            assert_eq!(
                fs::read(root.join(current.as_str())).unwrap(),
                b"crash-safe original bytes",
                "{stage}"
            );
            let saved = fs::read(media_records_fs::path(root, &current, false).unwrap()).unwrap();
            if stage != "Committed" {
                assert_eq!(saved, before);
                assert!(!root.join("1980/07/img_19800708_124845.mp4").exists());
            }
            let record = media_records_fs::parse(&saved).unwrap();
            assert_eq!(record.tags.unwrap(), ["urlaub".into()].into());
            repo.database
                .read(root, |db| {
                    let count: i64 = db.query_row(
                        "SELECT count(*) FROM media_tags WHERE media_path=?1 AND tag_name='urlaub'",
                        [current.as_str()],
                        |row| row.get(0),
                    )?;
                    assert_eq!(count, 1, "{stage}");
                    Ok(())
                })
                .unwrap();
            assert!(
                fs::read_dir(root.join("yaml"))
                    .unwrap()
                    .map(Result::unwrap)
                    .all(|entry| !entry.file_name().to_string_lossy().starts_with(PREFIX)),
                "journal left after {stage}"
            );
        }
    }
}

#[cfg(test)]
mod concurrency_tests {
    use super::*;
    use photosuite_application::transcription::queue::{Candidate, QueueStore};
    #[test]
    fn late_transcript_cannot_recreate_a_sidecar_at_the_old_path_after_transfer() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let repo = CaptureDates {
            database: Arc::new(Database),
            priority: Arc::default(),
        };
        let source = LibraryFile::new("broken/img_20111111_124844.mp4".into()).unwrap();
        fs::create_dir_all(root.join("broken")).unwrap();
        fs::write(root.join(source.as_str()), b"original bytes").unwrap();
        let store = Arc::new(crate::transcription_queue_fs::SqliteTranscriptionQueue::default());
        let version = store.version(root, &source).unwrap();
        let transcript = crate::domain::transcript::Transcript::new(
            version,
            crate::domain::transcript::TranscriptStatus::Complete,
            "tiny".into(),
            "de".into(),
            "late text".into(),
            vec![],
        )
        .unwrap();
        let date = CaptureTime::parse("1980:07:08 12:48:44", "test").unwrap();
        let target =
            photosuite_application::date_correction::relocation_target(&source, &date).unwrap();
        let worker = std::sync::Mutex::new(None);
        run(&repo, root, &source, &target, &date, None, &|stage| {
            if stage == Stage::Copied {
                let store = store.clone();
                let source = source.clone();
                let transcript = transcript.clone();
                let root = root.to_owned();
                let (send, receive) = std::sync::mpsc::channel();
                *worker.lock().unwrap() = Some(std::thread::spawn(move || {
                    send.send(()).unwrap();
                    store.write_transcript(
                        &root,
                        &Candidate {
                            source: source.clone(),
                            record: source,
                        },
                        &transcript,
                    )
                }));
                receive
                    .recv_timeout(std::time::Duration::from_secs(3))
                    .unwrap();
            }
            Ok(())
        })
        .unwrap();
        assert!(
            worker
                .into_inner()
                .unwrap()
                .unwrap()
                .join()
                .unwrap()
                .is_err()
        );
        assert!(!root.join(source.metadata_path().unwrap()).exists());
        assert!(root.join(target.metadata_path().unwrap()).exists());
    }
    #[test]
    fn failed_move_without_original_yaml_does_not_leave_an_invented_date_override() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let repo = CaptureDates {
            database: Arc::new(Database),
            priority: Arc::default(),
        };
        let source = LibraryFile::new("broken/img_20111111_124844.mp4".into()).unwrap();
        fs::create_dir_all(root.join("broken")).unwrap();
        fs::write(root.join(source.as_str()), b"original bytes").unwrap();
        let date = CaptureTime::parse("1980:07:08 12:48:44", "test").unwrap();
        let target =
            photosuite_application::date_correction::relocation_target(&source, &date).unwrap();
        assert!(
            run(
                &repo,
                root,
                &source,
                &target,
                &date,
                None,
                &|stage| if stage == Stage::Indexed {
                    Err("test failure".into())
                } else {
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(repo.load(root).unwrap().is_empty());
        assert!(!root.join(source.metadata_path().unwrap()).exists());
        assert_eq!(
            fs::read(root.join(source.as_str())).unwrap(),
            b"original bytes"
        );
    }
}
