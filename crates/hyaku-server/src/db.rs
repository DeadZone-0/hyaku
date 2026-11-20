use hyaku_protocol::{FloorCarving, HallOfFame, Position, Tile};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FloorProgress {
    pub shrine_shards: u32,
    pub stairs_unlocked: bool,
    pub unlocked_gates: HashSet<u8>,
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SavedFloor {
    pub progress: FloorProgress,
    pub structures: Vec<(Position, Tile)>,
    pub campfires_by_builder: BTreeMap<String, Position>,
    pub carvings: Vec<SavedCarving>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedCarving {
    pub carving: FloorCarving,
    #[serde(default)]
    pub voters: BTreeSet<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedLocation {
    pub floor: u32,
    pub pos: Position,
    #[serde(default)]
    pub temper: u8,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Save {
    pub world_seed: Option<u64>,
    pub highest_unlocked_floor: u32,
    pub hall_of_fame: HallOfFame,
    pub tower_shards_bank: u32,
    pub floors: BTreeMap<u32, SavedFloor>,
    pub last_locations: BTreeMap<String, SavedLocation>,
}

impl Default for Save {
    fn default() -> Self {
        Self {
            world_seed: None,
            highest_unlocked_floor: 1,
            hall_of_fame: BTreeMap::new(),
            tower_shards_bank: 0,
            floors: BTreeMap::new(),
            last_locations: BTreeMap::new(),
        }
    }
}

pub struct Database {
    path: Option<PathBuf>,
    saved: Save,
}

impl Database {
    pub fn load(path: PathBuf, seed: u64) -> io::Result<Self> {
        let mut saved: Save = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Save::default(),
            Err(error) => return Err(error),
        };
        if !(1..=100).contains(&saved.highest_unlocked_floor)
            || saved.hall_of_fame.keys().any(|floor| *floor < 1 || *floor > saved.highest_unlocked_floor)
            || saved.floors.keys().any(|floor| !(1..=100).contains(floor))
        {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid floor in save"));
        }
        saved.world_seed.get_or_insert(seed);
        Ok(Self { path: Some(path), saved })
    }

    pub fn memory(seed: u64) -> Self {
        Self { path: None, saved: Save { world_seed: Some(seed), ..Save::default() } }
    }

    pub fn saved(&self) -> &Save {
        &self.saved
    }

    pub fn update(&mut self, edit: impl FnOnce(&mut Save)) -> io::Result<()> {
        let mut next = self.saved.clone();
        edit(&mut next);
        if next != self.saved {
            if let Some(path) = &self.path {
                write_save(path, &next)?;
            }
            self.saved = next;
        }
        Ok(())
    }
}

fn write_save(path: &Path, saved: &Save) -> io::Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(temporary.as_file_mut(), saved).map_err(io::Error::other)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;

    // The rename has already committed. A directory sync error can't roll it back.
    if let Err(error) = File::open(parent).and_then(|directory| directory.sync_all()) {
        eprintln!("[SAVE] Could not sync {}: {error}", parent.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_write_does_not_commit_and_invalid_save_stays_intact() {
        let directory = tempfile::tempdir().unwrap();
        let missing_parent = directory.path().join("missing").join("hyaku_save.json");
        let mut db = Database::load(missing_parent, 42).unwrap();
        assert!(db.update(|save| save.highest_unlocked_floor = 2).is_err());
        assert_eq!(db.saved().highest_unlocked_floor, 1);

        let path = directory.path().join("hyaku_save.json");
        fs::write(&path, b"{broken").unwrap();
        assert!(Database::load(path.clone(), 42).is_err());
        assert_eq!(fs::read(path).unwrap(), b"{broken");
    }
}
