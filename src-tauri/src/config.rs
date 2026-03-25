use serde::{Deserialize, Serialize};
use tauri_plugin_store::StoreExt;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub hotkey: String,
    pub auto_mute_enabled: bool,
    pub language: String,
}

// Legacy config for migration
#[derive(Deserialize)]
struct OldConfig {
    hotkey: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey: "Cmd+Option+Space".to_string(),
            auto_mute_enabled: true,
            language: "en".to_string(),
        }
    }
}

pub fn init_store(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let store = app.store("config.json")?;

    // Load or create default
    if !store.has("config") {
        store.set(
            "config".to_string(),
            serde_json::to_value(AppConfig::default())?,
        );
        store.save()?;
    } else {
        // Migrate existing config if needed
        if let Some(config_val) = store.get("config") {
            // Try to parse as new config first
            if serde_json::from_value::<AppConfig>(config_val.clone()).is_err() {
                // Failed to parse - try old config format
                if let Ok(old_config) = serde_json::from_value::<OldConfig>(config_val) {
                    // Migrate old config to new format
                    let new_config = AppConfig {
                        hotkey: old_config.hotkey,
                        auto_mute_enabled: true, // Default to enabled for existing users
                        language: "en".to_string(), // Default for migrated users
                    };
                    store.set("config".to_string(), serde_json::to_value(new_config)?);
                    store.save()?;
                }
            }
        }
    }

    Ok(())
}
