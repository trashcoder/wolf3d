use std::{
    error::Error,
    fmt, fs,
    io::Write,
    path::{Path, PathBuf},
};

const FORMAT_ID: &str = "wolf3d-rust-save";
const FORMAT_VERSION: u32 = 1;
const MAX_SAVE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(serde::Serialize, serde::Deserialize)]
struct SaveEnvelope<T> {
    format: String,
    version: u32,
    state: T,
}

#[derive(Debug)]
struct SaveError(String);

impl fmt::Display for SaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for SaveError {}

pub fn save<T: serde::Serialize>(path: &Path, state: &T) -> Result<(), Box<dyn Error>> {
    save_with_progress(path, state, || Ok(()))
}

pub fn save_with_progress<T: serde::Serialize>(
    path: &Path,
    state: &T,
    mut progress: impl FnMut() -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let envelope = SaveEnvelope {
        format: FORMAT_ID.to_owned(),
        version: FORMAT_VERSION,
        state,
    };
    let bytes = serde_json::to_vec(&envelope)?;
    if bytes.len() as u64 > MAX_SAVE_BYTES {
        return Err(Box::new(SaveError(
            "save file exceeds the 64 MiB limit".into(),
        )));
    }

    let temporary = temporary_path(path);
    progress()?;
    let mut file = fs::File::create(&temporary)?;
    progress()?;
    file.write_all(&bytes)?;
    progress()?;
    file.sync_all()?;
    drop(file);

    if path.exists() {
        let backup = backup_path(path);
        if backup.exists() {
            fs::remove_file(&backup)?;
        }
        fs::rename(path, &backup)?;
        if let Err(error) = fs::rename(&temporary, path) {
            let _ = fs::rename(&backup, path);
            let _ = fs::remove_file(&temporary);
            return Err(Box::new(error));
        }
        fs::remove_file(backup)?;
    } else if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(Box::new(error));
    }
    Ok(())
}

pub fn load<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Box<dyn Error>> {
    load_with_progress(path, || Ok(()))
}

pub fn load_with_progress<T: serde::de::DeserializeOwned>(
    path: &Path,
    mut progress: impl FnMut() -> Result<(), Box<dyn Error>>,
) -> Result<T, Box<dyn Error>> {
    match load_from(path, &mut progress) {
        Ok(state) => Ok(state),
        Err(primary_error) => {
            let backup = backup_path(path);
            if backup.exists() {
                load_from(&backup, &mut progress).or(Err(primary_error))
            } else {
                Err(primary_error)
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct SaveMetadata {
    #[serde(default)]
    save_name: String,
}

pub fn display_name(path: &Path) -> Option<String> {
    load::<SaveMetadata>(path)
        .ok()
        .map(|metadata| metadata.save_name)
}

fn load_from<T: serde::de::DeserializeOwned>(
    path: &Path,
    progress: &mut impl FnMut() -> Result<(), Box<dyn Error>>,
) -> Result<T, Box<dyn Error>> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_SAVE_BYTES {
        return Err(Box::new(SaveError(
            "save file exceeds the 64 MiB limit".into(),
        )));
    }
    progress()?;
    let bytes = fs::read(path)?;
    let envelope: SaveEnvelope<T> = serde_json::from_slice(&bytes)?;
    if envelope.format != FORMAT_ID {
        return Err(Box::new(SaveError(
            "file is not a Wolfenstein 3D Rust save".into(),
        )));
    }
    if envelope.version != FORMAT_VERSION {
        return Err(Box::new(SaveError(format!(
            "unsupported save version {}; this version supports {}",
            envelope.version, FORMAT_VERSION
        ))));
    }
    Ok(envelope.state)
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("save-backup")
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".{}.tmp", std::process::id()));
    PathBuf::from(name)
}
