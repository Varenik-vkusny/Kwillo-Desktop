use std::path::PathBuf;
use uuid::Uuid;

pub fn temp_dir() -> PathBuf {
    std::env::temp_dir().join("audio_analyzer")
}

pub fn ensure_temp_dir() -> std::io::Result<()> {
    std::fs::create_dir_all(temp_dir())
}

pub fn new_recording_path() -> PathBuf {
    temp_dir().join(format!("{}.mp3", Uuid::new_v4()))
}

pub fn delete_recording(path: &PathBuf) -> std::io::Result<()> {
    std::fs::remove_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temp_dir_ends_with_audio_analyzer() {
        let dir = temp_dir();
        assert_eq!(dir.file_name().unwrap(), "audio_analyzer");
    }

    #[test]
    fn test_new_recording_path_has_mp3_extension() {
        let path = new_recording_path();
        assert_eq!(path.extension().unwrap(), "mp3");
    }

    #[test]
    fn test_new_recording_paths_are_unique() {
        let p1 = new_recording_path();
        let p2 = new_recording_path();
        assert_ne!(p1, p2);
    }

    #[test]
    fn test_recording_path_is_inside_temp_dir() {
        let path = new_recording_path();
        assert!(path.starts_with(temp_dir()));
    }

    #[test]
    fn test_ensure_temp_dir_creates_directory() {
        ensure_temp_dir().unwrap();
        assert!(temp_dir().exists());
    }
}
