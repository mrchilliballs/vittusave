// TODO: Do some sort of integrity check before loading saves

mod config;
mod utils;

use console::{Term, style};
use dialoguer::{Confirm, Input, Select};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap, error::Error, fs, iter, path::{Path, PathBuf}, sync::LazyLock
};
use strum::{IntoEnumIterator, VariantArray};
use strum_macros::{Display, EnumIter, VariantArray};

#[derive(
    Hash, Eq, PartialEq, Debug, Clone, Copy, VariantArray, Display, Serialize, Deserialize, EnumIter
)]
enum Game {
    #[strum(to_string = "My Summer Car")]
    MySummerCar,
    #[strum(to_string = "UNDERTALE")]
    Undertale,
}

// TOOD: Proper error handling
pub static HOME_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| dirs::home_dir().expect("no home directory found"));
static CONFIG_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    dirs::config_dir()
        .expect("no config directory found")
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
// TODO: Separate configs into separate files possibly
pub const CONFIG_FILENAME: &str = "vittusave";

// TODO: Support copying between multiple paths
static GAME_PATHS: LazyLock<HashMap<Game, Option<PathBuf>>> = LazyLock::new(|| {
    HashMap::from([
        #[cfg(target_os = "linux")]
        (
            Game::MySummerCar,
            Some([STEAM_LINUX_HOME.clone(), PathBuf::from(".local/share/Steam/steamapps/compatdata/516750/pfx/drive_c/users/steamuser/AppData/LocalLow/Amistech/My Summer Car")]
            .iter()
            .collect()),
        ),
        #[cfg(target_os = "windows")]
        (
            Game::MySummerCar,
            Some([HOME_DIR.to_path_buf(), PathBuf::from("AppData\\LocalLow\\Amistech\\My Summer Car")]
            .iter()
            .collect()),
        ),
        #[cfg(target_os = "linux")]
        (
            Game::Undertale,
            Some([STEAM_LINUX_HOME.clone(), PathBuf::from(".config/UNDERTALE")]
            .iter()
            .collect()),
        )
    ])
});
// TODO
pub static SAVE_SLOT_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    dirs::document_dir()
        .expect("no document directory found")
        .join("VittuSave")
});

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveSlot {
    label: String,
    path: PathBuf,
    loaded: bool,
}

