use std::{fs, path::PathBuf, sync::LazyLock, error::Error};

use serde::{de::DeserializeOwned, Serialize};

static CONFIG_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    [dirs::config_local_dir().expect("user’s config directory not found"), PathBuf::from("VittuSave")].iter().collect()
});

fn make_config_path(filename: &str) -> PathBuf {
    let mut path = CONFIG_DIR.clone();
    path.push(filename); 
    path.set_extension("toml");

    path
}

pub fn write_config<T: Serialize>(filename: &str, config: &T) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(CONFIG_DIR.as_path())?;

    let path = make_config_path(filename);

    let config_str = toml::to_string(config)?;
    fs::write(path, config_str)?;
    
    Ok(())
}

pub fn read_config<T: Serialize + DeserializeOwned + Default>(filename: &str) -> Result<T, Box<dyn Error>> {
    fs::create_dir_all(CONFIG_DIR.as_path())?;

    let path = make_config_path(filename);

    match fs::exists(&path) {
        Ok(true) => {
            let config_str = fs::read_to_string(&path)?;
            Ok(toml::from_str(&config_str)?)
        },
        Ok(false) => {
            let default_config = T::default();
            write_config(filename, &default_config)?;
            Ok(default_config)
        },
        Err(err) => Err(Box::new(err)),
    }
}