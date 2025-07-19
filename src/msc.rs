use std::{collections::BTreeMap, error::Error, ops::{Deref, DerefMut}, path::Path, rc::Rc};

use crate::{config::{read_config, write_config}, SaveMetadata, SaveSwapper, SaveSwapperConfig};

#[derive(Debug)]
pub struct MySummerCarSaveSwapper {
    // TODO: Ask user to save or add config pane
    config: SaveSwapperConfig,
}

impl MySummerCarSaveSwapper {
    const DISPLAY_NAME: &str = "My Summer Car";
    const CONFIG_FILENAME: &str = "My_Summer_Car";

    pub fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            config: read_config(Self::CONFIG_FILENAME)?,
        })
    }
}

impl Deref for MySummerCarSaveSwapper {
    type Target = BTreeMap<Rc<Path>, SaveMetadata>;

    fn deref(&self) -> &Self::Target {
        &self.config.saves
    }
}
impl DerefMut for MySummerCarSaveSwapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.config.saves
    }
}
impl Drop for MySummerCarSaveSwapper {
    fn drop(&mut self) {
        self.save().expect(&format!("failed to save \"{}\"'s configuration at drop", self.display_name()));
    }
}

impl SaveSwapper for MySummerCarSaveSwapper {
    fn display_name(&self) -> &'static str {
        Self::DISPLAY_NAME
    }
    fn config_filename(&self) -> &'static str {
        Self::CONFIG_FILENAME
    }
    fn save(&self) -> Result<(), Box<dyn Error>> {
        write_config(Self::CONFIG_FILENAME, &self.config)?;
        Ok(())
    }
}
