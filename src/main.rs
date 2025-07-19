// TODO: Do some sort of integrity check before loading saves

use console::{Term, style};
use dialoguer::{Confirm, Input, Select};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap, error::Error, fmt::{self, Display}, fs, io, ops::{Deref, DerefMut}, path::{Path, PathBuf}, rc::Rc
};

const SAVE_PATH: &str = "/home/matheus/Documents/VittuSave/";

#[derive(Serialize, Deserialize, Debug)]
struct SaveMetadata {
    pub label: String,
    pub loaded: bool,
    // TODO: timestamp
}
// impl Display for SaveMetadata {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         writeln!(f, "NAME:\t{}", self.label)?;
//         write!(f, "LOADED:\t")?;
//         if self.loaded {
//             writeln!(f, "YES")
//         } else {
//             writeln!(f, "NO")
//         }
//     }
// }

trait SaveSwapper: fmt::Debug + Deref<Target = BTreeMap<Rc<Path>, SaveMetadata>> + DerefMut {
    fn name(&self) -> &'static str;
    fn path(&self) -> &'static str;
    fn save(&self) -> io::Result<()>;
}

#[derive(Serialize, Deserialize, Debug)]
struct MySummerCarSaves {
    // TODO: Ask user to save or add config pane
    saves: BTreeMap<Rc<Path>, SaveMetadata>,
}

impl MySummerCarSaves {
    const FILE_PATH: &str = "/home/matheus/Documents/VittuSave/msc.toml";
    const FOLDER_PATH: &str = "/home/matheus/Documents/VittuSave";

    fn new() -> Result<Self, io::Error> {
        let default = Self { saves: BTreeMap::new() };
        if !fs::exists(Self::FILE_PATH)? {
            fs::create_dir_all(Self::FOLDER_PATH)?;
            fs::write(Self::FILE_PATH, toml::to_string(&default).unwrap())?;
        }
        let saves: Self = toml::from_str(&fs::read_to_string(Self::FILE_PATH)?).unwrap();
        let fuck = Box::leak(Box::new(default));
        Ok(saves)
    }
}

impl Deref for MySummerCarSaves {
    type Target = BTreeMap<Rc<Path>, SaveMetadata>;

    fn deref(&self) -> &Self::Target {
        &self.saves
    }
}
impl DerefMut for MySummerCarSaves {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.saves
    }
}
impl Drop for MySummerCarSaves {
    fn drop(&mut self) {
        self.save().unwrap();
    }
}

impl SaveSwapper for MySummerCarSaves {
    fn name(&self) -> &'static str {
        "My Summer Car"
    }
    fn path(&self) -> &'static str {
        "MySummerCar"
    }
    fn save(&self) -> io::Result<()> {
        fs::write(Self::FILE_PATH, toml::to_string(self).unwrap())
    }
}

fn clear_screen(term: &Term, game: Option<&str>, save: Option<&str>) -> io::Result<()> {
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

fn run_action(term: &Term, save_swapper: &mut Box<dyn SaveSwapper>, action: Action) -> io::Result<()> {
    match action {
        Action::Toggle(path, _) => todo!(),
        Action::Delete(path) => { save_swapper.remove(&path); save_swapper.save(); },
        Action::Create(loaded)=> {
            clear_screen(term, Some(save_swapper.name()), None)?;
            let label: String = Input::new()
                .with_prompt("Enter save label")
                .interact_text_on(term)
                .unwrap();
            clear_screen(term, Some(save_swapper.name()), Some(&label))?;
            let path: String = Input::new()
                .with_prompt("Enter the file path")
                .with_initial_text(String::from(SAVE_PATH) + save_swapper.path() + "/")
                .interact_text_on(term)
                .unwrap();
            let path = PathBuf::from(path);

            save_swapper.insert(Rc::from(path), SaveMetadata { label, loaded });
            save_swapper.save()?;
        },
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

    let mut save_swappers: Vec<Box<dyn SaveSwapper>> = vec![Box::new(MySummerCarSaves::new()?)];
    // save_swappers[0].insert(
    //     Rc::from(PathBuf::from(String::from(SAVE_PATH) + "MySummerCar/Save1")),
    //     SaveMetadata {
    //         label: "Save 1".to_string(),
    //         loaded: true,
    //     },
    // );
    // save_swappers[0].insert(
    //     Rc::from(PathBuf::from(String::from(SAVE_PATH) + "MySummerCar/Save2")),
    //     SaveMetadata {
    //         label: "Save 2".to_string(),
    //         loaded: false,
    //     },
    // );
    // dbg!(&save_swappers[0]);

    loop {
        clear_screen(&term, None, None)?;
        let items: Vec<&'static str> = save_swappers.iter().map(|e| e.name()).collect();
        let Some(selection) = Select::new()
            .with_prompt("Select a game")
            .items(&items)
            .default(0)
            .interact_on_opt(&term)
            .unwrap()
        else {
            break;
        };
        let save_swapper = &mut save_swappers[selection];

        loop {
            clear_screen(&term, Some(save_swapper.name()), None)?;
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
            let (keys, items): (Vec<&Rc<Path>>, Vec<String>) = save_swapper
                .iter()
                .map(|e| {
                    let loaded_str = if e.1.loaded { "X" } else { " " };
                    (
                        e.0,
                        String::from("[") + loaded_str + "] " + &e.1.label,
                    )
                })
                .collect();
                
            let Some(selection) = Select::new()
                .with_prompt("Select a save")
                .items(&items)
                .default(0)
                .interact_on_opt(&term)
                .unwrap()
            else {
                break;
            };
            let save_key = Rc::clone(keys[selection]);
            let save_metadata = save_swapper.get(&save_key).unwrap();

            clear_screen(
                &term,
                Some(save_swapper.name()),
                Some(&save_metadata.label),
            )?;
            let actions = [Action::Toggle(Rc::clone(&save_key), save_metadata.loaded), Action::Delete(save_key)];
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
