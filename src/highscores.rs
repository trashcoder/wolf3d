use std::{
    error::Error,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub const SCORE_COUNT: usize = 7;
pub const MAX_NAME_BYTES: usize = 57;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub score: u32,
    pub completed: u16,
    pub episode: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ScoreFile {
    version: u32,
    entries: Vec<Entry>,
}

#[derive(Clone, Debug)]
pub struct Table {
    pub entries: Vec<Entry>,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            entries: [
                "id software-'92",
                "Adrian Carmack",
                "John Carmack",
                "Kevin Cloud",
                "Tom Hall",
                "John Romero",
                "Jay Wilbur",
            ]
            .into_iter()
            .map(|name| Entry {
                name: name.to_owned(),
                score: 10_000,
                completed: 1,
                episode: 0,
            })
            .collect(),
        }
    }
}

impl Table {
    pub fn load(path: &Path) -> Self {
        for candidate in [path.to_path_buf(), backup_path(path)] {
            let Ok(bytes) = fs::read(candidate) else {
                continue;
            };
            let Ok(file) = serde_json::from_slice::<ScoreFile>(&bytes) else {
                continue;
            };
            if file.version == 1 && file.entries.len() == SCORE_COUNT {
                let mut entries: Vec<_> = file.entries.into_iter().map(sanitize_entry).collect();
                // Early Rust previews persisted the global map number in
                // `completed`. DOS stores an episode-local floor instead.
                entries.sort_by(|left, right| {
                    right
                        .score
                        .cmp(&left.score)
                        .then_with(|| right.completed.cmp(&left.completed))
                });
                return Self { entries };
            }
        }
        Self::default()
    }

    /// Inserts using the DOS ordering: score first, then completed floor.
    /// Returns the new rank when the score qualifies.
    pub fn insert(&mut self, score: u32, completed: u16, episode: u16) -> Option<usize> {
        let rank = self.entries.iter().position(|entry| {
            score > entry.score || (score == entry.score && completed > entry.completed)
        })?;
        self.entries.insert(
            rank,
            Entry {
                name: String::new(),
                score,
                completed,
                episode,
            },
        );
        self.entries.truncate(SCORE_COUNT);
        Some(rank)
    }

    pub fn set_name(&mut self, rank: usize, name: &str) {
        if let Some(entry) = self.entries.get_mut(rank) {
            entry.name = sanitize_name(name);
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&ScoreFile {
            version: 1,
            entries: self.entries.clone(),
        })?;
        let temporary = temporary_path(path);
        let mut file = File::create(&temporary)?;
        file.write_all(&bytes)?;
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
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("highscore-backup")
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".{}.tmp", std::process::id()));
    PathBuf::from(name)
}

fn sanitize_entry(mut entry: Entry) -> Entry {
    entry.name = sanitize_name(&entry.name);
    if entry.completed > 10 {
        let episode_offset = entry.episode.saturating_mul(10);
        if let Some(floor) = entry
            .completed
            .checked_sub(episode_offset)
            .filter(|floor| (1..=10).contains(floor))
        {
            entry.completed = floor;
        }
    }
    entry
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .filter(|character| character.is_ascii_graphic() || *character == ' ')
        .take(MAX_NAME_BYTES)
        .collect()
}
