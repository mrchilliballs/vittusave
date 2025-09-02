use anyhow::Result;
use console::Term;
use mediawiki::ApiSync;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::{
    consts::DATA_DIR,
    game_data::{GameDataError, GameId},
};

pub fn clear_screen(term: &Term, game: Option<&str>, save: Option<&str>) -> io::Result<()> {
    term.clear_screen()?;
    print!("GAME: ");
    if let Some(game) = game {
        println!("{game}");
    } else {
        println!("NONE");
    }
    print!("SAVE: ");
    if let Some(save) = save {
        println!("{save}");
    } else {
        println!("NONE");
    }
    println!();
    Ok(())
}

fn make_data_path(filename: impl AsRef<Path>) -> PathBuf {
    let mut path = DATA_DIR.clone();
    path.push(filename);
    path.set_extension("toml");

    path
}

pub fn write_data<T: Serialize>(filename: impl AsRef<Path>, config: &T) -> Result<()> {
    fs::create_dir_all(DATA_DIR.as_path())?;

    let path = make_data_path(filename);

    let config_str = toml::to_string(config)?;
    fs::write(path, config_str)?;

    Ok(())
}

pub fn read_data<T: DeserializeOwned>(filename: impl AsRef<Path>) -> Result<Option<T>> {
    fs::create_dir_all(DATA_DIR.as_path())?;

    let path = make_data_path(&filename);

    match fs::exists(&path) {
        Ok(true) => {
            let config_str = fs::read_to_string(&path)?;
            Ok(Some(toml::from_str(&config_str)?))
        }
        Ok(false) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

pub fn remove_dir_contents(path: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&path)?;
    fs::remove_dir_all(&path)?;
    fs::create_dir(&path)
}

// https://stackoverflow.com/a/65192210
// TODO: iteration instead of recursion
pub fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Looks up a Steam ID in the PCGW and returns the name of the page, if it exists
pub fn fetch_page_by_id(api: &ApiSync, steam_id: GameId) -> Result<String, GameDataError> {
    #[allow(unreachable_patterns)]
    let steam_id = match steam_id {
        GameId::Steam(id) => id,
        _ => todo!("non-Steam `GameId`s are not yet supported"),
    };
    // Query parameters
    let params = api.params_into(&[
        ("action", "cargoquery"),
        ("tables", "Infobox_game"),
        ("fields", "Infobox_game._pageName=Page"),
        ("where", &format!("Steam_AppID HOLDS {steam_id}")),
    ]);

    // Run query; this will automatically continue if more results are available, and merge all results into one
    let res = api.get_query_api_json_all(&params)?;

    res["cargoquery"]
        .as_array()
        .ok_or(GameDataError::ParseError)?
        .first()
        .ok_or(GameDataError::NotFound)?["title"]["Page"]
        .as_str()
        .map(str::to_string)
        .ok_or(GameDataError::ParseError)
}
