use std::path::{Path, PathBuf};

use crate::error::Error;

pub struct Config {
    pub postcode: Option<String>,
    pub dhl_api_key: Option<String>,
    pub seventeen_track_api_key: Option<String>,
    pub auto_cleanup_days: u32,
    pub cache_minutes: u32,
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pakket")
        .join("config.toml")
}

pub fn shipments_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pakket")
        .join("shipments.json")
}

impl Config {
    pub fn load(profile: Option<&str>) -> Result<Self, Error> {
        let api_key_env = std::env::var("PAKKET_API_KEY").ok();
        let dhl_key_env = std::env::var("PAKKET_DHL_API_KEY").ok();
        Self::load_from_with_env(
            &config_path(),
            profile,
            api_key_env.as_deref(),
            dhl_key_env.as_deref(),
        )
    }

    pub fn load_from(path: &Path, profile: Option<&str>) -> Result<Self, Error> {
        Self::load_from_with_env(path, profile, None, None)
    }

    pub fn load_from_with_env(
        path: &Path,
        profile: Option<&str>,
        env_api_key: Option<&str>,
        env_dhl_api_key: Option<&str>,
    ) -> Result<Self, Error> {
        let content = std::fs::read_to_string(path)
            .map_err(|_| Error::Config(format!("cannot read config file: {}", path.display())))?;

        let table: toml::Table = content
            .parse()
            .map_err(|e| Error::Config(format!("invalid TOML: {e}")))?;

        let profile_name = profile.unwrap_or("default");
        let section = table
            .get(profile_name)
            .and_then(|v| v.as_table())
            .ok_or_else(|| Error::Config(format!("profile '{profile_name}' not found")))?;

        let postcode = section
            .get("postcode")
            .and_then(|v| v.as_str())
            .map(String::from);

        let dhl_api_key = env_dhl_api_key
            .map(String::from)
            .or_else(|| {
                section
                    .get("dhl_api_key")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            });

        // Support both `seventeen_track_api_key` and the legacy `api_key` field name.
        let file_seventeen_key = section
            .get("seventeen_track_api_key")
            .or_else(|| section.get("api_key"))
            .and_then(|v| v.as_str())
            .map(String::from);

        let seventeen_track_api_key = env_api_key
            .map(String::from)
            .or(file_seventeen_key);

        let auto_cleanup_days = section
            .get("auto_cleanup_days")
            .and_then(|v| v.as_integer())
            .unwrap_or(7) as u32;

        let cache_minutes = section
            .get("cache_minutes")
            .and_then(|v| v.as_integer())
            .unwrap_or(30) as u32;

        Ok(Self {
            postcode,
            dhl_api_key,
            seventeen_track_api_key,
            auto_cleanup_days,
            cache_minutes,
        })
    }

    /// Save all config fields to the given path under the given profile section.
    /// Fields set to `None` are omitted from the file.
    pub fn save_config(
        path: &Path,
        profile: &str,
        postcode: Option<&str>,
        dhl_api_key: Option<&str>,
        seventeen_track_api_key: Option<&str>,
    ) -> Result<(), Error> {
        let mut table: toml::Table = if path.exists() {
            let content = std::fs::read_to_string(path)
                .map_err(|e| Error::Config(format!("cannot read config: {e}")))?;
            content
                .parse()
                .map_err(|e| Error::Config(format!("invalid TOML: {e}")))?
        } else {
            toml::Table::new()
        };

        let section = table
            .entry(profile.to_string())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .ok_or_else(|| Error::Config("profile section is not a table".to_string()))?;

        if let Some(pc) = postcode {
            section.insert("postcode".to_string(), toml::Value::String(pc.to_string()));
        }
        if let Some(key) = dhl_api_key {
            section.insert(
                "dhl_api_key".to_string(),
                toml::Value::String(key.to_string()),
            );
        }
        if let Some(key) = seventeen_track_api_key {
            section.insert(
                "seventeen_track_api_key".to_string(),
                toml::Value::String(key.to_string()),
            );
        }

        let content = toml::to_string_pretty(&table)
            .map_err(|e| Error::Config(format!("failed to serialize: {e}")))?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Config(format!("cannot create config dir: {e}")))?;
        }

        std::fs::write(path, content)
            .map_err(|e| Error::Config(format!("cannot write config: {e}")))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_default_profile_with_all_keys() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[default]
