use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub type AppConfig = HashMap<String, serde_json::Value>;

fn get_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub fn get_config_path() -> Option<PathBuf> {
    let home = get_home_dir()?;
    let dockex_path = home.join(".dockex.json");
    if dockex_path.exists() {
        return Some(dockex_path);
    }
    // Backward compatibility with previous config files
    let shipx_path = home.join(".shipx.json");
    if shipx_path.exists() {
        return Some(shipx_path);
    }
    let legacy_path = home.join(".dockerized.json");
    if legacy_path.exists() {
        return Some(legacy_path);
    }
    Some(dockex_path)
}

pub fn load_config() -> AppConfig {
    if let Some(path) = get_config_path() {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                    return config;
                }
            }
        }
    }
    HashMap::new()
}

pub fn save_config(config: &AppConfig) -> Result<(), std::io::Error> {
    if let Some(home) = get_home_dir() {
        let dockex_path = home.join(".dockex.json");
        let json = serde_json::to_string_pretty(config)?;
        fs::write(dockex_path, json)?;
    }
    Ok(())
}
