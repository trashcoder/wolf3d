use std::{
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};

const VARIANTS: [&str; 3] = ["WL6", "WL3", "WL1"];
const REQUIRED_STEMS: [&str; 8] = [
    "GAMEMAPS", "MAPHEAD", "VSWAP", "VGAHEAD", "VGAGRAPH", "VGADICT", "AUDIOHED", "AUDIOT",
];

#[derive(Debug, Clone)]
pub struct DataSet {
    pub directory: PathBuf,
    pub extension: &'static str,
}

#[derive(Debug)]
struct MissingData {
    searched: Vec<PathBuf>,
}

impl fmt::Display for MissingData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "required Wolfenstein 3D data files were not found. Provide a complete WL6, WL3, or WL1 data set obtained for your use with --data <directory>. Expected stems: {}. Searched: {}",
            REQUIRED_STEMS.join(", "),
            self.searched
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

impl Error for MissingData {}

pub fn locate(explicit: Option<&Path>) -> Result<DataSet, Box<dyn Error>> {
    let mut candidates = Vec::new();
    if let Some(path) = explicit {
        add_candidate(&mut candidates, path.to_path_buf());
    }
    if let Some(path) = std::env::var_os("WOLF3D_DATA") {
        add_candidate(&mut candidates, PathBuf::from(path));
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            add_candidate(&mut candidates, directory.join("data"));
            add_candidate(&mut candidates, directory.to_path_buf());
            // A conventional macOS .app keeps bundled resources below
            // Contents/Resources while its binary lives in Contents/MacOS.
            add_candidate(&mut candidates, directory.join("../Resources/data"));
            add_candidate(&mut candidates, directory.join("../Resources"));
        }
    }
    if let Some(directory) = user_data_directory() {
        add_candidate(&mut candidates, directory);
    }
    add_candidate(&mut candidates, PathBuf::from("."));
    add_candidate(&mut candidates, PathBuf::from("data"));

    for candidate in &candidates {
        for extension in VARIANTS {
            if REQUIRED_STEMS
                .iter()
                .all(|stem| find_data_file(candidate, &format!("{stem}.{extension}")).is_ok())
            {
                return Ok(DataSet {
                    directory: candidate
                        .canonicalize()
                        .unwrap_or_else(|_| candidate.clone()),
                    extension,
                });
            }
        }
    }
    Err(Box::new(MissingData {
        searched: candidates,
    }))
}

pub fn quick_save_path() -> PathBuf {
    let base = user_data_directory()
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|path| path.join(".wolf3d"))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("saves").join("quicksave.json")
}

pub const SAVE_SLOT_COUNT: usize = 10;

pub fn save_slot_path(slot: usize) -> PathBuf {
    let quick_save = quick_save_path();
    if slot == 0 {
        return quick_save;
    }
    quick_save
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("slot-{}.json", slot + 1))
}

pub fn high_score_path() -> PathBuf {
    let base = user_data_directory()
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|path| path.join(".wolf3d"))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("highscores.json")
}

pub fn settings_path() -> PathBuf {
    let base = user_config_directory()
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|path| path.join(".wolf3d"))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("settings.json")
}

/// The pre-XDG settings location, retained as a read fallback for existing users.
pub fn legacy_settings_path() -> PathBuf {
    let base = user_data_directory()
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|path| path.join(".wolf3d"))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("settings.json")
}

/// Finds an original game-data filename without assuming filesystem case rules.
/// This preserves the actual path returned by case-sensitive Linux/macOS filesystems.
pub fn find_data_file(directory: &Path, expected_name: &str) -> io::Result<PathBuf> {
    let exact = directory.join(expected_name);
    if exact.is_file() {
        return Ok(exact);
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case(expected_name))
        {
            return Ok(path);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("{expected_name} was not found in {}", directory.display()),
    ))
}

fn add_candidate(candidates: &mut Vec<PathBuf>, path: PathBuf) {
    if !candidates.contains(&path) {
        candidates.push(path);
    }
}

#[cfg(target_os = "linux")]
fn user_data_directory() -> Option<PathBuf> {
    absolute_environment_path("XDG_DATA_HOME")
        .or_else(|| absolute_environment_path("HOME").map(|home| home.join(".local/share")))
        .map(|base| base.join("wolf3d"))
}

#[cfg(target_os = "windows")]
fn user_data_directory() -> Option<PathBuf> {
    absolute_environment_path("APPDATA")
        .or_else(|| absolute_environment_path("LOCALAPPDATA"))
        .or_else(|| {
            absolute_environment_path("USERPROFILE").map(|profile| profile.join("AppData/Roaming"))
        })
        .map(|base| base.join("Wolfenstein 3D"))
}

#[cfg(target_os = "macos")]
fn user_data_directory() -> Option<PathBuf> {
    absolute_environment_path("HOME")
        .map(|home| home.join("Library/Application Support/Wolfenstein 3D"))
}

#[cfg(target_os = "linux")]
fn user_config_directory() -> Option<PathBuf> {
    absolute_environment_path("XDG_CONFIG_HOME")
        .or_else(|| absolute_environment_path("HOME").map(|home| home.join(".config")))
        .map(|base| base.join("wolf3d"))
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn user_config_directory() -> Option<PathBuf> {
    user_data_directory()
}

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
fn absolute_environment_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty() && path.is_absolute())
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn user_data_directory() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn finds_complete_data_set_in_explicit_directory() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wolf3d-data-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        for stem in REQUIRED_STEMS {
            fs::write(dir.join(format!("{stem}.WL6")), b"test").unwrap();
        }
        let found = locate(Some(&dir)).unwrap();
        assert_eq!(found.directory, dir.canonicalize().unwrap());
        assert_eq!(found.extension, "WL6");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn finds_lowercase_data_filenames_on_case_sensitive_filesystems() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wolf3d-lowercase-data-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        for stem in REQUIRED_STEMS {
            fs::write(
                dir.join(format!("{}.wl6", stem.to_ascii_lowercase())),
                b"test",
            )
            .unwrap();
        }
        let found = locate(Some(&dir)).unwrap();
        assert_eq!(found.extension, "WL6");
        assert!(
            find_data_file(&found.directory, "MAPHEAD.WL6")
                .unwrap()
                .is_file()
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_partial_data_set() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wolf3d-partial-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("GAMEMAPS.WL6"), b"test").unwrap();
        assert!(locate(Some(&dir)).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
