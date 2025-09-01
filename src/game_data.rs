// TODO: remember to use #[inline] functions when refactoring this

use std::{
    collections::HashMap,
    env,
    ffi::OsString,
    fs,
    hash::Hash,
    io,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use mediawiki::{Api, MediaWikiError, api_sync::ApiSync};
use scraper::{ElementRef, Html, Node, Selector};
use thiserror::Error;

use log::{debug, info, trace, warn};

use itertools::Itertools;

// TODO: rename this error type
#[derive(Debug, Error)]
pub enum GameDataError {
    #[error("failed to fetch data from MediaWiki API")]
    MediaWikiError(#[from] MediaWikiError),
    #[error("parse error")]
    ParseError,
    #[error("error reading or rendering note HTML")]
    NoteError(#[from] html2text::Error),
    #[error("no data returned by the server")]
    NotFound,
}

#[derive(Debug, Error)]
pub enum LocationError {
    #[error("built path does not exist")]
    InvalidPath(#[from] io::Error),
    #[error("undefined abbreviation in path")]
    UndefinedAbbr,
}

// Environment variable placeholders
pub static ENV_VARS: LazyLock<HashMap<&str, &[&str]>> = LazyLock::new(|| {
    HashMap::from([
        (
            "windows",
            [
                "%USERPROFILE%",
                "%APPDATA%",
                "%LOCALAPPDATA%",
                "%TEMP%",
                "%PUBLIC%",
                "%PROGRAMDATA%",
                "%PROGRAMFILES%", // FIXME: check for byteness
                "%WINDIR%",
            ]
            .as_ref(),
        ),
        ("macos", ["$HOME"].as_ref()),
        (
            "linux",
            ["$HOME", "$XDG_DATA_HOME", "$XDG_CONFIG_HOME"].as_ref(),
        ),
    ])
});

/// Warning: a value must not contain any of the map's keys, otherwise `replace_path_abbrs` will panic.
pub static ENV_VAR_DEFAULTS: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| {
    HashMap::from([
        ("$XDG_DATA_HOME", "$HOME/.local/share"),
        ("$XDG_CONFIG_HOME", "$HOME/.config"),
    ])
});

// Other Placeholders
pub const STEAM_FOLDER: &str = "<Steam-folder>";
pub const USER_ID: &str = "<user-id>";

/// Pre-processed location
#[derive(Debug, Default)]
pub struct Location {
    path: Option<PathBuf>,
    path_str: String,
    note: Option<String>,
}
impl Location {
    pub fn new(path_str: String, note: Option<String>) -> Self {
        Self {
            path_str,
            note,
            ..Default::default()
        }
    }
    #[inline]
    pub fn path_str(&self) -> &str {
        &self.path_str
    }
    pub fn expand_path(&mut self, install_dir: &Path, user_id: u64) -> Result<(), LocationError> {
        self.path.replace(
            GameData::replace_path_abbrs(
                &self.path_str,
                None,
                ExpansionParams {
                    install_dir,
                    user_id,
                },
            )
            .ok_or(LocationError::UndefinedAbbr)
            .map_or_else(Err, |path| {
                fs::exists(&path)
                    .map_err(LocationError::InvalidPath)
                    .map(|_| path)
            })?,
        );
        Ok(())
    }
    #[inline]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum LocationKind {
    OS(String),
    Steam,
}

// #[derive(Debug, Error)]
// enum LocationBuildError {
//     #[error("failed to communicate with the Mediawiki API")]
//     GameDataError(#[from] GameDataError),
// }

#[derive(Debug, Default)]
pub struct GameData {
    locations: HashMap<LocationKind, Vec<Location>>,
    extra_notes: Vec<String>,
}

#[derive(Debug)]
enum IdTy {
    CiteRef,
    CiteNote,
}

#[derive(Debug, Clone)]
pub struct ExpansionParams<'a> {
    pub install_dir: &'a Path,
    pub user_id: u64,
}

// TODO: Can user ID, steam path, etc. be turned optional somewhow?
impl GameData {
    /// This is function is nescessary because the numbers that prefix IDs are different when requesting for only a specific section. I don't know why.
    fn format_id(id: &str, ty: IdTy) -> Option<&str> {
        let (start, separator) = match ty {
            IdTy::CiteRef => ("cite_ref-", '_'),
            IdTy::CiteNote => ("cite_note-", '-'),
        };
        let begin = id.find(start)? + start.len();
        let end = id.rfind(separator)?;
        // CiteRef: When there is no note ID, CiteRef is formatted just like CiteNote, it separates the numbers at the end with '-' instead of '_'.
        // `str::rfind` would thus only find the `_` that is in-between `cite_ref, which stands at index 4.
        // Example: "cite_ref-2"
        if end == 4 {
            return Some("");
        }
        // CiteNote: `begin` is one ahead of the `start`` identifier, while `end` is one behind. `begin` will be one greater than `end` when there is no note name.
        // Example: "cite_note-2"
        if begin == end + 1 {
            return Some("");
        }
        Some(&id[begin..end])
    }

    /// Should be updated from <https://www.pcgamingwiki.com/wiki/Glossary:Game_data>
    /// Returns None if an undefined abbreviation in the path
    /// Supports non-unicode encoded env variable values
    /// Finds first match only
    fn replace_path_abbrs(
        path: &str,
        os: Option<&str>,
        params: ExpansionParams,
    ) -> Option<PathBuf> {
        let os = os.unwrap_or(std::env::consts::OS);

        let mut replacement_locations: HashMap<usize, &str> = HashMap::new();
        let mut replacement_data: HashMap<&str, OsString> = HashMap::new();

        // FIXME: Handle invalid OS without just returning None
        for var_key in *ENV_VARS.get(os)? {
            let var_key_lower = var_key.to_lowercase();
            let var_key_upper = var_key.to_uppercase();
            let find_result = match os {
                "windows" => path
                    .find(&var_key_lower)
                    .or_else(|| path.find(&var_key_upper)),
                "linux" | "macos" => path.find(var_key),
                _ => return None, // FIXME
            };
            if let Some(i) = find_result {
                let var_key_env_name = {
                    match os {
                        "windows" => &var_key_lower,
                        "linux" | "macos" => &var_key[1..], // Removes the "$"
                        _ => return None,                   // FIXME
                    }
                };
                let Some(var_val) = env::var_os(var_key_env_name).or_else(|| {
                    let default_val = ENV_VAR_DEFAULTS.get(var_key)?;
                    assert!(
                        !default_val.contains(var_key),
                        "default value would cause an infinite loop"
                    );
                    GameData::replace_path_abbrs(default_val, Some(os), params.clone())
                        .map(|path_buf| path_buf.into_os_string())
                }) else {
                    warn!("{var_key_env_name} does not have a default and is undefined");
                    return None;
                };
                info!("`{var_key}` env var is defined or has a default");
                replacement_locations.insert(i, var_key);
                replacement_data.insert(var_key, var_val);
            }
        }

        if let Some(i) = path.find(STEAM_FOLDER) {
            replacement_locations.insert(i, STEAM_FOLDER);
            replacement_data.insert(STEAM_FOLDER, params.install_dir.as_os_str().to_os_string());
        }

        if let Some(i) = path.find(USER_ID) {
            replacement_locations.insert(i, USER_ID);
            replacement_data.insert(USER_ID, OsString::from(params.user_id.to_string()));
        }

        let mut buf = OsString::new();
        let mut i = 0;
        while i < path.len() {
            if let Some(var_key) = replacement_locations.get(&i)
                && let Some(var_val) = replacement_data.get(var_key)
            {
                buf.push(var_val);
                i += var_key.len();
                continue;
            }
            buf.push(&path[i..=i]);
            i += 1;
        }

        Some(PathBuf::from(buf))
    }

    fn fetch_page_by_id(api: &ApiSync, steam_id: u32) -> Result<String, GameDataError> {
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
    fn fetch_section_id(
        api: &ApiSync,
        page: &str,
        section_line: &str,
    ) -> Result<String, GameDataError> {
        let params = api.params_into(&[("action", "parse"), ("page", page), ("prop", "sections")]);

        // FIXME: remove .unwrap()
        let res = api.get_query_api_json_all(&params).unwrap();

        res["parse"]["sections"]
            .as_array()
            .ok_or(GameDataError::NotFound)?
            .iter()
            .find_map(|section_val| {
                section_val["line"]
                    .as_str()
                    .filter(|line| section_line == *line)
                    .and(section_val["index"].as_str())
            })
            .map(str::to_string)
            .ok_or(GameDataError::ParseError)
    }
    fn section_html(api: &ApiSync, page: &str, section_line: &str) -> Result<Html, GameDataError> {
        let section_id = GameData::fetch_section_id(api, page, section_line)?;

        let params = api.params_into(&[
            ("action", "parse"),
            ("page", page),
            ("prop", "text"),
            ("section", &section_id),
        ]);

        let res = api.get_query_api_json_all(&params).unwrap();

        res["parse"]["text"]["*"]
            .as_str()
            .map(Html::parse_fragment)
            .ok_or(GameDataError::NotFound)
    }

    fn page_html(api: &ApiSync, page: &str) -> Result<Html, GameDataError> {
        let params = api.params_into(&[("action", "parse"), ("page", page), ("prop", "text")]);

        let res = api.get_query_api_json_all(&params).unwrap();

        res["parse"]["text"]["*"]
            .as_str()
            .map(Html::parse_fragment)
            .ok_or(GameDataError::NotFound)
    }

    fn extract_raw_location(el: ElementRef, notes: &HashMap<&str, String>) -> Location {
        let mut note = None;
        let path_str =
            el.children()
                .fold(String::new(), |mut path_str, child| match child.value() {
                    Node::Element(child_el) if child_el.name() == "a" => {
                        for a_child in child.children() {
                            if let Some(a_child) = a_child.value().as_text() {
                                debug!("pushing `{}` to path", &**a_child);
                                path_str.push_str(a_child);
                            } else if let Some(a_child_el) = a_child.value().as_element()
                                && a_child_el.name() == "abbr"
                                && let Some(abbr_first_child) = a_child.first_child()
                                && let Some(abbr_text) = abbr_first_child.value().as_text()
                            {
                                debug!("pushing abbr `{}` to path", &**abbr_text);
                                path_str.push_str(abbr_text);
                            }
                        }
                        path_str
                    }
                    Node::Element(child_el) if child_el.name() == "sup" => {
                        let note_id = child_el
                            .attr("id")
                            .and_then(|id| GameData::format_id(id, IdTy::CiteRef))
                            .map(|id| id.to_string());
                        debug!("note #id: `{note_id:?}`");
                        note = note_id
                            .and_then(|note_id| notes.get(note_id.as_str()))
                            .map(|note| note.to_string());
                        path_str
                    }
                    Node::Text(child_text) => {
                        debug!("pushing `{}` to path", &**child_text);
                        path_str.push_str(child_text);
                        path_str
                    }
                    _ => path_str,
                });
        debug!("finished building location");
        debug!("completed path: `{path_str:?}`");
        debug!("note: `{note:?}`");
        Location::new(path_str, note)
    }

    fn get_location_data(
        api: &ApiSync,
        steam_id: u32,
    ) -> Result<HashMap<LocationKind, Vec<Location>>, GameDataError> {
        // TODO: Different errors for different steps
        let page = GameData::fetch_page_by_id(api, steam_id)?;
        let section_html = GameData::section_html(api, &page, "Game data")?;
        let page_html = GameData::page_html(api, &page)?;

        let notes = GameData::extract_notes(&page_html)?;

        let parser_output_selector =
            Selector::parse(".mw-parser-output").expect("str should be a valid selector");
        let header_selector =
            Selector::parse(".mw-headline").expect("str should be a valid selector");
        let mut parser_output_iter = section_html
            .select(&parser_output_selector)
            .exactly_one()
            .map_err(|_| GameDataError::NotFound)?
            .child_elements();
        let mut save_table: Result<ElementRef, GameDataError> = Err(GameDataError::NotFound);
        while let Some(el) = parser_output_iter.next() {
            if el.value().name() == "h3"
                && el
                    .select(&header_selector)
                    .exactly_one()
                    .map_err(|_| GameDataError::ParseError)?
                    .text()
                    .next()
                    .ok_or(GameDataError::ParseError)?
                    == "Save game data location"
            {
                save_table = Ok(parser_output_iter.next().ok_or(GameDataError::ParseError)?);
                break;
            }
        }
        // FIXME: Extra notes are not avaliable in section thing
        // TODO: Thumbs down/Thumbs up display, convert HTML with library
        // let extra_notes = parser_output_iter
        //     .map_while(|el| {
        //         (el.value().name() == "dl").then_some(
        //             el.child_elements()
        //                 .exactly_one()
        //                 .map_err(|_| GameDataError::ParseError),
        //         ).inspect(|val| println!("it was {val:?}"))
        //     })
        //     .fold_ok(Vec::new(), |mut acc, el| {
        //         acc.push(el.html());
        //         acc
        //     })?;

        let row_selector =
            Selector::parse(".table-gamedata-body-row").expect("str should be a valid selector");
        let os_selector =
            Selector::parse(".table-gamedata-body-system").expect("str should be a valid selector");
        let location_selector = Selector::parse(".table-gamedata-body-location")
            .expect("str should be a valid selector");
        let infotable_path_selector =
            Selector::parse(".template-infotable-monospace").expect("str should be valid selector");

        let locations = save_table?
            .select(&row_selector)
            .filter_map(|row| {
                let location_el = row.select(&location_selector).next()?;
                let os = row.select(&os_selector).exactly_one().ok()?;
                let os_first_child = os.children().next()?;
                // FIXME: Steam Play path is generated later
                let os_name = if let Some(child) = os_first_child.value().as_element()
                    && child.name() == "abbr"
                {
                    Some(os_first_child.children().next()?.value().as_text()?.trim())
                } else {
                    os_first_child
                        .value()
                        .as_text()
                        .map(|os_name| os_name.trim())
                }?;
                let location_kind = match os_name {
                    "Steam" => LocationKind::Steam,
                    other => LocationKind::OS(other.to_string()),
                };
                let locations = location_el
                    .select(&infotable_path_selector)
                    .map(|location_el| GameData::extract_raw_location(location_el, &notes))
                    .collect();

                Some((location_kind, locations))
            })
            .collect();
        Ok(locations)
    }

    fn extract_notes(page_html: &Html) -> Result<HashMap<&str, String>, GameDataError> {
        let reference_notes_selector =
            Selector::parse("#pcgw-references-notes").expect("str should be a valid selector");
        let reference_list_selector =
            Selector::parse(".references").expect("str should be a valid selector");
        let reference_text_selector =
            Selector::parse(".reference-text").expect("str should be a valid selector");

        let notes_list = page_html
            .select(&reference_notes_selector)
            .exactly_one()
            .ok()
            .and_then(|el| el.select(&reference_list_selector).exactly_one().ok());
        // TODO: also save extra note below the table, detect thumbs down or thumbs up
        // NOTE: if there is more than one note that have no names, they will get overwritten and only the last one will save.
        // This is a limitation caused by sections have different element IDs versus the full parsed page.
        let notes = match notes_list {
            Some(notes_list) => notes_list
                .children()
                .filter_map(ElementRef::wrap)
                .map(|el| -> Result<(&str, String), GameDataError> {
                    let reference_text_el = el
                        .select(&reference_text_selector)
                        .exactly_one()
                        .map_err(|_| GameDataError::ParseError)?;
                    let reference_text_html = reference_text_el.html();
                    Ok((
                        GameData::format_id(
                            el.value().id().ok_or(GameDataError::ParseError)?,
                            IdTy::CiteNote,
                        )
                        .ok_or(GameDataError::ParseError)?,
                        html2text::from_read(
                            reference_text_html.as_bytes(),
                            reference_text_html.len(),
                        )
                        .inspect_err(|err| println!("error is {err}"))?,
                    ))
                })
                .collect::<Result<HashMap<_, _>, _>>()?,
            None => HashMap::default(),
        };

        // for (kind, locations) in self.locations {
        //     for location in locations {

        //     }
        // }
        // .iter_mut()
        // .map(|(kind, mut locations)| {
        //     for location in locations.iter_mut() {
        //         if let Some(Note::Id(id)) = &location.note
        //             && let Some(note) = notes.remove(id.as_str())
        //         {
        //             location.note = Some(Note::Value(note));
        //         }
        //     }
        //     (kind, locations)
        // })
        // .collect();

        Ok(notes)
    }

    pub fn build(api: &ApiSync, steam_id: u32) -> Result<Self, GameDataError> {
        Ok(GameData {
            locations: GameData::get_location_data(api, steam_id)?,
            extra_notes: Vec::new(),
        })
    }
    pub fn get_locations(&mut self, kind: LocationKind) -> &mut [Location] {
        self.locations
            .get_mut(&kind)
            .map(|vec| vec.as_mut_slice())
            .unwrap_or(&mut [])
    }
    // TODO: process_location_data in here
}

// TODO: Return moved data back in error
// fn process_location_data(
//     api: &ApiSync,
//     page: &str,
//     mut game_data: GameData,
// ) -> Result<GameData, GameDataError> {
//     // Location::new(
//     //     data.path,
//     //     data.note_id.and_then(|note_id| notes.remove(note_id)),
//     // )

//     Ok(GameData::new(locations, Vec::new()))
// }

// TODO: Options into struct not multiple fields

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let api = ApiSync::new("https://www.pcgamingwiki.com/w/api.php")?;

    let steam_dir = steamlocate::SteamDir::locate()?;
    println!("Steam installation - {}", steam_dir.path().display());
    // ^^ prints something like `Steam installation - C:\Program Files (x86)\Steam`

    let mut libraries_iter = steam_dir.libraries()?;

    // FIXME: should not panic on ParseError's or any error for that matter
    while let Some(Ok(library)) = libraries_iter.next() {
        let mut apps_iter = library.apps();
        while let Some(Ok(app)) = apps_iter.next() {
            // let page = match fetch_page_by_id(&api, app.app_id) {
            //     Ok(name) => name,
            //     Err(GameDataError::NotFound) => continue,
            //     Err(err) => return Err(err.into()),
            // };
            // let section_id = match fetch_section_id(&api, &page, "Game data") {
            //     Ok(id) => id,
            //     Err(GameDataError::NotFound) => continue,
            //     Err(err) => return Err(err.into()),
            // };
            // debug!("section id for `{page}`: `{section_id:?}`");
            // let section_html = match section_html(&api, &page, &section_id) {
            //     Ok(html) => html,
            //     Err(GameDataError::NotFound) => continue,
            //     Err(err) => return Err(err.into()),
            // };
            // let location_data = match GameData::build(&section_html) {
            //     Ok(data) => data,
            //     Err(GameDataError::NotFound) => continue,
            //     Err(err) => return Err(err.into()),
            // };
            // let mut locations = match process_location_data(&api, &page, location_data) {
            //     Ok(locations) => locations,
            //     Err(GameDataError::NotFound) => continue,
            //     Err(err) => return Err(err.into()),
            // };
            // // TODO: Resolve paths
            // let windows_locations =
            //     locations.get_locations(LocationKind::OS(String::from("Windows")));
            // println!("{:?}", windows_locations);
            let mut game_data = match GameData::build(&api, app.app_id) {
                Ok(data) => data,
                Err(GameDataError::NotFound) => continue,
                Err(err) => return Err(err.into()),
            };
            if let Some(user_id) = app.last_user
                && let Some(location) = game_data
                    .get_locations(LocationKind::OS(String::from("Linux")))
                    .get_mut(0)
            {
                location.expand_path(&library.resolve_app_dir(&app), user_id)?;
                println!("Resolved Linux path: {:?}", location);
            }
            println!("{game_data:?}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    #[test]
    fn test_format_id() {
        // 1    - Template-generated Proton path from website (8/29/25)
        // 2-3  - Undertale Linux notes (8/29/25)
        // 4    - My Summer Car Unity note (8/29/25)
        let (sample_ref_1, sample_note_1) = (
            "cite_ref-Proton_path_note_4-0",
            "cite_note-Proton_path_note-4",
        );
        let (sample_ref_2, sample_note_2) = ("cite_ref-Steam_old_3-0", "cite_note-Steam_old-3");
        let (sample_ref_3, sample_note_3) = ("cite_ref-Steam_2-0", "cite_note-Steam-2");
        let (sample_ref_4, sample_note_4) = ("cite_ref-2", "cite_note-2");
        let correct1 = "Proton_path_note";
        let correct2 = "Steam_old";
        let correct3 = "Steam";
        let correct4 = "";

        assert_eq!(
            GameData::format_id(sample_ref_1, IdTy::CiteRef),
            Some(correct1)
        );
        assert_eq!(
            GameData::format_id(sample_ref_2, IdTy::CiteRef),
            Some(correct2)
        );
        assert_eq!(
            GameData::format_id(sample_ref_3, IdTy::CiteRef),
            Some(correct3)
        );
        assert_eq!(
            GameData::format_id(sample_ref_4, IdTy::CiteRef),
            Some(correct4)
        );
        assert_eq!(
            GameData::format_id(sample_note_1, IdTy::CiteNote),
            Some(correct1)
        );
        assert_eq!(
            GameData::format_id(sample_note_2, IdTy::CiteNote),
            Some(correct2)
        );
        assert_eq!(
            GameData::format_id(sample_note_3, IdTy::CiteNote),
            Some(correct3)
        );
        assert_eq!(
            GameData::format_id(sample_note_4, IdTy::CiteNote),
            Some(correct4)
        );
    }

    const LOCALAPPDATA: &str = "C:\\Users\\matheus\\AppData\\Local";
    const HOME_MAC: &str = "/Users/matheus";
    const HOME_LINUX: &str = "/home/matheus";
    const STEAM_FOLDER: &str = "/home/matheus/.local/share/Steam";
    const XDG_CONFIG_HOME: &str = "/home/matheus/special/.config";
    const USER_ID: u64 = 69;

    #[test]
    #[serial]
    fn test_replace_path_abbrs() {
        // Safety: #[serial] attribute should ensure this does not run concurrently
        unsafe {
            env::set_var("%localappdata%", LOCALAPPDATA);
            env::set_var("HOME", HOME_MAC);
        }

        let windows_sample = r#"%LOCALAPPDATA%\UNDERTALE\"#;
        let mac_sample = "$HOME/Library/Application Support/com.tobyfox.undertale/";
        let linux_sample = "$HOME/.config/UNDERTALE/";

        let sample4 = "<Steam-folder>/userdata/<user-id>/391540/remote/";
        let sample5 = "$XDG_CONFIG_HOME/sample5";

        let windows_correct = LOCALAPPDATA.to_string() + r#"\UNDERTALE\"#;
        let mac_correct =
            HOME_MAC.to_string() + "/Library/Application Support/com.tobyfox.undertale/";
        let linux_correct = HOME_LINUX.to_string() + "/.config/UNDERTALE/";

        let correct4 =
            STEAM_FOLDER.to_string() + "/userdata/" + &USER_ID.to_string() + "/391540/remote/";
        let correct5_undefined = HOME_LINUX.to_string() + "/.config/sample5";
        let correct5_defined = XDG_CONFIG_HOME.to_string() + "/sample5";

        assert_eq!(
            GameData::replace_path_abbrs(
                windows_sample,
                Some("windows"),
                ExpansionParams {
                    install_dir: Path::new(STEAM_FOLDER),
                    user_id: USER_ID
                },
            ),
            Some(windows_correct.into())
        );
        assert_eq!(
            GameData::replace_path_abbrs(
                mac_sample,
                Some("macos"),
                ExpansionParams {
                    install_dir: Path::new(STEAM_FOLDER),
                    user_id: USER_ID
                },
            ),
            Some(mac_correct.into())
        );
        unsafe {
            env::set_var("HOME", HOME_LINUX);
        }
        assert_eq!(
            GameData::replace_path_abbrs(
                linux_sample,
                Some("linux"),
                ExpansionParams {
                    install_dir: Path::new(STEAM_FOLDER),
                    user_id: USER_ID
                },
            ),
            Some(linux_correct.into())
        );
        assert_eq!(
            GameData::replace_path_abbrs(
                sample4,
                Some("linux"),
                ExpansionParams {
                    install_dir: Path::new(STEAM_FOLDER),
                    user_id: USER_ID
                }
            ),
            Some(correct4.as_str().into())
        );

        unsafe {
            env::set_var("HOME", HOME_LINUX);
            env::set_var("XDG_CONFIG_HOME", XDG_CONFIG_HOME);
        }
        assert_eq!(
            GameData::replace_path_abbrs(
                sample5,
                Some("linux"),
                ExpansionParams {
                    install_dir: Path::new(STEAM_FOLDER),
                    user_id: USER_ID
                }
            ),
            Some(correct5_defined.into())
        );

        unsafe { env::remove_var("XDG_CONFIG_HOME") }
        assert_eq!(
            GameData::replace_path_abbrs(
                sample5,
                Some("linux"),
                ExpansionParams {
                    install_dir: Path::new(STEAM_FOLDER),
                    user_id: USER_ID
                }
            ),
            Some(correct5_undefined.into())
        );

        unsafe {
            env::remove_var("HOME");
        }
        assert_eq!(
            GameData::replace_path_abbrs(
                linux_sample,
                Some("linux"),
                ExpansionParams {
                    install_dir: Path::new(STEAM_FOLDER),
                    user_id: USER_ID
                }
            ),
            None
        );
    }
}