impl SaveSlot {
    pub fn new(label: &str) -> Self {
        SaveSlot {
            label: label.to_string(),
            path: [SAVE_SLOT_PATH.clone(), PathBuf::from(label)]
                .iter()
                .collect(),
            loaded: false,
        }
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn loaded(&self) -> bool {
        self.loaded
    }
    pub fn set_label(&mut self, new_label: &str) {
        self.label = new_label.to_string();
        self.path.set_file_name(new_label);
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}

// TODO: game-specific settings
#[derive(Debug, Serialize, Deserialize)]
struct SaveSwapper {
    save_slots: HashMap<Game, Vec<SaveSlot>>,
}
impl Default for SaveSwapper {
    fn default() -> Self {
        Self {
            save_slots: Game::VARIANTS
            .iter()
            .map(|game| (*game, Vec::new()))
            .collect(),
        }
    }
}

impl Drop for SaveSwapper {
    fn drop(&mut self) {
        // TODO: What to do here?
        let _ = self.save();
    }
}

impl SaveSwapper {
    pub fn build() -> Result<Self, Box<dyn Error>> {
        let config = config::read_config(CONFIG_FILENAME)?;

        if let Some(config) = config {
            Ok(config)
        } else {
            let config = config.unwrap_or_default();
            config.save()?;
            Ok(config)
        }
    }
    fn save(&self) -> Result<(), Box<dyn Error>> {
        config::write_config(CONFIG_FILENAME, self)?;
        Ok(())
    }
    pub fn is_os_supported(&self, game: Game) -> bool {
        GAME_PATHS.contains_key(&game)
    }
    pub fn get(&self, game: Game) -> &Vec<SaveSlot> {
        assert!(self.save_slots.contains_key(&game));

        self.save_slots.get(&game).unwrap()
    }
    pub fn is_empty(&self, game: Game) -> bool {
        assert!(self.is_os_supported(game));
        assert!(self.save_slots.contains_key(&game));

        self.save_slots.get(&game).unwrap().is_empty()
    }
    pub fn create(&mut self, game: Game, label: &str) -> Result<usize, Box<dyn Error>> {
        assert!(self.is_os_supported(game));
        assert!(self.save_slots.contains_key(&game));

        let save_slot = SaveSlot::new(label);
        fs::create_dir_all(save_slot.path())?;
        let save_slots = self.save_slots.get_mut(&game).unwrap();
        save_slots.push(save_slot);

        let len = save_slots.len();

        self.save()?;
        Ok(len - 1)
    }
    // TODO: Import from folder
    pub fn import(&mut self, game: Game, label: &str) -> Result<(), Box<dyn Error>> {
        assert!(self.is_os_supported(game));
        assert!(self.save_slots.get(&game).unwrap().is_empty());

        let index_slot = self.create(game, label)?;
        let save_slot = &mut self.save_slots.get_mut(&game).unwrap()[index_slot];

        // TODO: Custom game paths and error handling
        utils::copy_dir_all(
            GAME_PATHS.get(&game).unwrap().as_deref().unwrap(),
            save_slot.path(),
        )?;
        // TODO: Use setter instead
        save_slot.loaded = true;

        self.save()?;
        Ok(())
    }
    pub fn rename(&mut self, game: Game, index: usize, new_label: &str) -> Result<(), Box<dyn Error>> {
        assert!(self.is_os_supported(game));
        assert!(self.save_slots.contains_key(&game));
        assert!(self.save_slots.get(&game).unwrap().len() > index);

        let save_slot  = &mut self.save_slots.get_mut(&game).unwrap()[index];
        fs::rename(save_slot.path(), save_slot.path().with_file_name(new_label))?;
        save_slot.set_label(new_label);

        self.save()?;
        Ok(())
    }
    // TODO: Don't forget confirmation on the UI side
    pub fn delete(&mut self, game: Game, index: usize) -> Result<(), Box<dyn Error>> {
        assert!(self.is_os_supported(game));
        assert!(self.save_slots.contains_key(&game));
        assert!(self.save_slots.get(&game).unwrap().len() > index);

        let save_slot = self.save_slots.get_mut(&game).unwrap().remove(index);
        if save_slot.loaded() {
            utils::remove_dir_contents(GAME_PATHS.get(&game).unwrap().as_deref().unwrap())?;
        }
        fs::remove_dir_all(save_slot.path())?;

        self.save()?;
        Ok(())
    }
    fn load(&mut self, game: Game, index: usize) -> Result<(), Box<dyn Error>> {
        assert!(self.is_os_supported(game));
        assert!(self.save_slots.contains_key(&game));
        assert!(!self.save_slots.get(&game).unwrap()[index].loaded());

        let save_slot = &mut self.save_slots.get_mut(&game).unwrap()[index];
        let real_save_path = GAME_PATHS.get(&game).unwrap().as_deref().unwrap();

        utils::remove_dir_contents(real_save_path)?;
        utils::copy_dir_all(save_slot.path(), real_save_path)?;
        save_slot.loaded = true;

        self.save()?;
        Ok(())
    }
    fn unload(&mut self, game: Game, index: usize) -> Result<(), Box<dyn Error>> {
        assert!(self.is_os_supported(game));
        assert!(self.save_slots.contains_key(&game));
        assert!(self.save_slots.get(&game).unwrap()[index].loaded());

        let save_slot = &mut self.save_slots.get_mut(&game).unwrap()[index];
        let real_save_path = GAME_PATHS.get(&game).unwrap().as_deref().unwrap();

        utils::copy_dir_all(real_save_path, save_slot.path())?;
        utils::remove_dir_contents(real_save_path)?;
        save_slot.loaded = false;

        self.save()?;
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let term = Term::stdout();

    let mut save_swapper: SaveSwapper = SaveSwapper::build()?;

    loop {
        utils::clear_screen(&term, None, None)?;
        let items: Vec<String> = Game::VARIANTS
            .iter()
            .map(|game| {
                if save_swapper.is_os_supported(*game) {
                    game.to_string()
                } else {
                    style(game).red().to_string()
                }
            })
            .chain([String::from("Settings")])
            .collect();
        let Some(selection) = Select::new()
            .with_prompt("Menu")
            .items(&items)
            .default(0)
            .interact_on_opt(&term)?
        else {
            break;
        };
        if items[selection] == "Settings" {
            utils::clear_screen(&term, None, None)?;
            let items = ["Dummy 1", "Dummy 2"];
            let Some(_selection) = Select::new()
                .with_prompt("Settings")
                .items(&items)
                .default(0)
                .interact_on_opt(&term)?
            else {
                continue;
            };
            // Go to setting page...
        }
        let game = Game::VARIANTS[selection];
        if !save_swapper.is_os_supported(game) {
            continue;
        }
        // let save_slots = save_swapper.get(game);

        loop {
            utils::clear_screen(&term, Some(&game.to_string()), None)?;
            if save_swapper.is_empty(game) {
                let confirmation = Confirm::new()
                    .with_prompt("No saves found. Register the current one?")
                    .interact_on(&term)?;
                if confirmation {
                    // TODO: Deduplicate code
                    let label: String = Input::new()
                        .with_prompt("Enter save label")
                        .interact_text_on(&term)?;
                    save_swapper.import(game, &label)?;
                } else {
                    break;
                }
                continue;
            }
            let items: Vec<String> = save_swapper
                .get(game)
                .iter()
                .map(|slot| {
                    let loaded_str = if slot.loaded() { "X" } else { " " };
                    String::from("[") + loaded_str + "] " + slot.label()
                })
                .chain([String::from("New")])
                .collect();

            let Some(selection) = Select::new()
                .with_prompt("Select a save")
                .items(&items)
                .default(0)
                .interact_on_opt(&term)?
            else {
                break;
            };
            if items[selection] == "New" {
                utils::clear_screen(&term, Some(&game.to_string()), None)?;
                // TODO: Deduplicate code
                let label: String = Input::new()
                    .with_prompt("Enter save label")
                    .interact_text_on(&term)?;
                save_swapper.create(game, &label)?;
                continue;
            }
            let save_slot_index = selection;

            utils::clear_screen(
                &term,
                Some(&game.to_string()),
                Some(save_swapper.get(game)[save_slot_index].label()),
            )?;
            let mut items: Vec<&'static str> = vec![];
            let save_slot_loaded = save_swapper.get(game)[save_slot_index].loaded();
            if save_slot_loaded {
                items.push("Unload");
            } else {
                items.push("Load");
            }
            items.push("Rename");
            items.push("Delete");
            let Some(selection) = Select::new()
                .with_prompt("Select an action")
                .items(&items)
                .default(0)
                .interact_on_opt(&term)?
            else {
                continue;
            };
            match items[selection] {
                "Load" => save_swapper.load(game, save_slot_index)?,
                "Unload" => save_swapper.unload(game, save_slot_index)?,
                "Rename" => {
                    utils::clear_screen(
                        &term,
                        Some(&game.to_string()),
                        Some(save_swapper.get(game)[save_slot_index].label()),
                    )?;
                    let new_label: String = Input::new()
                        .with_prompt("Enter new label")
                        .with_initial_text(save_swapper.get(game)[save_slot_index].label())
                        .interact_text_on(&term)?;
                    save_swapper.rename(game, save_slot_index, &new_label)?;
                },
                "Delete" => {
                    utils::clear_screen(
                        &term,
                        Some(&game.to_string()),
                        Some(save_swapper.get(game)[save_slot_index].label()),
                    )?;
                    let confirmation = Confirm::new()
                        .with_prompt(format!(
                            "Are you sure you want to {} delete \"{}\"?",
                            style("permanently").red(),
                            save_swapper.get(game)[save_slot_index].label()
                        ))
                        .interact_on(&term)?;
                    if confirmation {
                        save_swapper.delete(game, save_slot_index)?;
                    }
                }
                _ => unreachable!(),
            }
        }
    }
    Ok(())
}
