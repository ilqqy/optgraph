//! Loading graph.json from a file, stdin (`-`) or a string.

use std::fmt;
use std::io::Read;
use std::path::Path;

use crate::model::Graph;

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    /// Not JSON, or not a graph.json (a field missing or of the wrong type).
    Json(serde_json::Error),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "cannot read the file: {e}"),
            LoadError::Json(e) if e.is_syntax() || e.is_eof() => write!(f, "not valid JSON: {e}"),
            LoadError::Json(e) => write!(
                f,
                "not an optgraph graph.json (expected meta, modules and options): {e}"
            ),
        }
    }
}

impl std::error::Error for LoadError {}

pub fn from_str(s: &str) -> Result<Graph, LoadError> {
    serde_json::from_str(s).map_err(LoadError::Json)
}

pub fn from_reader(r: impl Read) -> Result<Graph, LoadError> {
    let mut s = String::new();
    std::io::BufReader::new(r)
        .read_to_string(&mut s)
        .map_err(LoadError::Io)?;
    from_str(&s)
}

/// A file, or stdin for `-`.
pub fn from_path(path: &Path) -> Result<Graph, LoadError> {
    if path.as_os_str() == "-" {
        return from_reader(std::io::stdin().lock());
    }
    from_str(&std::fs::read_to_string(path).map_err(LoadError::Io)?)
}
