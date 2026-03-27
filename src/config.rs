use std::path::{Path, PathBuf};

use crate::error::Error;

pub struct Config {
    pub api_key: String,
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
        Self::load_from_with_env(&config_path(), profile, api_key_env.as_deref())
    }

    pub fn load_from(path: &Path, profile: Option<&str>) -> Result<Self, Error> {
        Self::load_from_with_env(path, profile, None)
    }

    pub fn load_from_with_env(
        path: &Path,
        profile: Option<&str>,
        env_api_key: Option<&str>,
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

        let file_api_key = section
            .get("api_key")
            .and_then(|v| v.as_str())
            .map(String::from);

        let api_key = env_api_key
            .map(String::from)
            .or(file_api_key)
            .ok_or_else(|| Error::Config("api_key not configured".to_string()))?;

        let auto_cleanup_days = section
            .get("auto_cleanup_days")
            .and_then(|v| v.as_integer())
            .unwrap_or(7) as u32;

        let cache_minutes = section
            .get("cache_minutes")
            .and_then(|v| v.as_integer())
            .unwrap_or(30) as u32;

        Ok(Self {
            api_key,
            auto_cleanup_days,
            cache_minutes,
        })
    }

    pub fn save_api_key(path: &Path, profile: &str, api_key: &str) -> Result<(), Error> {
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

        section.insert(
            "api_key".to_string(),
            toml::Value::String(api_key.to_string()),
        );

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
    fn load_default_profile() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[default]
api_key = "test-key"
backend = "17track"
auto_cleanup_days = 7
cache_minutes = 30
"#,
        )
        .unwrap();

        let config = Config::load_from(&path, None).unwrap();
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.auto_cleanup_days, 7);
        assert_eq!(config.cache_minutes, 30);
    }

    #[test]
    fn load_named_profile() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[default]
api_key = "default-key"

[work]
api_key = "work-key"
"#,
        )
        .unwrap();

        let config = Config::load_from(&path, Some("work")).unwrap();
        assert_eq!(config.api_key, "work-key");
    }

    #[test]
    fn missing_profile_returns_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[default]\napi_key = \"key\"\n").unwrap();

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
        std::fs::write(&path, "[default]\napi_key = \"key\"\n").unwrap();

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
        std::fs::write(&path, "[default]\napi_key = \"file-key\"\n").unwrap();

        let config = Config::load_from_with_env(&path, None, Some("env-key"));
        assert_eq!(config.unwrap().api_key, "env-key");
    }
}
