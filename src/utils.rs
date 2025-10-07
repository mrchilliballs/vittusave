use anyhow::Result;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    cell::{Ref, RefCell},
    fs, io,
    path::{Path, PathBuf},
};
use uuid::Uuid;

use crate::consts::{CACHE_DIR, DATA_DIR, FILE_EXTENSION};

// TODO: Move methods to relevant struct or remove them
fn build_data_path(filename: impl AsRef<Path>) -> PathBuf {
    let mut path = DATA_DIR.clone();
    path.push(filename);
    path.set_extension(FILE_EXTENSION);

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
pub trait CachedState: private::Sealed {}
pub mod states {
    use serde::{Deserialize, Serialize, de::DeserializeOwned};

    use super::CachedState;
    #[derive(Debug, Serialize, Deserialize)]
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    pub struct Resolved<T: Serialize + Default>(pub T);
    #[derive(Debug, Serialize, Deserialize)]
    pub struct Unresolved;

    impl<T: Serialize + DeserializeOwned + Default> CachedState for Resolved<T> {}
    impl CachedState for Unresolved {}
}
mod private {
    use serde::{Serialize, de::DeserializeOwned};

    use super::states::*;

    pub trait Sealed {}

    impl<T: Serialize + DeserializeOwned + Default> Sealed for Resolved<T> {}
    impl Sealed for Unresolved {}
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Cached<S: CachedState> {
    state: S,
    path: PathBuf,
}

impl Default for Cached<states::Unresolved> {
    fn default() -> Self {
        let path = {
            let mut path = CACHE_DIR.join(Path::new(&Uuid::new_v4().to_string()));
            path.set_extension(FILE_EXTENSION);
            path
        };
        Self {
            state: states::Unresolved,
            path,
        }
    }
}
impl Cached<states::Unresolved> {
    /// Reads or creates cache with default contents.
    pub fn read<T: Serialize + DeserializeOwned + Default>(
        self,
    ) -> Result<Cached<states::Resolved<T>>, anyhow::Error> {
        // TODO: use functions from mod utils
        let parent = self.path.parent().expect("cache file should have a parent");
        if !fs::exists(parent)? {
            fs::create_dir_all(parent)?;
        }
        let value = if !fs::exists(&self.path)? {
            let value = T::default();
            fs::write(&self.path, toml::to_string(&T::default())?)?;
            value
        } else {
            toml::from_slice(&fs::read(&self.path)?)?
        };
        Ok(Cached {
            state: states::Resolved(value),
            path: self.path,
        })
    }
}

impl<T: Serialize + DeserializeOwned + Default> Cached<states::Resolved<T>> {
    pub fn get(&self) -> &T {
        &self.state.0
    }
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.state.0
    }
}

// TODO: tests
