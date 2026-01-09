use crate::xdg;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const FREQ_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Usage {
    pub freq: u32,
    /// Unix timestamp (seconds). 0 means unknown.
    pub last_used: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct FrequencyFile {
    version: u32,
    map: HashMap<String, Usage>,
}

#[derive(Debug, Default)]
pub struct FrequencyStore {
    map: HashMap<String, Usage>,
    dirty: bool,
    path: PathBuf,
}

impl FrequencyStore {
    pub fn load() -> Self {
        let path = frequency_path();

        let mut store = Self {
            map: HashMap::new(),
            dirty: false,
            path,
        };

        if let Ok(data) = fs::read(&store.path)
            && let Ok(file) = postcard::from_bytes::<FrequencyFile>(&data)
            && file.version == FREQ_VERSION
        {
            store.map = file.map;
            return store;
        }

        store
    }

    pub fn get(&self, id: &str) -> Usage {
        self.map.get(id).copied().unwrap_or_default()
    }

    pub fn increment(&mut self, id: &str) -> u32 {
        let now = unix_seconds_now();
        let v = self.map.entry(id.to_string()).or_default();
        v.freq = v.freq.saturating_add(1);
        v.last_used = now;
        self.dirty = true;
        v.freq
    }

    pub fn map(&self) -> &HashMap<String, Usage> {
        &self.map
    }

    pub fn flush(&mut self) {
        if !self.dirty {
            return;
        }

        let Some(dir) = self.path.parent() else {
            return;
        };
        if fs::create_dir_all(dir).is_err() {
            return;
        }

        let file = FrequencyFile {
            version: FREQ_VERSION,
            map: self.map.clone(),
        };

        let Ok(data) = postcard::to_stdvec(&file) else {
            return;
        };

        // Best-effort atomic-ish write.
        let tmp = self.path.with_extension("bin.tmp");
        if fs::write(&tmp, data).is_ok() {
            let _ = fs::rename(tmp, &self.path);
            self.dirty = false;
        }
    }
}

fn frequency_path() -> PathBuf {
    xdg::data_dir().join(format!("frequencies.v{FREQ_VERSION}.bin"))
}

pub fn unix_seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
