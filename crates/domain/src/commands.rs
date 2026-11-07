//! Framework-independent media command catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    RotateLeft,
    RotateRight,
    ResetRotation,
    PlayVideo,
}
pub const ALL: [Command; 4] = [
    Command::RotateLeft,
    Command::RotateRight,
    Command::ResetRotation,
    Command::PlayVideo,
];
impl Command {
    pub fn apply(self, turns: u8) -> Option<u8> {
        match self {
            Self::RotateLeft => Some((turns % 4 + 3) % 4),
            Self::RotateRight => Some((turns % 4 + 1) % 4),
            Self::ResetRotation => Some(0),
            Self::PlayVideo => None,
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
