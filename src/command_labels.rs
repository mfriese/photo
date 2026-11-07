use crate::commands::Command;
pub trait CommandLabel {
    fn title(self) -> &'static str;
}
impl CommandLabel for Command {
    fn title(self) -> &'static str {
        match self {
            Self::RotateLeft => "Links drehen",
            Self::RotateRight => "Rechts drehen",
            Self::ResetRotation => "Zur EXIF-Ausrichtung",
            Self::PlayVideo => "Video abspielen",
        }
    }
}
