#![allow(dead_code)]

// TODO: do some sort of integrity check before loading saves
// TODO: steam Cloud support (info UT favorites)
// TODO: docs
// FIXME: bug when game save files don't exist
// TODO: unit tests
// TODO: use Cow for strings?
// TODO: alphabetical ordering of games, should be configurable

// TODO: replace legacy system

mod app;
mod consts;
mod pcgw;
mod utils;

use anyhow::Result;
use mediawiki::ApiSync;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use steamlocate::SteamDir;

use crate::{
    app::App,
    consts::{DATA_FILENAME, PCGW_API, SAVE_SLOT_PATH},
    pcgw::PCGWError,
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
pub struct Game {
    pub save_slots: Vec<SaveSlot>,
    // TODO: use pointer instead, maybe
    pub loaded_slot: Option<usize>,
    pub game_path: PathBuf,
}

// TODO: game-specific settings
// TODO: separate game save/slots data struct instead of multiple fields
#[derive(Debug, Serialize, Deserialize)]
struct SaveSwapper {
    game_data: HashMap<GameId, Game>,
}

impl Default for SaveSwapper {
    fn default() -> Self {
        let swapper = SaveSwapper {
            game_data: HashMap::default(),
        };
        swapper
    }
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
    // Finds the steam directory and loads games from all libraries. Returns `Ok(false)` if no
    // games or libraries are found but Steam is installed.
    pub fn load_steam_library(&mut self) -> Result<bool> {
        let steam_dir = SteamDir::locate()?;
        let api = ApiSync::new(PCGW_API)?;
        let steam_library = steam_dir.libraries()?.next();

        if let Some(steam_library) = steam_library {
            let steam_library = steam_library?;
            for &app_id in steam_library.app_ids() {
                match pcgw::utils::fetch_page_by_id(&api, GameId::Steam(app_id)) {
                    Ok(_) | Err(PCGWError::NotFound) => {
                        self.game_data
                            .insert(GameId::Steam(app_id), Game::default());
                    }
                    Err(err) => return Err(err.into()),
                }
            }
            if self.game_data.is_empty() {
                return Ok(false);
            }
            Ok(true)
        } else {
            Ok(false)
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
    #[inline]
    pub fn contains(&self, game: GameId) -> bool {
        self.game_data.contains_key(&game)
    }
    // TODO: do not expose Vec
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

    let steam_dir = SteamDir::locate()?;
    // TODO: remove unwrap, deal with multiple libraries
    let steam_library = steam_dir.libraries()?.next().unwrap()?;
    println!("Steam installation - {}", steam_dir.path().display());
    let api = ApiSync::new(PCGW_API)?;

    let save_swapper: SaveSwapper = SaveSwapper::build()?;
    let _games: Vec<_> = steam_library
        .apps()
        .filter_map(|game| {
            // TODO: remove unwrap
            let steam_id = match game {
                Ok(game) => game.app_id,
                Err(err) => return Some(Err(err.into())),
            };
            match pcgw::utils::fetch_page_by_id(&api, GameId::Steam(steam_id)) {
                Ok(page) => {
                    if save_swapper.contains(GameId::Steam(steam_id)) {
                        // TODO: Set to green color
                        Some(Ok(page.to_string()))
                    } else {
                        // TODO: Set to red color
                        Some(Ok(format!("{page} (unloaded)")))
                    }
                }
                Err(PCGWError::NotFound) => None,
                Err(err) => Some(Err(err.into())),
            }
        })
        .chain([Ok(String::from("Add")), Ok(String::from("Settings"))])
        .collect::<Result<_>>()?;

    let terminal = ratatui::init();
    let result = App::new().run(terminal);
    ratatui::restore();
    result
}
