//! Open project with undo/redo and the latest check report.

use std::path::Path;

use flp_core::{check, BlockPos, Layout, Library, Logic, Project, Report};

/// State that undo/redo restores.
#[derive(Clone)]
struct Snapshot {
    logic: Logic,
    layout: Layout,
}

pub struct Doc {
    pub project: Project,
    /// Unsaved changes.
    pub dirty: bool,
    /// Check result for the current schema, refreshed after every edit.
    pub report: Report,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// Key of the last edit: consecutive edits with the same key (dragging a block,
    /// typing a value) form one undo step.
    last_key: Option<String>,
}

const UNDO_LIMIT: usize = 200;

impl Doc {
    pub fn open(path: &Path, lib: &Library) -> flp_core::Result<Self> {
        let mut project = Project::load(path)?;
        place_unplaced(&mut project);
        let report = check(&project.logic, lib, Some(&project.layout));
        Ok(Self {
            project,
            dirty: false,
            report,
            undo: Vec::new(),
            redo: Vec::new(),
            last_key: None,
        })
    }

    pub fn recheck(&mut self, lib: &Library) {
        self.report = check(&self.project.logic, lib, Some(&self.project.layout));
    }

    /// Applies an edit as one undo step. `key` merges consecutive edits of the same
    /// thing; `None` always starts a new step. A failed edit leaves no trace.
    pub fn edit<T, E>(
        &mut self,
        lib: &Library,
        key: Option<String>,
        f: impl FnOnce(&mut Project) -> Result<T, E>,
    ) -> Result<T, E> {
        let before = self.snapshot();
        let merge = key.is_some() && key == self.last_key;
        match f(&mut self.project) {
            Ok(value) => {
                if !merge {
                    self.undo.push(before);
                    if self.undo.len() > UNDO_LIMIT {
                        self.undo.remove(0);
                    }
                }
                self.redo.clear();
                self.last_key = key;
                self.dirty = true;
                self.recheck(lib);
                Ok(value)
            }
            Err(e) => {
                self.restore(before);
                Err(e)
            }
        }
    }

    /// Ends merging of consecutive edits (e.g. when a drag is released).
    pub fn end_edit(&mut self) {
        self.last_key = None;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, lib: &Library) {
        if let Some(s) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.restore(s);
            self.after_history(lib);
        }
    }

    pub fn redo(&mut self, lib: &Library) {
        if let Some(s) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.restore(s);
            self.after_history(lib);
        }
    }

    pub fn save(&mut self) -> flp_core::Result<()> {
        self.project.save()?;
        self.dirty = false;
        Ok(())
    }

    fn after_history(&mut self, lib: &Library) {
        self.last_key = None;
        self.dirty = true;
        self.recheck(lib);
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            logic: self.project.logic.clone(),
            layout: self.project.layout.clone(),
        }
    }

    fn restore(&mut self, s: Snapshot) {
        self.project.logic = s.logic;
        self.project.layout = s.layout;
    }
}

/// Gives positions to blocks that have none (schemas written by hand or by AI):
/// a grid on sheet 1 below the placed blocks.
fn place_unplaced(project: &mut Project) {
    let layout = &mut project.layout;
    let bottom = layout
        .blocks
        .values()
        .filter(|p| p.sheet == 1)
        .map(|p| p.y)
        .fold(0.0_f32, f32::max);
    let missing: Vec<String> = project
        .logic
        .blocks
        .iter()
        .filter(|b| !layout.blocks.contains_key(&b.id))
        .map(|b| b.id.clone())
        .collect();
    for (i, id) in missing.into_iter().enumerate() {
        let (col, row) = (i % 5, i / 5);
        layout.blocks.insert(
            id,
            BlockPos {
                sheet: 1,
                x: 40.0 + 200.0 * col as f32,
                y: bottom + 160.0 + 140.0 * row as f32,
            },
        );
    }
}
