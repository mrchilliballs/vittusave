use std::{path::PathBuf, sync::LazyLock};

// TODO: move these to relevant structs when possible

// TOOD: Proper error handling
pub static HOME_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| dirs::home_dir().expect("no home directory found"));
pub static DATA_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    dirs::data_local_dir()
        .expect("no data directory found")
        .join("VittuSave")
});
pub const DATA_FILENAME: &str = "vittusave";

pub const PCGW_API: &str = "https://www.pcgamingwiki.com/w/api.php";

pub static SAVE_SLOT_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    dirs::document_dir()
        .expect("no document directory found")
        .join("VittuSave")
});
