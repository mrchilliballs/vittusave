mod api;
mod utils;

pub use api::PCGWSaveMeta;
pub use utils::fetch_page_by_id;

use mediawiki::MediaWikiError;
use thiserror::Error;

// TODO: look into Anyhow's `Error` and `Context` instead of this
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
