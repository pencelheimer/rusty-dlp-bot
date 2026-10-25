use std::{
    collections::BTreeMap,
    env, fs,
    path::PathBuf,
    sync::{PoisonError, RwLock},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::media::MediaKind;

fn default_cookies_path() -> PathBuf {
    PathBuf::from("./cookies.txt")
}

/// Telegram's upload limit for bots on the cloud Bot API.
fn default_max_file_size_mb() -> u64 {
    50
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub bot_tag: String,

    #[serde(default = "default_cookies_path")]
    pub cookies_file: PathBuf,

    #[serde(default = "default_max_file_size_mb")]
    pub max_file_size_mb: u64,

    #[serde(default)]
    admins: RwLock<Vec<String>>,

    #[serde(default)]
    sources: RwLock<BTreeMap<String, MediaKind>>,
}

impl Config {
    pub fn get_config_path() -> PathBuf {
        env::var("RUSTY_DLP_CONFIG_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("config.toml"))
    }

    pub fn new() -> Result<Self> {
        let path = Self::get_config_path();

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("failed to parse config file: {}", path.display()))?;

        Ok(config)
    }

    pub fn max_file_size(&self) -> u64 {
        self.max_file_size_mb * 1_000_000
    }

    pub fn kind_for(&self, url: &Url) -> Option<MediaKind> {
        let host = url.host_str()?;

        self.sources
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(host)
            .copied()
    }

    pub fn is_admin(&self, username: Option<&str>) -> bool {
        let Some(username) = username else {
            return false;
        };

        self.admins
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|admin| admin.eq_ignore_ascii_case(username))
    }

    pub fn admins(&self) -> Vec<String> {
        self.admins
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Admin handles for error messages, e.g. `@a, @b`.
    pub fn admin_contacts(&self) -> String {
        self.admins().join(", ")
    }

    /// Adds an admin (stored as `@username`). Returns `false` if already present.
    pub fn add_admin(&self, username: &str) -> Result<bool> {
        let handle = format!("@{}", username.trim_start_matches('@'));

        Self::modify(&self.admins, self, |admins| {
            if admins.iter().any(|a| a.eq_ignore_ascii_case(&handle)) {
                return false;
            }
            admins.push(handle);
            true
        })
    }

    /// Removes an admin. Returns `false` if not present. Refuses to remove the last admin.
    pub fn remove_admin(&self, username: &str) -> Result<bool> {
        let name = username.trim_start_matches('@');

        Self::modify(&self.admins, self, |admins| {
            let Some(pos) = admins
                .iter()
                .position(|a| a.trim_start_matches('@').eq_ignore_ascii_case(name))
            else {
                return Ok(false);
            };

            if admins.len() == 1 {
                return Err(anyhow::anyhow!("can't remove the last admin"));
            }

            admins.remove(pos);
            Ok(true)
        })?
    }

    pub fn sources(&self) -> BTreeMap<String, MediaKind> {
        self.sources
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Adds (or replaces) a source and persists the config. Rolls back if saving fails.
    pub fn add_source(&self, host: String, kind: MediaKind) -> Result<Option<MediaKind>> {
        Self::modify(&self.sources, self, |sources| {
            sources.insert(host, kind)
        })
    }

    /// Removes a source and persists the config. Rolls back if saving fails.
    pub fn remove_source(&self, host: &str) -> Result<Option<MediaKind>> {
        Self::modify(&self.sources, self, |sources| sources.remove(host))
    }

    /// Mutates one field under its write lock, then persists the whole config.
    /// The field is restored if saving fails.
    fn modify<V: Clone, T>(
        field: &RwLock<V>,
        config: &Config,
        f: impl FnOnce(&mut V) -> T,
    ) -> Result<T> {
        let mut guard = field.write().unwrap_or_else(PoisonError::into_inner);
        let backup = guard.clone();

        let result = f(&mut guard);

        // Serializing takes the read locks, so release the write lock first.
        drop(guard);

        if let Err(e) = config.save() {
            *field.write().unwrap_or_else(PoisonError::into_inner) = backup;
            return Err(e);
        }

        Ok(result)
    }

    fn save(&self) -> Result<()> {
        let path = Self::get_config_path();

        let config = toml::to_string_pretty(self).context("failed to Serialize config")?;

        fs::write(&path, config)
            .with_context(|| format!("failed to write config file: {}", path.display()))?;

        Ok(())
    }
}
