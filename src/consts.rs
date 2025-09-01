use std::{collections::HashMap, fs, path::PathBuf, sync::LazyLock};

use strum_macros::{VariantArray, Display, EnumIter};
use serde::{Serialize, Deserialize};

#[derive(
    Hash, Eq, PartialEq, Debug, Clone, Copy, VariantArray, Display, Serialize, Deserialize, EnumIter,
)]
pub enum Game {
    #[strum(to_string = "My Summer Car")]
    MySummerCar,
    #[strum(to_string = "UNDERTALE")]
    Undertale,
}

// TOOD: Proper error handling
pub static HOME_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| dirs::home_dir().expect("no home directory found"));
pub static DATA_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    dirs::data_local_dir()
        .expect("no data directory found")
        .join("VittuSave")
});
pub static STEAM_PKG_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| HOME_DIR.join(PathBuf::from(".steam")));
pub static STEAM_FLATPAK_HOME: LazyLock<PathBuf> =
    LazyLock::new(|| HOME_DIR.join(PathBuf::from(".var/app/com.valvesoftware.Steam")));
pub static STEAM_LINUX_HOME: LazyLock<PathBuf> = LazyLock::new(|| {
    if cfg!(target_os = "linux") {
        let steam_package_installed = fs::exists(STEAM_PKG_PATH.as_path()).unwrap_or_default();
        let steam_flatpak_installed = fs::exists(STEAM_FLATPAK_HOME.as_path()).unwrap_or_default();

        if steam_package_installed {
            HOME_DIR.to_path_buf()
        } else if steam_flatpak_installed {
            STEAM_FLATPAK_HOME.to_path_buf()
        } else {
            // Steam is not installed to the default path, for some reason
            HOME_DIR.to_path_buf()
        }
    } else {
        PathBuf::new()
    }
});
pub const DATA_FILENAME: &str = "vittusave";

// TODO: Support copying between multiple paths
pub static DEFAULT_GAME_PATHS: LazyLock<HashMap<Game, PathBuf>> = LazyLock::new(|| {
    HashMap::from([
        #[cfg(target_os = "linux")]
        (
            Game::MySummerCar,
            [STEAM_LINUX_HOME.clone(), PathBuf::from(".local/share/Steam/steamapps/compatdata/516750/pfx/drive_c/users/steamuser/AppData/LocalLow/Amistech/My Summer Car")]
            .iter()
            .collect(),
        ),
        #[cfg(target_os = "windows")]
        (
            Game::MySummerCar,
            [HOME_DIR.to_path_buf(), PathBuf::from("AppData\\LocalLow\\Amistech\\My Summer Car")]
            .iter()
            .collect(),
        ),
        #[cfg(target_os = "linux")]
        (
            Game::Undertale,
            [STEAM_LINUX_HOME.clone(), PathBuf::from(".config/UNDERTALE")]
            .iter()
            .collect(),
        )
        // TODO: Other OS paths
    ])
});
// FIXME
pub static SAVE_SLOT_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    dirs::document_dir()
        .expect("no document directory found")
        .join("VittuSave")
});