//! Shared technical codec for per-media tags and photo comments. No workflow decisions.
use crate::domain::tags::{LibraryFile, normalize_tag_name};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub version: u32,
    #[serde(rename = "file", alias = "photo")]
    pub photo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<BTreeSet<String>>,
    #[serde(default)]
    pub comments: BTreeMap<String, String>,
}
impl Record {
    pub fn new(file: &LibraryFile) -> Self {
        Self {
            version: 3,
            photo: file.as_str().into(),
            tags: None,
            comments: Default::default(),
        }
    }
}
pub(crate) fn parse(bytes: &[u8]) -> Result<Record, String> {
    let mut record: Record =
        serde_saphyr::from_str(std::str::from_utf8(bytes).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if ![2, 3].contains(&record.version) {
        return Err("Unbekannte Version der Medien-YAML".into());
    }
    let file = LibraryFile::new(record.photo.clone())?;
    file.metadata_path()?;
    if let Some(tags) = record.tags.take() {
        record.tags = Some(
            tags.iter()
                .map(|t| normalize_tag_name(t))
                .collect::<Result<_, _>>()?,
        );
    }
    let mut comments = crate::domain::comments::CommentCatalog::default();
    for (id, audio) in &record.comments {
        comments.insert(crate::domain::comments::Comment::new(
            id.clone(),
            LibraryFile::new(audio.clone())?,
            file.clone(),
        )?)?;
    }
    record.comments = comments
        .entries()
        .values()
        .map(|c| (c.id().to_owned(), c.audio().as_str().to_owned()))
        .collect();
    Ok(record)
}
pub(crate) fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    if !crate::metadata_fs::check_file(path)? {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("Medien-YAML überschreitet 1 MiB".into());
    }
    Ok(Some(bytes))
}
pub(crate) fn path(
    root: &Path,
    file: &LibraryFile,
    create: bool,
) -> Result<std::path::PathBuf, String> {
    crate::metadata_fs::library_path(root, &file.metadata_path()?, create)
}
pub(crate) fn write(
    root: &Path,
    file: &LibraryFile,
    record: &Record,
    expected: Option<&[u8]>,
) -> Result<(), String> {
    if record.photo != file.as_str() {
        return Err("Medien-YAML gehört zu einer anderen Datei".into());
    }
    let mut record = parse(
        &serde_saphyr::to_string(record)
            .map_err(|e| e.to_string())?
            .into_bytes(),
    )?;
    record.version = 3;
    let bytes = serde_saphyr::to_string(&record)
        .map_err(|e| e.to_string())?
        .into_bytes();
    if bytes.len() > 1024 * 1024 {
        return Err("Medien-YAML überschreitet 1 MiB".into());
    }
    crate::metadata_fs::atomic_write_if_current(&path(root, file, true)?, &bytes, expected)
}
pub(crate) fn visit(
    root: &Path,
    cancel: &AtomicBool,
    priority: &photosuite_application::work_priority::WorkPriority,
    mut visitor: impl FnMut(LibraryFile, Record, Vec<u8>) -> Result<(), String>,
) -> Result<(), String> {
    let marker = crate::metadata_fs::library_path(root, "yaml/.scan", false)?;
    let base = marker.parent().ok_or("YAML-Verzeichnis fehlt")?;
    if !base.try_exists().map_err(|e| e.to_string())? {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(base)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with("._"))
    {
        if cancel.load(Ordering::Relaxed) || !priority.wait_background(cancel) {
            return Err("Neuaufbau abgebrochen; Index unverändert".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.depth() == 1
            && entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.len() == 4 && n.bytes().all(|b| b.is_ascii_digit()))
            && !entry.file_type().is_dir()
        {
            return Err("Jahreszweig unter yaml ist kein reguläres Verzeichnis".into());
        }
        if entry.file_type().is_symlink() {
            return Err(format!(
                "Symbolischer Metadatenpfad: {}",
                entry.path().display()
            ));
        }
        if !entry.file_type().is_file() || entry.path().extension().is_none_or(|e| e != "yaml") {
            continue;
        }
        let Some(name) = entry.path().file_stem().and_then(|n| n.to_str()) else {
            continue;
        };
        if crate::domain::Media::from_path(name.into(), String::new()).is_none() {
            continue;
        }
        let bytes = read(entry.path())?.ok_or("Medien-YAML während Neuaufbau entfernt")?;
        let record = parse(&bytes)?;
        let file = LibraryFile::new(record.photo.clone())?;
        if path(root, &file, false)? != entry.path() {
            return Err(format!(
                "Medien-YAML liegt am falschen Ort: {}",
                entry.path().display()
            ));
        }
        visitor(file, record, bytes)?;
    }
    Ok(())
}
