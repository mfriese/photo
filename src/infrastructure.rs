//! Concrete outer adapters, assembled only in main.
mod collection;
mod images;
mod opener;
mod orientation;
pub use collection::FileSystemRepository;
pub use images::RasterDecoder;
pub use opener::SystemMediaOpener;
pub use orientation::ExifOrientationWriter;
