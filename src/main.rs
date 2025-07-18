// TODO: Do some sort of integrity check before loading saves

use console::{Term, style};
use dialoguer::Select;
use std::{
    collections::BTreeMap,
    error::Error,
    fmt::{self, Display},
    io,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

const SAVE_PATH: &'static str = "~/Documents/VittuSave";

#[derive(Debug)]
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

trait SaveSwapper: fmt::Debug + Deref<Target = BTreeMap<PathBuf, SaveMetadata>> + DerefMut {
    fn name(&self) -> &'static str;
    fn run_action(&self, action: &Action);
}

#[derive(Debug)]
struct MySummerCarSaves {
    saves: BTreeMap<PathBuf, SaveMetadata>,
}

impl MySummerCarSaves {
    fn new() -> Self {
        MySummerCarSaves {
            saves: BTreeMap::new(),
        }
    }
}

impl Deref for MySummerCarSaves {
    type Target = BTreeMap<PathBuf, SaveMetadata>;

    fn deref(&self) -> &Self::Target {
        &self.saves
    }
}
impl DerefMut for MySummerCarSaves {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.saves
    }
}

impl SaveSwapper for MySummerCarSaves {
    fn name(&self) -> &'static str {
        "My Summer car"
    }
    fn run_action(&self, action: &Action) {
        todo!()
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

#[derive(Debug, Clone)]
enum Action {
    Toggle(bool),
    Delete,
}
impl Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Action::Toggle(loaded) => {
                if *loaded {
                    write!(f, "Unload")
                } else {
                    write!(f, "Load")
                }
            }
            Action::Delete => write!(f, "{}", style("Delete").red()),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let term = Term::stdout();

    let mut save_swappers: Vec<Box<dyn SaveSwapper>> = vec![Box::new(MySummerCarSaves::new())];
    save_swappers[0].insert(
        PathBuf::from(String::from(SAVE_PATH) + "/Save1"),
        SaveMetadata {
            label: "Save 1".to_string(),
            loaded: true,
        },
    );
    save_swappers[0].insert(
        PathBuf::from(String::from(SAVE_PATH) + "/Save2"),
        SaveMetadata {
            label: "Save 2".to_string(),
            loaded: false,
        },
    );
    dbg!(&save_swappers[0]);

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
        let save_swapper = &save_swappers[selection];

        loop {
            clear_screen(&term, Some(&save_swapper.name()), None)?;
            let (keys, items): (Vec<&Path>, Vec<String>) = save_swapper
                .iter()
                .map(|e| {
                    let loaded_str = if e.1.loaded { "X" } else { " " };
                    (
                        e.0.as_path(),
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
            let save_metadata = &save_swapper[keys[selection]];

            clear_screen(
                &term,
                Some(&save_swapper.name()),
                Some(&save_metadata.label),
            )?;
            let actions = vec![Action::Toggle(save_metadata.loaded), Action::Delete];
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
            save_swapper.run_action(&action);
        }
    }
    Ok(())
}
