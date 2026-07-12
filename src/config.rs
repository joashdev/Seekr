use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;
#[cfg(test)]
use std::sync::{Mutex, OnceLock};

const APP_DIR_NAME: &str = "seekr";
const CONFIG_FILE_NAME: &str = "config.toml";
const DATABASE_FILE_NAME: &str = "seekr.db";
const DEFAULT_IGNORED_COMMANDS: [&str; 4] = ["ls", "cd", "pwd", "clear"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl ResolvedPaths {
    pub fn from_env() -> io::Result<Self> {
        Ok(Self {
            config_dir: resolve_config_dir()?,
            data_dir: resolve_data_dir()?,
        })
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join(CONFIG_FILE_NAME)
    }

    pub fn database_file(&self) -> PathBuf {
        self.data_dir.join(DATABASE_FILE_NAME)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeekrConfig {
    #[serde(default)]
    pub privacy: PrivacyConfig,
    #[serde(default)]
    pub shell: ShellConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyConfig {
    #[serde(default)]
    pub redaction_enabled: bool,
    #[serde(default = "default_ignored_commands")]
    pub ignore_commands: Vec<String>,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            redaction_enabled: false,
            ignore_commands: default_ignored_commands(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellConfig {
    #[serde(default)]
    pub preferred_shell: Option<String>,
}

pub fn load(paths: &ResolvedPaths) -> io::Result<SeekrConfig> {
    let config_file = paths.config_file();
    match fs::read_to_string(&config_file) {
        Ok(contents) => parse_config(&contents),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(SeekrConfig::default()),
        Err(error) => Err(error),
    }
}

pub fn save(paths: &ResolvedPaths, config: &SeekrConfig) -> io::Result<()> {
    fs::create_dir_all(&paths.config_dir)?;
    fs::write(paths.config_file(), to_toml(config)?)?;
    Ok(())
}

pub fn write_default_if_missing(paths: &ResolvedPaths) -> io::Result<()> {
    let config_file = paths.config_file();
    if config_file.exists() {
        return Ok(());
    }

    save(paths, &SeekrConfig::default())
}

fn parse_config(contents: &str) -> io::Result<SeekrConfig> {
    toml::from_str(contents).map_err(invalid_data)
}

fn to_toml(config: &SeekrConfig) -> io::Result<String> {
    toml::to_string_pretty(config).map_err(invalid_data)
}

fn invalid_data(error: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

fn resolve_config_dir() -> io::Result<PathBuf> {
    resolve_override("SEEKR_CONFIG_DIR").or_else(default_config_dir)
}

fn resolve_data_dir() -> io::Result<PathBuf> {
    resolve_override("SEEKR_DATA_DIR").or_else(default_data_dir)
}

fn resolve_override(name: &str) -> io::Result<PathBuf> {
    match env::var_os(name) {
        Some(value) if !value.is_empty() => Ok(PathBuf::from(value)),
        _ => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{name} is not set"),
        )),
    }
}

fn default_config_dir(_: io::Error) -> io::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        Ok(home_dir()?
            .join("Library/Application Support")
            .join(APP_DIR_NAME))
    }

    #[cfg(target_os = "windows")]
    {
        env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| missing_env("APPDATA"))
            .map(|path| path.join(APP_DIR_NAME))
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        Ok(xdg_dir("XDG_CONFIG_HOME", ".config")?.join(APP_DIR_NAME))
    }
}

fn default_data_dir(_: io::Error) -> io::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        Ok(home_dir()?
            .join("Library/Application Support")
            .join(APP_DIR_NAME))
    }

    #[cfg(target_os = "windows")]
    {
        env::var_os("LOCALAPPDATA")
            .or_else(|| env::var_os("APPDATA"))
            .map(PathBuf::from)
            .ok_or_else(|| missing_env("LOCALAPPDATA"))
            .map(|path| path.join(APP_DIR_NAME))
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        Ok(xdg_dir("XDG_DATA_HOME", ".local/share")?.join(APP_DIR_NAME))
    }
}