postcode = "1234AB"
dhl_api_key = "dhl-key"
seventeen_track_api_key = "17-key"
auto_cleanup_days = 7
cache_minutes = 30
"#,
        )
        .unwrap();

        let config = Config::load_from(&path, None).unwrap();
        assert_eq!(config.postcode.as_deref(), Some("1234AB"));
        assert_eq!(config.dhl_api_key.as_deref(), Some("dhl-key"));
        assert_eq!(config.seventeen_track_api_key.as_deref(), Some("17-key"));
        assert_eq!(config.auto_cleanup_days, 7);
        assert_eq!(config.cache_minutes, 30);
    }

    #[test]
    fn load_with_legacy_api_key_field() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[default]\napi_key = \"legacy-key\"\n").unwrap();

        let config = Config::load_from(&path, None).unwrap();
        assert_eq!(config.seventeen_track_api_key.as_deref(), Some("legacy-key"));
    }

    #[test]
    fn load_with_no_api_keys_succeeds() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[default]\npostcode = \"9999ZZ\"\n").unwrap();

        let config = Config::load_from(&path, None).unwrap();
        assert_eq!(config.postcode.as_deref(), Some("9999ZZ"));
        assert!(config.dhl_api_key.is_none());
        assert!(config.seventeen_track_api_key.is_none());
    }

    #[test]
    fn load_named_profile() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[default]
dhl_api_key = "default-dhl"

[work]
dhl_api_key = "work-dhl"
"#,
        )
        .unwrap();

        let config = Config::load_from(&path, Some("work")).unwrap();
        assert_eq!(config.dhl_api_key.as_deref(), Some("work-dhl"));
    }

    #[test]
    fn missing_profile_returns_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[default]\npostcode = \"1234AB\"\n").unwrap();

        let result = Config::load_from(&path, Some("nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn missing_file_returns_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.toml");
        let result = Config::load_from(&path, None);
        assert!(result.is_err());
    }

    #[test]
    fn defaults_for_optional_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[default]\n").unwrap();

        let config = Config::load_from(&path, None).unwrap();
        assert_eq!(config.auto_cleanup_days, 7);
        assert_eq!(config.cache_minutes, 30);
    }

    #[test]
    fn config_path_returns_path() {
        let path = config_path();
        assert!(path.to_string_lossy().contains("pakket"));
        assert!(path.to_string_lossy().ends_with("config.toml"));
    }

    #[test]
    fn env_api_key_overrides_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[default]\nseventeen_track_api_key = \"file-key\"\n").unwrap();

        let config = Config::load_from_with_env(&path, None, Some("env-key"), None);
        assert_eq!(
            config.unwrap().seventeen_track_api_key.as_deref(),
            Some("env-key")
        );
    }

    #[test]
    fn env_dhl_key_overrides_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[default]\ndhl_api_key = \"file-dhl\"\n").unwrap();

        let config = Config::load_from_with_env(&path, None, None, Some("env-dhl"));
        assert_eq!(config.unwrap().dhl_api_key.as_deref(), Some("env-dhl"));
    }

    #[test]
    fn save_config_creates_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        Config::save_config(&path, "default", Some("1234AB"), Some("dhl-key"), None).unwrap();

        let config = Config::load_from(&path, None).unwrap();
        assert_eq!(config.postcode.as_deref(), Some("1234AB"));
        assert_eq!(config.dhl_api_key.as_deref(), Some("dhl-key"));
        assert!(config.seventeen_track_api_key.is_none());
    }

    #[test]
    fn save_config_updates_existing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[default]\npostcode = \"0000AA\"\n").unwrap();

        Config::save_config(&path, "default", Some("9999ZZ"), None, None).unwrap();

        let config = Config::load_from(&path, None).unwrap();
        assert_eq!(config.postcode.as_deref(), Some("9999ZZ"));
    }
}
