use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    rc::Rc,
    sync::LazyLock,
};

use crate::{
    SaveMetadata, SaveSwapper, SaveSwapperConfig,
    config::{read_config, write_config},
};

#[derive(Debug)]
pub struct MySummerCarSaveSwapper {
    // TODO: Ask user to save or add config pane
    config: SaveSwapperConfig,
}

const MSC_LINUX: &str = ".local/share/Steam/steamapps/compatdata/516750/pfx/drive_c/users/steamuser/AppData/LocalLow/Amistech/My Summer Car";
const MSC_FLATPAK_STEAM: &str = ".var/app/com.valvesoftware.Steam";
static MSC_DEFAULT_DIR: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
    let mut dir = dirs::home_dir().unwrap();
    if cfg!(target_os = "linux") {
        if fs::exists(dir.join(MSC_FLATPAK_STEAM)).is_ok_and(|result| result) {
            dir.push(MSC_FLATPAK_STEAM);
        }
        dir.push(MSC_LINUX);
        // TODO: Real Linux path
        Some(dir)
    } else if cfg!(target_os = "windows") {
        // TODO: Real Windows path
        dir.push("AppData\\LocalLow\\Amistech\\My Summer Car");
        Some(dir)
    } else {
        None
    }
});

impl MySummerCarSaveSwapper {
    const DISPLAY_NAME: &str = "My Summer Car";
    const CONFIG_FILENAME: &str = "My_Summer_Car";

    pub fn build() -> Result<Self, Box<dyn Error>> {
        let config = read_config(Self::CONFIG_FILENAME)?
            .unwrap_or(SaveSwapperConfig::new(MSC_DEFAULT_DIR.clone()));

        Ok(Self { config })
    }
}

impl Deref for MySummerCarSaveSwapper {
    type Target = BTreeMap<Rc<Path>, SaveMetadata>;

    fn deref(&self) -> &Self::Target {
        &self.config.saves
    }
}
impl DerefMut for MySummerCarSaveSwapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.config.saves
    }
}
impl Drop for MySummerCarSaveSwapper {
    fn drop(&mut self) {
        self.save().expect(&format!(
            "failed to save \"{}\"'s configuration at drop",
            self.display_name()
        ));
    }
}

impl SaveSwapper for MySummerCarSaveSwapper {
    fn display_name(&self) -> &'static str {
        Self::DISPLAY_NAME
    }
    fn config_filename(&self) -> &'static str {
        Self::CONFIG_FILENAME
    }
    fn save(&self) -> Result<(), Box<dyn Error>> {
        write_config(Self::CONFIG_FILENAME, &self.config)?;
        Ok(())
    }
    fn default_dir(&self) -> Option<&Path> {
        MSC_DEFAULT_DIR.as_deref()
    }
    fn get_dir(&self) -> Option<&Path> {
        self.config.path.as_deref()
    }
    fn set_dir(&mut self, dir: PathBuf) {
        let _ = self.config.path.insert(dir);
    }
}
