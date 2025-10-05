use std::{
    collections::HashMap,
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use mediawiki::ApiSync;
use scraper::{ElementRef, Html, Node, Selector};

use log::{debug, info, warn /*, trace*/};

use itertools::Itertools;

use crate::{
    pcgw::{
        PCGWError,
        api::{Location, LocationKind},
    },
    save_manager::GameId,
};

// TODO: Move methods to relevant struct or remove them
/// Looks up a Steam ID in the PCGW and returns the name of the page, if it exists
pub fn fetch_page_by_id(api: &ApiSync, steam_id: GameId) -> Result<String, PCGWError> {
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
        .ok_or(PCGWError::ParseError)?
        .first()
        .ok_or(PCGWError::NotFound)?["title"]["Page"]
        .as_str()
        .map(str::to_string)
        .ok_or(PCGWError::ParseError)
}

#[derive(Debug)]
enum HtmlIdTy {
    CiteRef,
    CiteNote,
}

/// This is function is nescessary because the numbers that prefix IDs are different when requesting for only a specific section. I don't know why.
fn format_id(id: &str, ty: HtmlIdTy) -> Option<&str> {
    let (start, separator) = match ty {
        HtmlIdTy::CiteRef => ("cite_ref-", '_'),
        HtmlIdTy::CiteNote => ("cite_note-", '-'),
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

// Environment variable placeholders
static ENV_VARS: LazyLock<HashMap<&str, &[&str]>> = LazyLock::new(|| {
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
static ENV_VAR_DEFAULTS: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| {
    HashMap::from([
        ("$XDG_DATA_HOME", "$HOME/.local/share"),
        ("$XDG_CONFIG_HOME", "$HOME/.config"),
    ])
});

// Other Placeholders
const STEAM_FOLDER: &str = "<Steam-folder>";
const USER_ID: &str = "<user-id>";

#[derive(Debug, Clone)]
pub struct ExpansionParams<'a> {
    pub install_dir: &'a Path,
    pub user_id: u64,
}
/// Should be updated from <https://www.pcgamingwiki.com/wiki/Glossary:Game_data>
/// Returns None if an undefined abbreviation in the path
/// Supports non-unicode encoded env variable values
/// Finds first match only
// TODO: Fix this super and others
pub fn replace_path_abbrs(
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
                replace_path_abbrs(default_val, Some(os), params.clone())
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

fn fetch_section_id(api: &ApiSync, page: &str, section_line: &str) -> Result<String, PCGWError> {
    let params = api.params_into(&[("action", "parse"), ("page", page), ("prop", "sections")]);

    let res = api.get_query_api_json_all(&params)?;

    res["parse"]["sections"]
        .as_array()
        .ok_or(PCGWError::NotFound)?
        .iter()
        .find_map(|section_val| {
            section_val["line"]
                .as_str()
                .filter(|line| section_line == *line)
                .and(section_val["index"].as_str())
        })
        .map(str::to_string)
        .ok_or(PCGWError::ParseError)
}
fn section_html(api: &ApiSync, page: &str, section_line: &str) -> Result<Html, PCGWError> {
    let section_id = fetch_section_id(api, page, section_line)?;

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
        .ok_or(PCGWError::NotFound)
}

fn page_html(api: &ApiSync, page: &str) -> Result<Html, PCGWError> {
    let params = api.params_into(&[("action", "parse"), ("page", page), ("prop", "text")]);

    let res = api.get_query_api_json_all(&params).unwrap();

    res["parse"]["text"]["*"]
        .as_str()
        .map(Html::parse_fragment)
        .ok_or(PCGWError::NotFound)
}

fn extract_raw_location(el: ElementRef, notes: &HashMap<&str, String>) -> Location {
    let mut note = None;
    let path_str = el
        .children()
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
                    .and_then(|id| format_id(id, HtmlIdTy::CiteRef))
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

fn parse_data_table(
    section_html: &Html,
    notes: HashMap<&str, String>,
) -> Result<HashMap<LocationKind, Vec<Location>>, PCGWError> {
    let parser_output_selector =
        Selector::parse(".mw-parser-output").expect("str should be a valid selector");
    let header_selector = Selector::parse(".mw-headline").expect("str should be a valid selector");
    let mut parser_output_iter = section_html
        .select(&parser_output_selector)
        .exactly_one()
        .map_err(|_| PCGWError::NotFound)?
        .child_elements();
    let mut save_table: Result<ElementRef, PCGWError> = Err(PCGWError::NotFound);
    while let Some(el) = parser_output_iter.next() {
        if el.value().name() == "h3"
            && el
                .select(&header_selector)
                .exactly_one()
                .map_err(|_| PCGWError::ParseError)?
                .text()
                .next()
                .ok_or(PCGWError::ParseError)?
                == "Save game data location"
        {
            save_table = Ok(parser_output_iter.next().ok_or(PCGWError::ParseError)?);
            break;
        }
    }
    // FIXME: extra notes are not avaliable in section thing
    // TODO: thumbs down/Thumbs up display, convert HTML with library
    // let extra_notes = parser_output_iter
    //     .map_while(|el| {
    //         (el.value().name() == "dl").then_some(
    //             el.child_elements()
    //                 .exactly_one()
    //                 .map_err(|_| PCGWError::ParseError),
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
    let location_selector =
        Selector::parse(".table-gamedata-body-location").expect("str should be a valid selector");
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
                .map(|location_el| extract_raw_location(location_el, &notes))
                .collect();

            Some((location_kind, locations))
        })
        .collect();

    Ok(locations)
}

pub(super) fn get_location_data(
    api: &ApiSync,
    steam_id: GameId,
) -> Result<HashMap<LocationKind, Vec<Location>>, PCGWError> {
    // TODO: different errors for different steps
    let page = fetch_page_by_id(api, steam_id)?;
    let section_html = section_html(api, &page, "Game data")?;
    let page_html = page_html(api, &page)?;
    let notes = extract_notes(&page_html)?;

    parse_data_table(&section_html, notes)
}

fn extract_notes(page_html: &Html) -> Result<HashMap<&str, String>, PCGWError> {
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
            .map(|el| -> Result<(&str, String), PCGWError> {
                let reference_text_el = el
                    .select(&reference_text_selector)
                    .exactly_one()
                    .map_err(|_| PCGWError::ParseError)?;
                let reference_text_html = reference_text_el.html();
                Ok((
                    format_id(
                        el.value().id().ok_or(PCGWError::ParseError)?,
                        HtmlIdTy::CiteNote,
                    )
                    .ok_or(PCGWError::ParseError)?,
                    html2text::from_read(reference_text_html.as_bytes(), reference_text_html.len())
                        .inspect_err(|err| println!("error is {err}"))?,
                ))
            })
            .collect::<Result<HashMap<_, _>, _>>()?,
        None => HashMap::default(),
    };

    Ok(notes)
}

// TODO: Separate tests
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

        assert_eq!(format_id(sample_ref_1, HtmlIdTy::CiteRef), Some(correct1));
        assert_eq!(format_id(sample_ref_2, HtmlIdTy::CiteRef), Some(correct2));
        assert_eq!(format_id(sample_ref_3, HtmlIdTy::CiteRef), Some(correct3));
        assert_eq!(format_id(sample_ref_4, HtmlIdTy::CiteRef), Some(correct4));
        assert_eq!(format_id(sample_note_1, HtmlIdTy::CiteNote), Some(correct1));
        assert_eq!(format_id(sample_note_2, HtmlIdTy::CiteNote), Some(correct2));
        assert_eq!(format_id(sample_note_3, HtmlIdTy::CiteNote), Some(correct3));
        assert_eq!(format_id(sample_note_4, HtmlIdTy::CiteNote), Some(correct4));
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
            replace_path_abbrs(
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
            replace_path_abbrs(
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
            replace_path_abbrs(
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
            replace_path_abbrs(
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
            replace_path_abbrs(
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
            replace_path_abbrs(
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
            replace_path_abbrs(
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
