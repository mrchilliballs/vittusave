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
mod dir_swapper;
mod pcgw;
mod save_manager;
mod utils;

use anyhow::Result;

use crate::app::App;

// TODO: use async Steam API in the future
fn main() -> Result<()> {
    env_logger::init();

    // let steam_dir = SteamDir::locate()?;
    // // TODO: remove unwrap, deal with multiple libraries
    // let steam_library = steam_dir.libraries()?.next().unwrap()?;
    // println!("Steam installation - {}", steam_dir.path().display());
    // let api = ApiSync::new(PCGW_API)?;
    //
    // let save_swapper: SaveSwapper = SaveSwapper::build()?;
    // let _games: Vec<_> = steam_library
    //     .apps()
    //     .filter_map(|game| {
    //         // TODO: remove unwrap
    //         let steam_id = match game {
    //             Ok(game) => game.app_id,
    //             Err(err) => return Some(Err(err.into())),
    //         };
    //         match pcgw::utils::fetch_page_by_id(&api, GameId::Steam(steam_id)) {
    //             Ok(page) => {
    //                 if save_swapper.contains(GameId::Steam(steam_id)) {
    //                     // TODO: Set to green color
    //                     Some(Ok(page.to_string()))
    //                 } else {
    //                     // TODO: Set to red color
    //                     Some(Ok(format!("{page} (unloaded)")))
    //                 }
    //             }
    //             Err(PCGWError::NotFound) => None,
    //             Err(err) => Some(Err(err.into())),
    //         }
    //     })
    //     .chain([Ok(String::from("Add")), Ok(String::from("Settings"))])
    //     .collect::<Result<_>>()?;
    //
    let terminal = ratatui::init();
    let result = App::new().run(terminal);
    ratatui::restore();
    result
}
