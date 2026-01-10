use crate::empty_query::EmptyQueryMode;
use crate::models::DesktopEntryOut;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "kebab-case")]
pub enum Request {
    Search {
        roots: Vec<String>,
        query: String,
        limit: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        empty_mode: Option<EmptyQueryMode>,

        /// If true, filter out entries whose TryExec is present but not available.
        #[serde(default)]
        respect_try_exec: bool,
    },
    /// Build (or ensure) the in-memory index for the given roots.
    Warmup {
        roots: Vec<String>,

        /// If true, filter out entries whose TryExec is present but not available.
        #[serde(default)]
        respect_try_exec: bool,
    },
    List {
        roots: Vec<String>,

        /// If true, filter out entries whose TryExec is present but not available.
        #[serde(default)]
        respect_try_exec: bool,
    },
    Launch {
        roots: Vec<String>,
        desktop_id: String,
        action: Option<String>,

        /// If true, filter out entries whose TryExec is present but not available.
        #[serde(default)]
        respect_try_exec: bool,
    },
    Status,

    Shutdown,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Response {
    Ok,
    Error { message: String },
    Entries { entries: Vec<DesktopEntryOut> },
    Status { has_index_count: usize },
}
