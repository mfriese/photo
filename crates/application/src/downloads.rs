//! Shared technical ports for global downloadable assets; no HTTP or filesystem dependency.
use std::sync::atomic::AtomicBool;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Loading,
    Checking,
    Downloading,
    Saving,
}
#[derive(Clone, Copy, Debug)]
pub struct Progress {
    pub stage: Stage,
    pub bytes: u64,
    pub total: u64,
}
/// Technical download/hash primitives, injected into the local store in the composition root.
pub trait AssetTransport: Send + Sync + 'static {
    fn download(
        &self,
        url: &str,
        path: &std::path::Path,
        expected: u64,
        cancel: &AtomicBool,
        progress: &(dyn Fn(Progress) + Sync),
    ) -> Result<(), String>;
    /// Variable-size HTTPS asset, bounded while streaming (no exact upstream size).
    fn download_bounded(
        &self,
        url: &str,
        path: &std::path::Path,
        limit: u64,
        cancel: &AtomicBool,
        progress: &(dyn Fn(Progress) + Sync),
    ) -> Result<(), String>;
    fn sha256(
        &self,
        path: &std::path::Path,
        cancel: &AtomicBool,
        progress: &(dyn Fn(Progress) + Sync),
    ) -> Result<String, String>;
}
