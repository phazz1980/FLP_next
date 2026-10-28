//! `[layout]` of the project file: block coordinates, used only by the editor.

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
    /// Hand-set routes keyed by the input the link enters ("block.port").
    #[serde(rename = "wire", default, skip_serializing_if = "BTreeMap::is_empty")]
    pub wires: BTreeMap<String, WireRoute>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SheetInfo {
    pub number: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub comment: String,
    /// The blocks of the sheet run in a background task (the second core of the ESP32).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub background: bool,
    /// Period of a background sheet on a single-core chip, ms; 0 is the default.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub period: u32,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

/// Default period of a background sheet on a single-core chip, ms.
pub const DEFAULT_BACKGROUND_PERIOD_MS: u32 = 10;

/// Manual route of a link. A link without an entry is routed automatically.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct WireRoute {
    /// Where the middle segment runs, in sheet coordinates: x of the vertical segment
    /// for a link going forward, y of the horizontal one for a link going back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bend: Option<f32>,
    /// Drawn as two labelled stubs, like a link between sheets.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub broken: bool,
}

impl Layout {
    pub fn is_empty(&self) -> bool {
        self.sheets.is_empty() && self.blocks.is_empty() && self.wires.is_empty()
    }

    /// Key of a link in `wires`.
    pub fn wire_key(to: &crate::PortRef) -> String {
        format!("{}.{}", to.block, to.port)
    }

    pub fn sheet(&self, number: u32) -> Option<&SheetInfo> {
        self.sheets.iter().find(|s| s.number == number)
    }

    /// The blocks of the sheet run in the background task.
    pub fn is_background(&self, number: u32) -> bool {
        self.sheet(number).is_some_and(|s| s.background)
    }

    /// Period of the background sheet when it runs inside `loop()`, ms.
    pub fn background_period(&self, number: u32) -> u32 {
        match self.sheet(number).map_or(0, |s| s.period) {
            0 => DEFAULT_BACKGROUND_PERIOD_MS,
            p => p,
        }
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
