// TODO: Do some sort of integrity check before loading saves

mod config;
mod msc;
mod utils;

use console::{Term, style};
use dialoguer::{Confirm, Input, Select};
use msc::MySummerCarSaveSwapper;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    error::Error,
    fmt::{self, Display},
    fs,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct SaveMetadata {
    pub label: String,
    pub loaded: bool,
    // TODO: timestamp
}

#[derive(Serialize, Deserialize, Debug)]
struct SaveSwapperConfig {
    saves: BTreeMap<Rc<Path>, SaveMetadata>,
    path: Option<PathBuf>,
}
impl SaveSwapperConfig {
    fn new(path: Option<PathBuf>) -> Self {
        SaveSwapperConfig {
            saves: BTreeMap::new(),
            path,
        }
    }
}

// TODO: game-specific settings
trait SaveSwapper: fmt::Debug + Deref<Target = BTreeMap<Rc<Path>, SaveMetadata>> + DerefMut {
    fn display_name(&self) -> &'static str;
    fn config_filename(&self) -> &'static str;
    fn default_dir(&self) -> Option<&Path>;
    fn get_dir(&self) -> Option<&Path>;
    fn set_dir(&mut self, dir: PathBuf);
    fn save(&self) -> Result<(), Box<dyn Error>>;
}

fn run_action(
    term: &Term,
    save_swapper: &mut Box<dyn SaveSwapper>,
    action: Action,
) -> Result<(), Box<dyn Error>> {
    match action {
        Action::Toggle(path, _) => {
            let metadata = save_swapper
                .get_mut(&path)
                .expect("tried to load/unload a non-existant save entry");
            metadata.loaded = !metadata.loaded;

            // TODO: Remove hard-coded save path
            // src = origin
            // dst = game
            // TODO: Error handling
            if metadata.loaded {
                dbg!(&path);
                dbg!(save_swapper.get_dir());

                // TODO: Swap directories
                utils::remove_dir_contents(save_swapper.get_dir().unwrap())?;
                utils::copy_dir_all(&path, save_swapper.get_dir().unwrap())?;
            } else {
                utils::remove_dir_contents(&path)?;
                utils::copy_dir_all(save_swapper.get_dir().unwrap(), path)?;
                utils::remove_dir_contents(save_swapper.get_dir().unwrap())?;
            }
        }
        Action::Delete(path) => {
            save_swapper.remove(&path);
            fs::remove_dir_all(&path)?;
            save_swapper.save()?;
        }
        Action::Create(copy_src) => {
            utils::clear_screen(term, Some(save_swapper.display_name()), None)?;
            let label: String = Input::new()
                .with_prompt("Enter save label")
                .interact_text_on(term)
                .unwrap();

            let mut path =
                PathBuf::from(dirs::document_dir().expect("Cannot find document directory"));
            path.push("VittuSave");
            path.push(&label);
            let path = Rc::from(path);

            save_swapper.insert(
                Rc::clone(&path),
                SaveMetadata {
                    label,
                    loaded: copy_src,
                },
            );
            if copy_src {
                if let Some(Ok(_)) = fs::read_dir(save_swapper.get_dir().unwrap())?.next() {
                    utils::copy_dir_all(save_swapper.get_dir().unwrap(), path)?;
                }
            }
            save_swapper.save()?;
        }
    };
    Ok(())
}

#[derive(Debug, Clone)]
enum Action {
    Toggle(Rc<Path>, bool),
    Delete(Rc<Path>),
    Create(bool),
}
impl Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Action::Toggle(_, loaded) => {
                if *loaded {
                    write!(f, "Unload")
                } else {
                    write!(f, "Load")
                }
            }
            Action::Delete(_) => write!(f, "{}", style("Delete").red()),
            Action::Create(_) => write!(f, "{}", style("Create").green()),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let term = Term::stdout();

    let mut save_swappers: Vec<Box<dyn SaveSwapper>> =
        vec![Box::new(MySummerCarSaveSwapper::build()?)];

    loop {
        utils::clear_screen(&term, None, None)?;
        let items: Vec<&'static str> = save_swappers
            .iter()
            .map(|e| e.display_name())
            .chain(["Settings"])
            .collect();
        let Some(selection) = Select::new()
            .with_prompt("Menu")
            .items(&items)
            .default(0)
            .interact_on_opt(&term)
            .unwrap()
        else {
            break;
        };
        if items[selection] == "Settings" {
            utils::clear_screen(&term, None, None)?;
            let items = ["Dummy 1", "Dummy 2"];
            let Some(selection) = Select::new()
                .with_prompt("Settings")
                .items(&items)
                .default(0)
                .interact_on_opt(&term)
                .unwrap()
            else {
                continue;
            };
            // Go to setting page...
        }
        let save_swapper = &mut save_swappers[selection];

        loop {
            utils::clear_screen(&term, Some(save_swapper.display_name()), None)?;
            if save_swapper.is_empty() {
                let confirmation = Confirm::new()
                    .with_prompt("No saves found. Register the current one?")
                    .interact_on(&term)
                    .unwrap();
                if confirmation {
                    run_action(&term, save_swapper, Action::Create(true))?;
                } else {
                    break;
                }
                continue;
            }
            let (keys, mut items): (Vec<&Rc<Path>>, Vec<String>) = save_swapper
                .iter()
                .map(|e| {
                    let loaded_str = if e.1.loaded { "X" } else { " " };
                    (e.0, String::from("[") + loaded_str + "] " + &e.1.label)
                })
                .collect();
            items.push(String::from("New"));

            let Some(selection) = Select::new()
                .with_prompt("Select a save")
                .items(&items)
                .default(0)
                .interact_on_opt(&term)
                .unwrap()
            else {
                break;
            };
            if items[selection] == "New" {
                run_action(&term, save_swapper, Action::Create(false))?;
                continue;
            }
            let save_key = Rc::clone(keys[selection]);
            let save_metadata = save_swapper.get(&save_key).unwrap();

            utils::clear_screen(
                &term,
                Some(save_swapper.display_name()),
                Some(&save_metadata.label),
            )?;
            let actions = [
                Action::Toggle(Rc::clone(&save_key), save_metadata.loaded),
                Action::Delete(save_key),
            ];
            let items: Vec<String> = actions.iter().map(|action| action.to_string()).collect();
            let Some(selection) = Select::new()
                .with_prompt("Select an action")
                .items(&items)
                .default(0)
                .interact_on_opt(&term)
                .unwrap()
            else {
                continue;
            };
            let action = actions[selection].clone();
            run_action(&term, save_swapper, action)?;
        }
    }
    Ok(())
}
