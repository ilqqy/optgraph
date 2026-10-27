//! Serde types of graph.json (schema/graph.schema.json, docs/schema.md).
//!
//! Field names follow the schema. Unknown fields are ignored, so a newer
//! producer with extra fields still loads; missing required fields fail.

use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Deserialize)]
pub struct Graph {
    pub meta: Meta,
    pub modules: Vec<Module>,
    pub options: Vec<OptionEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub host: Option<String>,
    pub nixpkgs_rev: Option<String>,
    pub nixpkgs_version: String,
    pub generated_at: Option<String>,
    pub tool_version: String,
    pub scope: Scope,
    pub attribution: Attribution,
    pub complete: bool,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    User,
    All,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::User => "user",
            Scope::All => "all",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Attribution {
    Aligned,
    FileBased,
}

impl Attribution {
    pub fn as_str(self) -> &'static str {
        match self {
            Attribution::Aligned => "aligned",
            Attribution::FileBased => "file-based",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Warning {
    /// One of the codes in the schema; kept as text so a new code still loads.
    pub code: String,
    pub message: String,
    pub subject: Option<String>,
}

/// `user`, `user-inline`, `nixpkgs`, `unknown` or `input:<name>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Origin {
    User,
    UserInline,
    Nixpkgs,
    Unknown,
    Input(String),
}

impl Origin {
    pub fn parse(s: &str) -> Origin {
        match s {
            "user" => Origin::User,
            "user-inline" => Origin::UserInline,
            "nixpkgs" => Origin::Nixpkgs,
            _ => match s.strip_prefix("input:") {
                Some(name) if !name.is_empty() => Origin::Input(name.to_string()),
                _ => Origin::Unknown,
            },
        }
    }

    /// The text as in graph.json: `input:shared`.
    pub fn label(&self) -> String {
        match self {
            Origin::User => "user".into(),
            Origin::UserInline => "user-inline".into(),
            Origin::Nixpkgs => "nixpkgs".into(),
            Origin::Unknown => "unknown".into(),
            Origin::Input(n) => format!("input:{n}"),
        }
    }

    /// Colour class: `input:foo/bar` -> `Input`.
    pub fn class(&self) -> OriginClass {
        match self {
            Origin::User => OriginClass::User,
            Origin::UserInline => OriginClass::UserInline,
            Origin::Nixpkgs => OriginClass::Nixpkgs,
            Origin::Unknown => OriginClass::Unknown,
            Origin::Input(_) => OriginClass::Input,
        }
    }

    /// Everything but nixpkgs counts as the user's (the scope filter's rule).
    pub fn is_userish(&self) -> bool {
        !matches!(self, Origin::Nixpkgs)
    }
}

impl<'de> Deserialize<'de> for Origin {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(Origin::parse(&s))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OriginClass {
    User,
    UserInline,
    Input,
    Nixpkgs,
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Module {
    pub id: String,
    pub file: String,
    pub imports: Vec<String>,
    pub origin: Origin,
    pub disabled: bool,
    pub modules_index: Option<u64>,
    pub position: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionEntry {
    pub path: String,
    pub loc: Vec<String>,
    pub declared_in: Vec<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub definitions: Vec<Definition>,
    pub winners: Vec<usize>,
    pub highest_prio: Option<i64>,
    pub omitted: Omitted,
    pub error: Option<String>,
}

impl OptionEntry {
    /// nixpkgs definitions counted instead of listed.
    pub fn omitted_total(&self) -> u64 {
        self.omitted.nixpkgs_active + self.omitted.nixpkgs_inactive
    }

    /// Winners counted in `omitted` instead of listed. 0 with a single
    /// winner (it is always listed) and in graphs from before the field.
    pub fn hidden_winners(&self) -> u64 {
        self.omitted.nixpkgs_winners
    }

    /// Every winner: the listed ones and the omitted nixpkgs ones.
    pub fn winner_count(&self) -> u64 {
        self.winners.len() as u64 + self.omitted.nixpkgs_winners
    }

    pub fn is_winner(&self, def: usize) -> bool {
        self.winners.contains(&def)
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Omitted {
    pub nixpkgs_active: u64,
    pub nixpkgs_inactive: u64,
    /// How many of `nixpkgs_active` win (at `highest_prio`); not counted twice.
    #[serde(default)]
    pub nixpkgs_winners: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DefKind {
    Definition,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Condition {
    #[serde(rename = "mkIf-false")]
    MkIfFalse,
    #[serde(rename = "mkIf-error")]
    MkIfError,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Definition {
    pub kind: DefKind,
    /// Absent when it equals the module's file (and `null` for a default
    /// without a declaration): see [`crate::Model::def_file`].
    #[serde(default)]
    pub file: Option<String>,
    pub module: Option<String>,
    pub priority: Option<i64>,
    pub active: bool,
    pub condition: Option<Condition>,
    pub value_preview: Option<String>,
}

impl Definition {
    pub fn is_default(&self) -> bool {
        self.kind == DefKind::Default
    }
}
