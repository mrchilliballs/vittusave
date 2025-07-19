use console::Term;
use std::io;

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

pub fn copy_dir_entries(from: &str, to: &str) {
    
}