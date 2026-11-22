//! Framework-independent media command catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    RotateLeft,
    RotateRight,
    ResetRotation,
    PlayVideo,
    CorrectDate,
}
pub const ALL: [Command; 5] = [
    Command::RotateLeft,
    Command::RotateRight,
    Command::ResetRotation,
    Command::PlayVideo,
    Command::CorrectDate,
];
impl Command {
    /// Stable persistence identity, independent of translated labels and GUI frameworks.
    pub const fn id(self) -> &'static str {
        match self {
            Self::RotateLeft => "rotate_left",
            Self::RotateRight => "rotate_right",
            Self::ResetRotation => "reset_rotation",
            Self::PlayVideo => "play_video",
            Self::CorrectDate => "correct_date",
        }
    }
    /// One availability rule for menus, buttons and shortcut dispatch.
    pub fn available_for(self, media: &crate::Media) -> bool {
        match self {
            Self::PlayVideo => media.is_video(),
            Self::CorrectDate => !media.is_audio(),
            Self::RotateLeft | Self::RotateRight | Self::ResetRotation => media.is_photo(),
        }
    }
    pub fn apply(self, turns: u8) -> Option<u8> {
        match self {
            Self::RotateLeft => Some((turns % 4 + 3) % 4),
            Self::RotateRight => Some((turns % 4 + 1) % 4),
            Self::ResetRotation => Some(0),
            Self::PlayVideo | Self::CorrectDate => None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotations_wrap() {
        assert_eq!(Command::RotateLeft.apply(0), Some(3));
        assert_eq!(Command::RotateRight.apply(3), Some(0));
        assert_eq!(Command::ResetRotation.apply(2), Some(0));
    }
}
