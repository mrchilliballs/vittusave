// TODO: Do some sort of integrity check before loading saves
// TODO: Steam Cloud support (info UT favorites)
// TODO: Docs
// FIXME: Bug when game save files don't exist

mod utils;
mod consts;
mod game_data;

use console::{Term, style};
use dialoguer::{Confirm, Input, Select};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use crate::consts::{Game, DATA_FILENAME, DEFAULT_GAME_PATHS, SAVE_SLOT_PATH};
use strum::VariantArray;


#[derive(Debug, Serialize, Deserialize)]
pub struct SaveSlot {
    label: String,
    path: PathBuf,
}

impl SaveSlot {
    pub fn new(label: &str, game_name: &str) -> Self {
        SaveSlot {
            label: label.to_string(),
            path: [
                SAVE_SLOT_PATH.clone(),
                PathBuf::from(game_name),
                PathBuf::from(label),
            ]
            .iter()
            .collect(),
        }
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn set_label(&mut self, new_label: &str) {
        self.label = new_label.to_string();
        self.path.set_file_name(new_label);
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameData {
    pub save_slots: Vec<SaveSlot>,
    // TODO: Use pointer instead, maybe
    pub loaded_slot: Option<usize>,
    pub custom_game_path: Option<PathBuf>,
}
impl Default for GameData {
    fn default() -> Self {
        Self {
            save_slots: Vec::new(),
            loaded_slot: None,
            custom_game_path: None,
        }
    }
}

// TODO: game-specific settings
// TODO: Separate game save/slots data struct instead of multiple fields
#[derive(Debug, Serialize, Deserialize)]
struct SaveSwapper {
    game_data: HashMap<Game, GameData>,
}
impl Default for SaveSwapper {
    fn default() -> Self {
        Self {
            game_data: Game::VARIANTS
                .iter()
                .map(|game| (*game, GameData::default()))
                .collect(),
        }
    }
}

impl Drop for SaveSwapper {
    fn drop(&mut self) {
        // FIXME: What to do here?
        let _ = self.save();
    }
}

// TODO: Reorder functionality
impl SaveSwapper {
    pub fn build() -> Result<Self, Box<dyn Error>> {
        let save_swapper_data = utils::read_data(DATA_FILENAME)?;

        if let Some(save_swapper) = save_swapper_data {
            Ok(save_swapper)
        } else {
            let save_swapper = save_swapper_data.unwrap_or_default();
            save_swapper.save()?;
            Ok(save_swapper)
        }
    }
    fn save(&self) -> Result<(), Box<dyn Error>> {
        utils::write_data(DATA_FILENAME, self)?;
        Ok(())
    }
    pub fn find_path(&self, game: Game) -> Option<&Path> {
        self.game_data[&game]
            .custom_game_path
            .as_deref()
            .or(DEFAULT_GAME_PATHS
                .get(&game)
                .map(|path_buf| path_buf.as_path()))
    }
    pub fn is_path_defined(&self, game: Game) -> bool {
        self.find_path(game).is_some()
    }
    pub fn set_path(&mut self, game: Game, new_path: &Path) {
        self.game_data
            .get_mut(&game)
            .unwrap()
            .custom_game_path
            .replace(new_path.to_path_buf());
    }
    // TODO: Do not expose Vec, change method name
    pub fn get(&self, game: Game) -> &Vec<SaveSlot> {
        &self.game_data[&game].save_slots
    }
    // TODO: Do not expose indexes, use HashMap with keys or expose references somehow
    pub fn create(&mut self, game: Game, label: &str) -> Result<usize, Box<dyn Error>> {
        assert!(self.is_path_defined(game));

        let save_slot = SaveSlot::new(label, &game.to_string());
        fs::create_dir_all(save_slot.path())?;
        let save_slots = &mut self.game_data.get_mut(&game).unwrap().save_slots;
        save_slots.push(save_slot);

        let len = save_slots.len();

        self.save()?;
        Ok(len - 1)
    }
    // TODO: Add import from folder intrsuctions
    // FIXME: Do something about empty imports, will panic right now
    pub fn import(&mut self, game: Game, label: &str) -> Result<(), Box<dyn Error>> {
        assert!(self.is_path_defined(game));
        assert!(self.game_data[&game].save_slots.is_empty());

        // TODO: Update this to not use the index
        let index_slot = self.create(game, label)?;

        let game_path = self.find_path(game).unwrap();
        let save_slot_path = self.game_data[&game].save_slots[index_slot].path();

        // Copies the whole game's save folder
        utils::copy_dir_all(
            game_path,
            save_slot_path.join(game_path.canonicalize()?.file_name().unwrap()),
        )?;
        // TODO: Use setter instead or just don't share SaveSlot
        self.game_data
            .get_mut(&game)
            .unwrap()
            .loaded_slot
            .replace(index_slot);

        self.save()?;
        Ok(())
    }
    pub fn rename(
        &mut self,
        game: Game,
        index: usize,
        new_label: &str,
    ) -> Result<(), Box<dyn Error>> {
        assert!(self.is_path_defined(game));
        // FIXME: Bad check
        assert!(self.game_data[&game].save_slots.len() > index);

        let save_slot = &mut self.game_data.get_mut(&game).unwrap().save_slots[index];
        fs::rename(save_slot.path(), save_slot.path().with_file_name(new_label))?;
        save_slot.set_label(new_label);

        self.save()?;
        Ok(())
    }
    pub fn delete(&mut self, game: Game, index: usize) -> Result<(), Box<dyn Error>> {
        assert!(self.is_path_defined(game));
        // FIXME: Bad check
        assert!(self.game_data[&game].save_slots.len() > index);

        // TODO: Change these when SaveSlot becomes private
        let save_slot = self
            .game_data
            .get_mut(&game)
            .unwrap()
            .save_slots
            .remove(index);
        let save_slot_path = save_slot.path();
        let game_path = self.find_path(game).unwrap();
        if self.is_loaded(game, index) {
            utils::remove_dir_contents(game_path)?;
        }
        fs::remove_dir_all(save_slot_path)?;

        self.save()?;
        Ok(())
    }
    pub fn load(&mut self, game: Game, index: usize) -> Result<(), Box<dyn Error>> {
        assert!(self.is_path_defined(game));
        assert!(!self.is_loaded(game, index));
        // FIXME: Bad check
        assert!(self.game_data[&game].save_slots.len() > index);

        let save_slot_path = self.game_data[&game].save_slots[index].path();
        let game_path = self.find_path(game).unwrap();

        utils::remove_dir_contents(game_path)?;
        // Copies the whole game's save folder
        utils::copy_dir_all(
            save_slot_path.join(game_path.canonicalize()?.file_name().unwrap()),
            game_path,
        )?;
        // TODO: Use setter or don't expose SaveSlot
        // TOOD: Stop using indexes
        self.game_data
            .get_mut(&game)
            .unwrap()
            .loaded_slot
            .replace(index);

        self.save()?;
        Ok(())
    }
    pub fn unload(&mut self, game: Game, index: usize) -> Result<(), Box<dyn Error>> {
        assert!(self.is_path_defined(game));
        assert!(self.is_loaded(game, index));
        // FIXME: Bad check
        assert!(self.game_data[&game].save_slots.len() > index);

        let save_slot_path = self.game_data[&game].save_slots[index].path();
        let game_path = self.find_path(game).unwrap();

        utils::copy_dir_all(
            game_path,
            save_slot_path.join(game_path.canonicalize()?.file_name().unwrap()),
        )?;
        utils::remove_dir_contents(game_path)?;
        self.game_data.get_mut(&game).unwrap().loaded_slot.take();

        self.save()?;
        Ok(())
    }
    pub fn is_loaded(&self, game: Game, index: usize) -> bool {
        assert!(self.is_path_defined(game));

        self.game_data[&game]
            .loaded_slot
            .is_some_and(|i| i == index)
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
                if save_swapper.is_path_defined(*game) {
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
        if !save_swapper.is_path_defined(game) {
            continue;
        }

        loop {
            utils::clear_screen(&term, Some(&game.to_string()), None)?;
            if save_swapper.get(game).is_empty() {
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
                .enumerate()
                .map(|e| {
                    let loaded_str = if save_swapper.is_loaded(game, e.0) {
                        "X"
                    } else {
                        " "
                    };
                    String::from("[") + loaded_str + "] " + e.1.label()
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
            let save_slot_loaded = save_swapper.is_loaded(game, save_slot_index);
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
                }
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
