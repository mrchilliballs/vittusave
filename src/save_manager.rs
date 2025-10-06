use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex},
};

use anyhow::Result;
use mediawiki::ApiSync;
use serde::{Deserialize, Serialize};
use steamlocate::SteamDir;
use strum::Display;

use crate::{
    consts::{DATA_FILENAME, PCGW_API},
    dir_swapper::DirSwapper,
    pcgw::{self, PCGWError},
    utils,
};

// TODO: cache this in $XDG_CACHE_HOME, etc.
static NAME_CACHE: LazyLock<Mutex<HashMap<GameId, Result<Arc<str>, Arc<anyhow::Error>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::default()));

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Clone, Copy, Display)]
#[non_exhaustive]
pub enum GameId {
    #[strum(to_string = "{0}")]
    Steam(u32),
}
impl GameId {
    // TODO: See if I can figure out a better return type
    pub fn get_name(&self) -> Result<Arc<str>, Arc<anyhow::Error>> {
        let mut name_cache = NAME_CACHE.lock().unwrap();
        if let Some(result) = name_cache.get(self) {
            result
                .as_ref()
                .map(|err| err.clone())
                .map_err(|err| err.clone())
        } else {
            let api = match ApiSync::new(PCGW_API) {
                Ok(api) => api,
                Err(err) => {
                    name_cache.insert(*self, Err(Arc::new(err.into())));
                    return name_cache
                        .get(self)
                        .unwrap()
                        .as_ref()
                        .map(|name| name.clone())
                        .map_err(|err| err.clone());
                }
            };
            let result = pcgw::fetch_page_by_id(&api, *self);
            name_cache.insert(
                *self,
                result
                    .map(|name| Arc::from(name))
                    .map_err(|err| Arc::new(err.into())),
            );
            name_cache
                .get(self)
                .unwrap()
                .as_ref()
                .map(|name| name.clone())
                .map_err(|err| err.clone())
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SlotMeta {
    // TODO: date created, last loaded, etc.
}

// TODO: Use getters and setters for relevant data maybe to not expose irrelevant
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Game {
    pub slot_metadata: HashMap<String, SlotMeta>,
    pub slot_swapper: DirSwapper,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct SaveManager {
    game_data: HashMap<GameId, Game>,
}

impl Drop for SaveManager {
    fn drop(&mut self) {
        // FIXME: what to do here?
        let _ = self.save();
    }
}

impl SaveManager {
    /// Loads save swapper from `crate::consts::DATA_FILENAME`
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
    /// Finds the steam directory and loads games from all libraries. Returns `Ok(false)` if no
    /// games or libraries are found but Steam is installed.
    pub fn load_steam_library(&mut self) -> Result<bool> {
        let steam_dir = SteamDir::locate()?;
        let api = ApiSync::new(PCGW_API)?;
        let steam_library = steam_dir.libraries()?.next();

        if let Some(steam_library) = steam_library {
            let steam_library = steam_library?;
            for &app_id in steam_library.app_ids() {
                // Makes sure that app is a game
                match pcgw::fetch_page_by_id(&api, GameId::Steam(app_id)) {
                    Ok(_) => {
                        self.game_data
                            .insert(GameId::Steam(app_id), Game::default());
                    }
                    // Not a game
                    Err(PCGWError::NotFound) => {}
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
    pub fn path(&self, id: GameId) -> Option<&Path> {
        self.game_data
            .get(&id)
            .map(|game| game.slot_swapper.get_version_dir())
    }
    #[inline]
    pub fn set_path(&mut self, id: GameId, new_path: PathBuf) -> Result<(), ()> {
        self.game_data.get_mut(&id).map_or(Err(()), |game| {
            Ok(game.slot_swapper.set_primary_dir(new_path))
        })
    }
    #[inline]
    pub fn contains(&self, id: GameId) -> bool {
        self.game_data.contains_key(&id)
    }
    // TODO: do not expose Vec
    #[inline]
    pub fn get(&self, id: GameId) -> Option<&Game> {
        self.game_data.get(&id)
    }
    // TODO: do not expose indexes, use HashMap with keys or expose references somehow
    pub fn create(&mut self, id: GameId, name: &str) -> Result<Option<()>> {
        self.game_data
            .get_mut(&id)
            .map(|game| game.slot_swapper.add_version(name))
            .transpose()
            .map(|game| game.flatten())
    }
    pub fn rename(&mut self, game: GameId, name: &str, new_name: &str) -> Result<Option<()>> {
        self.game_data
            .get_mut(&game)
            .map(|game| game.slot_swapper.rename_version(name, new_name))
            .transpose()
            .map(|game| game.flatten())
    }
    pub fn delete(&mut self, game: GameId, name: &str) -> Result<Option<()>> {
        self.game_data
            .get_mut(&game)
            .map(|game| game.slot_swapper.delete_version(name))
            .transpose()
            .map(|game| game.flatten())
    }
    pub fn load(&mut self, game: GameId, name: String) -> Result<Option<()>> {
        self.game_data
            .get_mut(&game)
            .map(|game| game.slot_swapper.set_active(name))
            .transpose()
            .map(|game| game.flatten())
    }
    #[inline]
    pub fn is_loaded(&self, game: GameId, name: &str) -> Option<bool> {
        self.game_data.get(&game).map(|game| {
            game.slot_swapper
                .active_version()
                .is_some_and(|version| version == name)
        })
    }
    #[inline]
    pub fn game_data(&self) -> &HashMap<GameId, Game> {
        &self.game_data
    }
}
