use std::{
    collections::HashMap,
    fs,
    hash::Hash,
    io,
    path::{Path, PathBuf},
};

use mediawiki::{MediaWikiError, api_sync::ApiSync};
use thiserror::Error;

use crate::{
    GameId,
    pcgw::{self, utils::ExpansionParams},
};

// TODO: rename this error type
#[derive(Debug, Error)]
pub enum PCGWError {
    #[error("failed to fetch data from MediaWiki API")]
    MediaWikiError(#[from] MediaWikiError),
    #[error("parse error")]
    ParseError,
    #[error("error reading or rendering note HTML")]
    NoteError(#[from] html2text::Error),
    #[error("no data returned by the server")]
    NotFound,
}

#[derive(Debug, Error)]
pub enum LocationError {
    #[error("built path does not exist")]
    InvalidPath(#[from] io::Error),
    #[error("undefined abbreviation in path")]
    UndefinedAbbr,
}

/// Pre-processed location
#[derive(Debug, Default)]
pub struct Location {
    path: Option<PathBuf>,
    path_str: String,
    note: Option<String>,
}
impl Location {
    pub fn new(path_str: String, note: Option<String>) -> Self {
        Self {
            path_str,
            note,
            ..Default::default()
        }
    }
    #[inline]
    pub fn path_str(&self) -> &str {
        &self.path_str
    }
    #[inline]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
    pub fn expand_path(&mut self, install_dir: &Path, user_id: u64) -> Result<(), LocationError> {
        self.path.replace(
            pcgw::utils::replace_path_abbrs(
                &self.path_str,
                None,
                ExpansionParams {
                    install_dir,
                    user_id,
                },
            )
            .ok_or(LocationError::UndefinedAbbr)
            .map_or_else(Err, |path| {
                fs::exists(&path)
                    .map_err(LocationError::InvalidPath)
                    .map(|_| path)
            })?,
        );
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum LocationKind {
    OS(String),
    Steam,
}

#[derive(Debug, Default)]
pub struct PCGWSaveMeta {
    locations: HashMap<LocationKind, Vec<Location>>,
    extra_notes: Vec<String>,
}

// TODO: Can user ID, steam path, etc. be turned optional somewhow?
impl PCGWSaveMeta {
    // TODO: return Self back in error
    pub fn build(api: &ApiSync, id: GameId) -> Result<Self, PCGWError> {
        Ok(PCGWSaveMeta {
            locations: pcgw::utils::get_location_data(api, id)?,
            extra_notes: Vec::new(),
        })
    }
    pub fn get_locations(&mut self, kind: LocationKind) -> &mut [Location] {
        self.locations
            .get_mut(&kind)
            .map(|vec| vec.as_mut_slice())
            .unwrap_or(&mut [])
    }
}
