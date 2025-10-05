use anyhow::Result;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::consts::DATA_DIR;

// TODO: Move methods to relevant struct or remove them
fn build_data_path(filename: impl AsRef<Path>) -> PathBuf {
    let mut path = DATA_DIR.clone();
    path.push(filename);
    path.set_extension("toml");

    path
}

/// Write data to `crate::consts::DATA_DIR`.
pub fn write_data<T: Serialize>(filename: impl AsRef<Path>, data: &T) -> Result<()> {
    fs::create_dir_all(DATA_DIR.as_path())?;

    let path = build_data_path(filename);

    let config_str = toml::to_string(data)?;
    fs::write(path, config_str)?;

    Ok(())
}

/// Read file from `crate::consts::DATA_DIR`.
pub fn read_data<T: DeserializeOwned>(filename: impl AsRef<Path>) -> Result<Option<T>> {
    fs::create_dir_all(DATA_DIR.as_path())?;

    let path = build_data_path(&filename);

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

// TODO: tests
