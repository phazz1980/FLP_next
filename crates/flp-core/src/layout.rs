//! `layout.toml`: block coordinates, used only by the editor.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Names and comments of sheets; sheets without them need no entry.
    #[serde(rename = "sheet", default, skip_serializing_if = "Vec::is_empty")]
    pub sheets: Vec<SheetInfo>,
    /// Positions keyed by block id.
    #[serde(rename = "block", default, skip_serializing_if = "BTreeMap::is_empty")]
    pub blocks: BTreeMap<String, BlockPos>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SheetInfo {
    pub number: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,
}

impl Layout {
    pub fn sheet(&self, number: u32) -> Option<&SheetInfo> {
        self.sheets.iter().find(|s| s.number == number)
    }
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
