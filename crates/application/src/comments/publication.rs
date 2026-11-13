//! Publish one comment: validate references, choose a collision-free name and copy through a port.
use super::*;
pub fn publish_comment(
    store: &dyn CommentMediaStore,
    target: &PhotoTarget,
    take: &CapturedTake,
) -> Result<Comment, String> {
    let recorded = crate::domain::comments::RecordedAt::parse(&take.recorded_at)?;
    // Validate all reference values before any adapter operation; read metadata only
    // after the adapter has checked regular files and containment.
    comment_candidate(target, take, &recorded, target.time.clone(), 0)?;
    store.validate(target, take)?;
    let photo_time = target
        .time
        .clone()
        .or_else(|| store.photo_time(target).ok());
    for suffix in 0..10000 {
        let (path, comment) =
            comment_candidate(target, take, &recorded, photo_time.clone(), suffix)?;
        if store.exists(&path)? {
            continue;
        }
        store.copy_verified(&take.path, &path)?;
        return Ok(comment);
    }
    Err("Zu viele gleichzeitige Aufnahmen mit demselben Zeitstempel".into())
}
fn comment_candidate(
    target: &PhotoTarget,
    take: &CapturedTake,
    recorded: &crate::domain::comments::RecordedAt,
    photo_time: Option<CaptureTime>,
    suffix: usize,
) -> Result<(PathBuf, Comment), String> {
    let path = recorded.target(&target.root, suffix);
    let relative = path
        .strip_prefix(&target.root)
        .map_err(|e| e.to_string())?
        .to_str()
        .ok_or("Ungültiger Audiopfad")?
        .to_owned();
    let comment = Comment::new(
        take.id.clone(),
        LibraryFile::new(relative)?,
        target.file.clone(),
        recorded.clone(),
        photo_time,
    )?;
    Ok((path, comment))
}
