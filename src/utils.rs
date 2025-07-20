use console::Term;
use std::{fs, io, path::Path};

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

pub fn remove_dir_contents(path: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&path)?;
    fs::remove_dir_all(&path)?;
    fs::create_dir(&path)
}

// https://stackoverflow.com/a/65192210
pub fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&src)?;
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
