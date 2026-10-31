//! optgraph's graph.json as a library: serde types (`model`), loading
//! (`load`), the derived data of the viewer (`analysis`), formatting
//! helpers (`text`). No GUI dependency.

pub mod analysis;
pub mod instances;
pub mod layout;
pub mod load;
pub mod model;
pub mod search;
pub mod text;

pub use analysis::{
    Analysis, Beats, LadderRow, Model, ModuleStatus, OffRef, OptionInfo, Stats, Status, ValueView, instance_name,
    module_detail_label, module_label, module_short_label, nested_file_label, omitted_badge, parent_path, ring_text,
    value_view,
};
pub use model::{Condition, DefKind, Definition, Graph, Meta, Module, OptionEntry, Origin, OriginClass, Warning};