fn default_ignored_commands() -> Vec<String> {
    DEFAULT_IGNORED_COMMANDS
        .iter()
        .map(|command| (*command).to_string())
        .collect()
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn xdg_dir(name: &str, fallback: &str) -> io::Result<PathBuf> {
    if let Some(path) = env::var_os(name)
        .map(PathBuf::from)
        .filter(|value| !value.as_os_str().is_empty())
    {
        Ok(path)
    } else {
        Ok(home_dir()?.join(fallback))
    }
}

fn home_dir() -> io::Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|value| !value.as_os_str().is_empty())
        .ok_or_else(|| missing_env("HOME"))
}

fn missing_env(name: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("required environment variable {name} is not set"),
    )
}

#[cfg(test)]
pub(crate) fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
mod tests {
    use super::{load, save, write_default_if_missing, PrivacyConfig, ResolvedPaths, SeekrConfig};
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn missing_config_uses_defaults() {
        let root = temp_root("defaults");
        let paths = test_paths(&root);

        let config = load(&paths).expect("missing config should use defaults");

        assert_eq!(config, SeekrConfig::default());
    }

    #[test]
    fn environment_overrides_resolve_paths() {
        let _guard = super::env_lock().lock().expect("env lock");
        let root = temp_root("env");
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let original_config_dir = env::var_os("SEEKR_CONFIG_DIR");
        let original_data_dir = env::var_os("SEEKR_DATA_DIR");

        unsafe {
            env::set_var("SEEKR_CONFIG_DIR", &config_dir);
            env::set_var("SEEKR_DATA_DIR", &data_dir);
        }

        let paths = ResolvedPaths::from_env().expect("env overrides should resolve");

        assert_eq!(paths.config_dir, config_dir);
        assert_eq!(paths.data_dir, data_dir);
        assert_eq!(paths.config_file(), config_dir.join("config.toml"));
        assert_eq!(paths.database_file(), data_dir.join("seekr.db"));

        restore_env("SEEKR_CONFIG_DIR", original_config_dir);
        restore_env("SEEKR_DATA_DIR", original_data_dir);
    }

    #[test]
    fn parses_existing_config_file() {
        let root = temp_root("parse");
        let paths = test_paths(&root);
        fs::create_dir_all(&paths.config_dir).expect("config dir");
        fs::write(
            paths.config_file(),
            r#"[privacy]
redaction_enabled = true
ignore_commands = ["ls", "git status"]

[shell]
preferred_shell = "zsh"
"#,
        )
        .expect("config file");

        let config = load(&paths).expect("config should parse");

        assert_eq!(
            config,
            SeekrConfig {
                privacy: PrivacyConfig {
                    redaction_enabled: true,
                    ignore_commands: vec!["ls".to_string(), "git status".to_string()],
                },
                shell: super::ShellConfig {
                    preferred_shell: Some("zsh".to_string()),
                },
            }
        );
    }

    #[test]
    fn writes_default_config_when_missing() {
        let root = temp_root("write-default");
        let paths = test_paths(&root);

        write_default_if_missing(&paths).expect("should write default config");
        let config = load(&paths).expect("written config should load");

        assert_eq!(config, SeekrConfig::default());
    }

    #[test]
    fn saves_config_file() {
        let root = temp_root("save");
        let paths = test_paths(&root);
        let config = SeekrConfig {
            privacy: PrivacyConfig {
                redaction_enabled: true,
                ignore_commands: vec!["pwd".to_string()],
            },
            shell: super::ShellConfig {
                preferred_shell: Some("bash".to_string()),
            },
        };

        save(&paths, &config).expect("config should save");

        assert_eq!(load(&paths).expect("saved config should parse"), config);
    }

    fn test_paths(root: &Path) -> ResolvedPaths {
        ResolvedPaths {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
        }
    }

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = env::temp_dir().join(format!("seekr-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).expect("temp dir");
        path
    }

    fn restore_env(name: &str, value: Option<OsString>) {
        unsafe {
            match value {
                Some(value) => env::set_var(name, value),
                None => env::remove_var(name),
            }
        }
    }
}
