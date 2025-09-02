#![allow(dead_code)]

// TODO: do some sort of integrity check before loading saves
// TODO: steam Cloud support (info UT favorites)
// TODO: docs
// FIXME: bug when game save files don't exist
// TODO: unit tests
// TODO: use Cow for strings?
// TODO: alphabetical ordering of games, should be configurable

// TODO: replace legacy system

mod consts;
mod pcgw;
mod utils;

use anyhow::Result;
use console::{Term, style};
use dialoguer::{Confirm, Input, Select};
use itertools::Itertools;
use mediawiki::{ApiSync, api};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use steamlocate::SteamDir;

use crate::{
    consts::{DATA_FILENAME, PCGW_API, SAVE_SLOT_PATH}, pcgw::PCGWError,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveSlot {
    label: String,
    path: PathBuf,
}

#[non_exhaustive]
#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Clone, Copy)]
pub enum GameId {
    Steam(u32),
}

impl SaveSlot {
    pub fn new(label: String, game_name: String) -> Self {
        SaveSlot {
            label: label.clone(),
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

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GameData {
    pub save_slots: Vec<SaveSlot>,
    // TODO: use pointer instead, maybe
    pub loaded_slot: Option<usize>,
    pub game_path: PathBuf,
}

// TODO: game-specific settings
// TODO: separate game save/slots data struct instead of multiple fields
#[derive(Debug, Serialize, Deserialize, Default)]
struct SaveSwapper {
    game_data: HashMap<GameId, GameData>,
}

impl Drop for SaveSwapper {
    fn drop(&mut self) {
        // FIXME: what to do here?
        let _ = self.save();
    }
}

// TODO: reorder functionality
impl SaveSwapper {
    pub fn build() -> Result<Self> {
        let save_swapper_data = utils::read_data(DATA_FILENAME)?;

        if let Some(save_swapper) = save_swapper_data {
            Ok(save_swapper)
        } else {
            let save_swapper = save_swapper_data.unwrap_or_default();
            save_swapper.save()?;
            Ok(save_swapper)
        }
    }
    #[inline]
    fn save(&self) -> Result<()> {
        utils::write_data(DATA_FILENAME, self)?;
        Ok(())
    }
    #[inline]
    pub fn path(&self, game: GameId) -> &Path {
        &self.game_data[&game].game_path
    }
    #[inline]
    pub fn set_path(&mut self, game: GameId, new_path: &Path) {
        self.game_data.get_mut(&game).unwrap().game_path = new_path.to_path_buf();
    }
    // TODO: do not expose Vec, change method name
    #[inline]
    pub fn get(&self, game: GameId) -> &Vec<SaveSlot> {
        &self.game_data[&game].save_slots
    }
    // TODO: do not expose indexes, use HashMap with keys or expose references somehow
    pub fn create(&mut self, game: GameId, label: String) -> Result<usize> {
        let api = ApiSync::new(PCGW_API)?;

        let save_slot = SaveSlot::new(label, pcgw::utils::fetch_page_by_id(&api, game)?);
        fs::create_dir_all(save_slot.path())?;
        let save_slots = &mut self.game_data.get_mut(&game).unwrap().save_slots;
        save_slots.push(save_slot);

        let len = save_slots.len();

        self.save()?;
        Ok(len - 1)
    }
    // TODO: add import from folder intrsuctions
    // FIXME: do something about empty imports, will panic right now
    pub fn import(&mut self, game: GameId, label: String) -> Result<()> {
        assert!(self.game_data[&game].save_slots.is_empty());

        // TODO: update this to not use the index
        let index_slot = self.create(game, label)?;

        let game_path = self.path(game);
        let save_slot_path = self.game_data[&game].save_slots[index_slot].path();

        // copies the whole game's save folder
        utils::copy_dir_all(
            game_path,
            save_slot_path.join(game_path.canonicalize()?.file_name().unwrap()),
        )?;
        // TODO: use setter instead or just don't share SaveSlot
        self.game_data
            .get_mut(&game)
            .unwrap()
            .loaded_slot
            .replace(index_slot);

        self.save()?;
        Ok(())
    }
    pub fn rename(&mut self, game: GameId, index: usize, new_label: &str) -> Result<()> {
        // FIXME: bad check
        assert!(self.game_data[&game].save_slots.len() > index);

        let save_slot = &mut self.game_data.get_mut(&game).unwrap().save_slots[index];
        fs::rename(save_slot.path(), save_slot.path().with_file_name(new_label))?;
        save_slot.set_label(new_label);

        self.save()?;
        Ok(())
    }
    pub fn delete(&mut self, game: GameId, index: usize) -> Result<()> {
        // FIXME: bad check
        assert!(self.game_data[&game].save_slots.len() > index);

        // TODO: change these when SaveSlot becomes private
        let save_slot = self
            .game_data
            .get_mut(&game)
            .unwrap()
            .save_slots
            .remove(index);
        let save_slot_path = save_slot.path();
        let game_path = self.path(game);
        if self.is_loaded(game, index) {
            utils::remove_dir_contents(game_path)?;
        }
        fs::remove_dir_all(save_slot_path)?;

        self.save()?;
        Ok(())
    }
    pub fn load(&mut self, game: GameId, index: usize) -> Result<()> {
        assert!(!self.is_loaded(game, index));
        // FIXME: bad check
        assert!(self.game_data[&game].save_slots.len() > index);

        let save_slot_path = self.game_data[&game].save_slots[index].path();
        let game_path = self.path(game);

        utils::remove_dir_contents(game_path)?;
        // Copies the whole game's save folder
        utils::copy_dir_all(
            save_slot_path.join(game_path.canonicalize()?.file_name().unwrap()),
            game_path,
        )?;
        // TODO: use setter or don't expose SaveSlot
        // TOOD: stop using indexes
        self.game_data
            .get_mut(&game)
            .unwrap()
            .loaded_slot
            .replace(index);

        self.save()?;
        Ok(())
    }
    pub fn unload(&mut self, game: GameId, index: usize) -> Result<()> {
        assert!(self.is_loaded(game, index));
        // FIXME: bad check
        assert!(self.game_data[&game].save_slots.len() > index);

        let save_slot_path = self.game_data[&game].save_slots[index].path();
        let game_path = self.path(game);

        utils::copy_dir_all(
            game_path,
            save_slot_path.join(game_path.canonicalize()?.file_name().unwrap()),
        )?;
        utils::remove_dir_contents(game_path)?;
        self.game_data.get_mut(&game).unwrap().loaded_slot.take();

        self.save()?;
        Ok(())
    }
    #[inline]
    pub fn is_loaded(&self, game: GameId, index: usize) -> bool {
        self.game_data[&game]
            .loaded_slot
            .is_some_and(|i| i == index)
    }
}

// TODO: use async Steam API in the future
fn main() -> Result<()> {
    env_logger::init();

    let term = Term::stdout();
    let steam_dir = SteamDir::locate()?;
    // TODO: remove unwrap, deal with multiple libraries
    let steam_library = steam_dir.libraries()?.nth(0).unwrap()?;
    println!("Steam installation - {}", steam_dir.path().display());
    let api = ApiSync::new(PCGW_API)?;

    let mut save_swapper: SaveSwapper = SaveSwapper::build()?;

    loop {
        utils::clear_screen(&term, None, None)?;
        let mut items: Vec<_> = steam_library
            .apps()
            .filter_map(|game| {
                // TODO: remove unwrap
                match pcgw::utils::fetch_page_by_id(&api, GameId::Steam(game.unwrap().app_id)) {
                    Ok(page) => Some(Ok(page)),
                    Err(PCGWError::NotFound) => None, // just skip it
                    Err(err) => return Some(Err(err)),    // propagate other errors
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        items.push("Settings".to_string());
        println!("Selecting");
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
        //     let game = Game::VARIANTS[selection];
        //     if !save_swapper.is_path_defined(game) {
        //         continue;
        //     }

        //     loop {
        //         utils::clear_screen(&term, Some(&game.to_string()), None)?;
        //         if save_swapper.get(game).is_empty() {
        //             let confirmation = Confirm::new()
        //                 .with_prompt("No saves found. Register the current one?")
        //                 .interact_on(&term)?;
        //             if confirmation {
        //                 // TODO: deduplicate code
        //                 let label: String = Input::new()
        //                     .with_prompt("Enter save label")
        //                     .interact_text_on(&term)?;
        //                 save_swapper.import(game, &label)?;
        //             } else {
        //                 break;
        //             }
        //             continue;
        //         }
        //         let items: Vec<String> = save_swapper
        //             .get(game)
        //             .iter()
        //             .enumerate()
        //             .map(|e| {
        //                 let loaded_str = if save_swapper.is_loaded(game, e.0) {
        //                     "X"
        //                 } else {
        //                     " "
        //                 };
        //                 String::from("[") + loaded_str + "] " + e.1.label()
        //             })
        //             .chain([String::from("New")])
        //             .collect();

        //         let Some(selection) = Select::new()
        //             .with_prompt("Select a save")
        //             .items(&items)
        //             .default(0)
        //             .interact_on_opt(&term)?
        //         else {
        //             break;
        //         };
        //         if items[selection] == "New" {
        //             utils::clear_screen(&term, Some(&game.to_string()), None)?;
        //             // TODO: deduplicate code
        //             let label: String = Input::new()
        //                 .with_prompt("Enter save label")
        //                 .interact_text_on(&term)?;
        //             save_swapper.create(game, &label)?;
        //             continue;
        //         }
        //         let save_slot_index = selection;

        //         utils::clear_screen(
        //             &term,
        //             Some(&game.to_string()),
        //             Some(save_swapper.get(game)[save_slot_index].label()),
        //         )?;
        //         let mut items: Vec<&'static str> = vec![];
        //         let save_slot_loaded = save_swapper.is_loaded(game, save_slot_index);
        //         if save_slot_loaded {
        //             items.push("Unload");
        //         } else {
        //             items.push("Load");
        //         }
        //         items.push("Rename");
        //         items.push("Delete");
        //         let Some(selection) = Select::new()
        //             .with_prompt("Select an action")
        //             .items(&items)
        //             .default(0)
        //             .interact_on_opt(&term)?
        //         else {
        //             continue;
        //         };
        //         match items[selection] {
        //             "Load" => save_swapper.load(game, save_slot_index)?,
        //             "Unload" => save_swapper.unload(game, save_slot_index)?,
        //             "Rename" => {
        //                 utils::clear_screen(
        //                     &term,
        //                     Some(&game.to_string()),
        //                     Some(save_swapper.get(game)[save_slot_index].label()),
        //                 )?;
        //                 let new_label: String = Input::new()
        //                     .with_prompt("Enter new label")
        //                     .with_initial_text(save_swapper.get(game)[save_slot_index].label())
        //                     .interact_text_on(&term)?;
        //                 save_swapper.rename(game, save_slot_index, &new_label)?;
        //             }
        //             "Delete" => {
        //                 utils::clear_screen(
        //                     &term,
        //                     Some(&game.to_string()),
        //                     Some(save_swapper.get(game)[save_slot_index].label()),
        //                 )?;
        //                 let confirmation = Confirm::new()
        //                     .with_prompt(format!(
        //                         "Are you sure you want to {} delete \"{}\"?",
        //                         style("permanently").red(),
        //                         save_swapper.get(game)[save_slot_index].label()
        //                     ))
        //                     .interact_on(&term)?;
        //                 if confirmation {
        //                     save_swapper.delete(game, save_slot_index)?;
        //                 }
        //             }
        //             _ => unreachable!(),
        //         }
        //     }
    }
    Ok(())
}
