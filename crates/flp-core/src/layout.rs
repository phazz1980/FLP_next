//! `layout.toml`: block coordinates, used only by the editor.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Positions keyed by block id.
    #[serde(rename = "block", default, skip_serializing_if = "BTreeMap::is_empty")]
    pub blocks: BTreeMap<String, BlockPos>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BlockPos {
    /// Number of the sheet (board) the block is drawn on, starting from 1.
    #[serde(default = "first_sheet")]
    pub sheet: u32,
    pub x: f32,
    pub y: f32,
}

fn first_sheet() -> u32 {
    1
}
