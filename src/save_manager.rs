use std::{
    cell::Ref,
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    num::ParseIntError,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, LazyLock, Mutex},
};

use anyhow::Result;
use mediawiki::ApiSync;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use steamlocate::SteamDir;
use strum::Display;

use crate::{
    consts::{DATA_FILENAME, PCGW_API},
    dir_swapper::DirSwapper,
    pcgw::{self, PCGWError},
    utils::{self, Cached, states},
};

#[derive(Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Clone, Copy, Display)]
#[non_exhaustive]
pub enum GameId {
    // #[strum(to_string = "{}")]
    Steam(u32),
}
impl GameId {
    // TODO: use better approach
    pub fn fetch_name(self) -> Result<String, anyhow::Error> {
        Ok(pcgw::fetch_page_by_id(&ApiSync::new(PCGW_API)?, self)?)
    }
}
impl FromStr for GameId {
    type Err = ParseIntError;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(GameId::Steam(s.parse()?))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SlotMeta {
    // TODO: date created, last loaded, etc.
}

// TODO: Use getters and setters for relevant data maybe to not expose irrelevant
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GameSaves {
    pub slot_metadata: HashMap<String, SlotMeta>,
    pub slot_swapper: DirSwapper,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct SaveManager {
    /// TODO: make GameId use strings and a getter and setter maybe or find another solution.
    #[serde_as(as = "HashMap<serde_with::DisplayFromStr, _>")]
    game_data: HashMap<GameId, GameSaves>,
    name_cache: Cached<states::Resolved<BTreeMap<String, GameId>>>,
    steam_loaded: bool,
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
            println!("Found swapper");
            Ok(save_swapper)
        } else {
            let save_swapper = save_swapper_data.unwrap_or(Self {
                game_data: Default::default(),
                name_cache: Cached::default().read()?,
                steam_loaded: Default::default(),
            });
            save_swapper.save()?;
            Ok(save_swapper)
        }
    }
    /// Finds the steam directory and loads games from all libraries. Returns `Ok(false)` if no
    /// games or libraries are found but Steam is installed.
    pub fn load_steam_library(&mut self) -> Result<bool> {
        // TODO: Maybe don't store this as a flag?
        if !self.steam_loaded {
            let steam_dir = SteamDir::locate()?;
            let api = ApiSync::new(PCGW_API)?;
            let steam_library = steam_dir.libraries()?.next();

            if let Some(steam_library) = steam_library {
                let steam_library = steam_library?;
                for &app_id in steam_library.app_ids() {
                    // Makes sure that app is a game
                    match pcgw::fetch_page_by_id(&api, GameId::Steam(app_id)) {
                        Ok(_) => {
                            let id = GameId::Steam(app_id);
                            self.game_data.insert(id, GameSaves::default());
                            // TODO: Do something about this expensive operation, likely a loading
                            // screen. On demand cash refresh would be great too.
                            let cache = self.name_cache.get_mut();
                            println!("Running...");
                            cache.insert(id.fetch_name()?, id);
                        }
                        // Not a game
                        Err(PCGWError::NotFound) => {}
                        Err(err) => return Err(err.into()),
                    }
                }
                if self.game_data.is_empty() {
                    return Ok(false);
                }

                self.steam_loaded = true;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(true)
        }
    }
    #[inline]
    pub fn save(&self) -> Result<()> {
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
    pub fn get(&self, id: GameId) -> Option<&GameSaves> {
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
    // TODO: Make a `Vec<&str>` or something to improve performance
    pub fn games(&self) -> &BTreeMap<String, GameId> {
        self.name_cache.get()
    }
}
