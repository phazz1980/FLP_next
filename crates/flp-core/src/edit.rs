//! Schema editing operations shared by the editor and (later) the AI commands.
//!
//! Every operation keeps `logic` and `layout` consistent and rejects edits that
//! would make the schema structurally invalid (unknown types, wrong port
//! directions, incompatible types). Semantic problems (unconnected inputs,
//! loops) are left to [`crate::check`].

use crate::ident::is_valid_ident;
use crate::vars::{Var, VAR_GET, VAR_PARAM, VAR_SET, WRITE_ENABLE};
use crate::{Block, BlockPos, Library, Link, ParamValue, PortRef, Project, SignalType};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    #[error("нет блока «{0}»")]
    NoBlock(String),
    #[error("неизвестный тип блока «{0}»")]
    UnknownType(String),
    #[error("недопустимый id «{0}»: только буквы (можно русские), цифры и «_», первой — буква, без пробелов и точек")]
    InvalidId(String),
    #[error("id «{0}» уже занят")]
    DuplicateId(String),
    #[error("у блока «{block}» нет выхода «{port}»")]
    NoOutput { block: String, port: String },
    #[error("у блока «{block}» нет входа «{port}»")]
    NoInput { block: String, port: String },
    #[error("выход {from} нельзя подать на вход {to}: не совпадают типы")]
    TypeMismatch { from: PortRef, to: PortRef },
    #[error("порт {0} не логический: инвертировать можно только bool")]
    NotBool(PortRef),
    #[error("у блока «{block}» нет параметра «{param}»")]
    NoParam { block: String, param: String },
    #[error("значение не подходит к параметру «{0}»")]
    BadValue(String),
    #[error("нет переменной «{0}»")]
    NoVar(String),
    #[error("блок переменной «{0}» уже подключён с другой стороны: переменную в одном блоке можно либо читать, либо записывать")]
    VarBothSides(String),
    #[error(
        "у блока переменной «{0}» включён вход разрешения записи: он только записывает, выхода нет"
    )]
    VarWriteOnly(String),
    #[error("недопустимое имя переменной «{0}»: пустое, с пробелами по краям или с управляющими символами")]
    InvalidVarName(String),
    #[error("переменная «{0}» уже есть")]
    DuplicateVar(String),
    #[error("начальное значение не подходит к типу переменной «{0}»")]
    BadInit(String),
}

impl Project {
    /// Adds a block of type `kind` with a generated id (`timer_on_1`, ...) and returns the id.
    pub fn add_block(
        &mut self,
        lib: &Library,
        kind: &str,
        pos: BlockPos,
    ) -> Result<String, EditError> {
        if lib.get(kind).is_none() {
            return Err(EditError::UnknownType(kind.to_owned()));
        }
        let id = (1..)
            .map(|n| format!("{kind}_{n}"))
            .find(|id| self.logic.block(id).is_none())
            .expect("infinite range");
        self.logic.blocks.push(Block {
            id: id.clone(),
            kind: kind.to_owned(),
            ..Default::default()
        });
        self.layout.blocks.insert(id.clone(), pos);
        Ok(id)
    }

    /// Adds a block that reads (`write = false`) or writes a variable; the id is
    /// derived from the variable name (`setpoint_r1`, `setpoint_w1`).
    pub fn add_var_block(
        &mut self,
        var: &str,
        write: bool,
        pos: BlockPos,
    ) -> Result<String, EditError> {
        if self.logic.var(var).is_none() {
            return Err(EditError::NoVar(var.to_owned()));
        }
        let (kind, suffix) = if write {
            (VAR_SET, "w")
        } else {
            (VAR_GET, "r")
        };
        let id = (1..)
            .map(|n| format!("{var}_{suffix}{n}"))
            .find(|id| self.logic.block(id).is_none())
            .expect("infinite range");
        self.logic.blocks.push(Block {
            id: id.clone(),
            kind: kind.to_owned(),
            params: [(VAR_PARAM.to_owned(), ParamValue::Text(var.to_owned()))].into(),
            ..Default::default()
        });
        self.layout.blocks.insert(id.clone(), pos);
        Ok(id)
    }

    /// Declares a new variable.
    pub fn add_var(&mut self, var: Var) -> Result<(), EditError> {
        self.validate_var(&var, None)?;
        self.logic.vars.push(var);
        Ok(())
    }

    /// Replaces a variable declaration (name, type, initial value, description).
    /// Blocks using it follow a rename; links that no longer fit the new type stay
    /// and are reported by the check.
    pub fn update_var(&mut self, name: &str, var: Var) -> Result<(), EditError> {
        let index = self.var_index(name)?;
        self.validate_var(&var, Some(name))?;
        if var.name != name {
            for block in &mut self.logic.blocks {
                if block.var_name() == Some(name) {
                    block
                        .params
                        .insert(VAR_PARAM.to_owned(), ParamValue::Text(var.name.clone()));
                }
            }
        }
        self.logic.vars[index] = var;
        Ok(())
    }

    /// Removes a variable together with the blocks that read or write it.
    pub fn remove_var(&mut self, name: &str) -> Result<(), EditError> {
        let index = self.var_index(name)?;
        self.logic.vars.remove(index);
        let users: Vec<String> = self
            .logic
            .blocks
            .iter()
            .filter(|b| b.var_name() == Some(name))
            .map(|b| b.id.clone())
            .collect();
        for id in users {
            self.remove_block(&id)?;
        }
        Ok(())
    }

    fn var_index(&self, name: &str) -> Result<usize, EditError> {
        self.logic
            .vars
            .iter()
            .position(|v| v.name == name)
            .ok_or_else(|| EditError::NoVar(name.to_owned()))
    }

    fn validate_var(&self, var: &Var, old_name: Option<&str>) -> Result<(), EditError> {
        if !crate::vars::is_valid_var_name(&var.name) {
            return Err(EditError::InvalidVarName(var.name.clone()));
        }
        if Some(var.name.as_str()) != old_name && self.logic.var(&var.name).is_some() {
            return Err(EditError::DuplicateVar(var.name.clone()));
        }
        if var.init.as_ref().is_some_and(|i| !var.ty.accepts(i)) {
            return Err(EditError::BadInit(var.name.clone()));
        }
        Ok(())
    }

    /// A free spot on `sheet` for a block placed without coordinates (by AI or a
    /// command): blocks go left to right in rows below the existing ones.
    pub fn free_position(&self, sheet: u32) -> BlockPos {
        let on_sheet: Vec<&BlockPos> = self
            .layout
            .blocks
            .values()
            .filter(|p| p.sheet == sheet)
            .collect();
        let bottom = on_sheet.iter().map(|p| p.y).fold(-120.0_f32, f32::max);
        let row: Vec<&&BlockPos> = on_sheet.iter().filter(|p| p.y == bottom).collect();
        let right = row.iter().map(|p| p.x).fold(-140.0_f32, f32::max);
        if right + 140.0 < 1000.0 && !row.is_empty() {
            BlockPos {
                sheet,
                x: right + 140.0,
                y: bottom,
            }
        } else {
            BlockPos {
                sheet,
                x: 40.0,
                y: bottom + 120.0,
            }
        }
    }

    /// Sets the name and comment of a sheet; an empty pair removes the entry.
    pub fn set_sheet_info(&mut self, info: crate::SheetInfo) {
        let sheets = &mut self.layout.sheets;
        sheets.retain(|s| s.number != info.number);
        if !info.name.is_empty() || !info.comment.is_empty() {
            sheets.push(info);
            sheets.sort_by_key(|s| s.number);
        }
    }

    /// Removes a block with its links and position.
    pub fn remove_block(&mut self, id: &str) -> Result<(), EditError> {
        let index = self.block_index(id)?;
        self.logic.blocks.remove(index);
        self.logic
            .links
            .retain(|l| l.from.block != id && l.to.block != id);
        self.layout.blocks.remove(id);
        Ok(())
    }

    /// Renames a block, updating its links and position.
    pub fn rename_block(&mut self, old: &str, new: &str) -> Result<(), EditError> {
        let index = self.block_index(old)?;
        if old == new {
            return Ok(());
        }
        if !is_valid_ident(new) {
            return Err(EditError::InvalidId(new.to_owned()));
        }
        if self.logic.block(new).is_some() {
            return Err(EditError::DuplicateId(new.to_owned()));
        }
        self.logic.blocks[index].id = new.to_owned();
        for link in &mut self.logic.links {
            for end in [&mut link.from, &mut link.to] {
                if end.block == old {
                    end.block = new.to_owned();
                }
            }
        }
        if let Some(pos) = self.layout.blocks.remove(old) {
            self.layout.blocks.insert(new.to_owned(), pos);
        }
        Ok(())
    }

    /// Connects an output to an input. An existing link into the same input is replaced.
    pub fn connect(&mut self, lib: &Library, from: PortRef, to: PortRef) -> Result<(), EditError> {
        // A variable block reads or writes depending on the side the link comes to.
        self.adapt_var_block(&from.block, false)?;
        self.adapt_var_block(&to.block, true)?;
        let out_ty = {
            let def = self.block_def(lib, &from.block)?;
            def.output(&from.port)
                .ok_or_else(|| EditError::NoOutput {
                    block: from.block.clone(),
                    port: from.port.clone(),
                })?
                .ty
        };
        let in_ty = {
            let def = self.block_def(lib, &to.block)?;
            def.input(&to.port)
                .ok_or_else(|| EditError::NoInput {
                    block: to.block.clone(),
                    port: to.port.clone(),
                })?
                .ty
        };
        if !out_ty.can_feed(in_ty) {
            return Err(EditError::TypeMismatch { from, to });
        }
        self.logic.links.retain(|l| l.to != to);
        if let Some(b) = self.logic.blocks.iter_mut().find(|b| b.id == to.block) {
            b.consts.remove(&to.port);
        }
        self.logic.links.push(Link { from, to });
        Ok(())
    }

    /// Puts a constant on an input (dropping its link) or removes it (`None`).
    pub fn set_input_const(
        &mut self,
        lib: &Library,
        input: &PortRef,
        value: Option<ParamValue>,
    ) -> Result<(), EditError> {
        let ty = self
            .block_def(lib, &input.block)?
            .input(&input.port)
            .ok_or_else(|| EditError::NoInput {
                block: input.block.clone(),
                port: input.port.clone(),
            })?
            .ty;
        let block = self.block_mut(&input.block)?;
        match value {
            Some(v) if !ty.accepts(&v) => return Err(EditError::BadValue(input.port.clone())),
            Some(v) => {
                block.consts.insert(input.port.clone(), v);
                self.logic.links.retain(|l| &l.to != input);
            }
            None => {
                block.consts.remove(&input.port);
            }
        }
        Ok(())
    }

    /// Inverts a logical port (an output for all its links, or an input), or
    /// stops inverting it.
    pub fn set_port_inverted(
        &mut self,
        lib: &Library,
        port: &PortRef,
        output: bool,
        on: bool,
    ) -> Result<(), EditError> {
        let def = self.block_def(lib, &port.block)?;
        let ty = if output {
            def.output(&port.port).ok_or_else(|| EditError::NoOutput {
                block: port.block.clone(),
                port: port.port.clone(),
            })?
        } else {
            def.input(&port.port).ok_or_else(|| EditError::NoInput {
                block: port.block.clone(),
                port: port.port.clone(),
            })?
        }
        .ty;
        if ty != SignalType::Bool {
            return Err(EditError::NotBool(port.clone()));
        }
        let block = self.block_mut(&port.block)?;
        let set = if output {
            &mut block.inverted
        } else {
            &mut block.inverted_inputs
        };
        if on {
            set.insert(port.port.clone());
        } else {
            set.remove(&port.port);
        }
        Ok(())
    }

    fn block_mut(&mut self, id: &str) -> Result<&mut Block, EditError> {
        self.logic
            .blocks
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or_else(|| EditError::NoBlock(id.to_owned()))
    }

    /// Switches the write enable input `en` of one variable block. Turning it on
    /// makes an unconnected read a write; a read with links cannot get it.
    pub fn set_var_write_enable(&mut self, id: &str, on: bool) -> Result<(), EditError> {
        let linked = self
            .logic
            .links
            .iter()
            .any(|l| l.from.block == id || l.to.block == id);
        let block = self
            .logic
            .blocks
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or_else(|| EditError::NoBlock(id.to_owned()))?;
        if block.var_name().is_none() {
            return Err(EditError::NoBlock(id.to_owned()));
        }
        if on {
            if block.kind == VAR_GET {
                if linked {
                    return Err(EditError::VarBothSides(id.to_owned()));
                }
                block.kind = VAR_SET.to_owned();
            }
            block
                .params
                .insert(WRITE_ENABLE.to_owned(), ParamValue::Bool(true));
        } else {
            block.params.remove(WRITE_ENABLE);
            self.logic
                .links
                .retain(|l| !(l.to.block == id && l.to.port == "en"));
        }
        Ok(())
    }

    /// Turns a variable block into a reader or a writer before a link is attached;
    /// allowed only while the block has no links.
    fn adapt_var_block(&mut self, id: &str, write: bool) -> Result<(), EditError> {
        let want = if write { VAR_SET } else { VAR_GET };
        let Some(block) = self.logic.block(id) else {
            return Ok(());
        };
        if (block.kind != VAR_GET && block.kind != VAR_SET) || block.kind == want {
            return Ok(());
        }
        // A write with the enable input stays a write.
        if matches!(block.params.get(WRITE_ENABLE), Some(ParamValue::Bool(true))) {
            return Err(EditError::VarWriteOnly(id.to_owned()));
        }
        if self
            .logic
            .links
            .iter()
            .any(|l| l.from.block == id || l.to.block == id)
        {
            return Err(EditError::VarBothSides(id.to_owned()));
        }
        if let Some(b) = self.logic.blocks.iter_mut().find(|b| b.id == id) {
            b.kind = want.to_owned();
            if !write {
                b.params.remove(WRITE_ENABLE);
            }
        }
        Ok(())
    }

    /// Removes the link into `to`. Returns whether there was one.
    pub fn disconnect(&mut self, to: &PortRef) -> bool {
        let before = self.logic.links.len();
        self.logic.links.retain(|l| &l.to != to);
        self.logic.links.len() != before
    }

    /// Sets a parameter; `None` removes it so the default applies.
    pub fn set_param(
        &mut self,
        lib: &Library,
        id: &str,
        name: &str,
        value: Option<ParamValue>,
    ) -> Result<(), EditError> {
        let def = self.block_def(lib, id)?;
        let param = def.param(name).ok_or_else(|| EditError::NoParam {
            block: id.to_owned(),
            param: name.to_owned(),
        })?;
        if value
            .as_ref()
            .is_some_and(|v| !param.kind.accepts(v) || !param.allows(v))
        {
            return Err(EditError::BadValue(name.to_owned()));
        }
        let index = self.block_index(id)?;
        let params = &mut self.logic.blocks[index].params;
        match value {
            Some(v) => params.insert(name.to_owned(), v),
            None => params.remove(name),
        };
        Ok(())
    }

    /// Moves a block; blocks without a position get one.
    pub fn move_block(&mut self, id: &str, pos: BlockPos) -> Result<(), EditError> {
        self.block_index(id)?;
        self.layout.blocks.insert(id.to_owned(), pos);
        Ok(())
    }

    fn block_index(&self, id: &str) -> Result<usize, EditError> {
        self.logic
            .blocks
            .iter()
            .position(|b| b.id == id)
            .ok_or_else(|| EditError::NoBlock(id.to_owned()))
    }

    fn block_def<'l>(&self, lib: &'l Library, id: &str) -> Result<&'l crate::BlockDef, EditError> {
        let block = &self.logic.blocks[self.block_index(id)?];
        lib.resolve(&self.logic, block)
            .map(|b| &b.def)
            .ok_or_else(|| EditError::UnknownType(block.kind.clone()))
    }
}

/// Blocks copied to the clipboard: the blocks, links between them, the variables
/// they use and positions relative to the top-left block. Serialized as JSON, so
/// it can be pasted into another editor window or project.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Fragment {
    #[serde(default)]
    pub vars: Vec<Var>,
    pub blocks: Vec<Block>,
    #[serde(default)]
    pub links: Vec<Link>,
    /// Offsets from the top-left corner of the copied group, by block id.
    #[serde(default)]
    pub offsets: std::collections::BTreeMap<String, (f32, f32)>,
}

impl Fragment {
    /// Marker so arbitrary clipboard text is not taken for blocks.
    const TAG: &'static str = "flp-fragment";

    pub fn to_clipboard(&self) -> String {
        let mut v = serde_json::to_value(self).expect("fragment serializes");
        v["format"] = Self::TAG.into();
        serde_json::to_string_pretty(&v).expect("json")
    }

    /// Parses clipboard text; `None` when it is not a copied fragment.
    pub fn from_clipboard(text: &str) -> Option<Self> {
        let v: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
        if v.get("format")?.as_str()? != Self::TAG {
            return None;
        }
        serde_json::from_value(v).ok()
    }
}

impl Project {
    /// Copies the given blocks with the links among them; unknown ids are skipped.
    pub fn copy_fragment(&self, ids: &[String]) -> Fragment {
        let blocks: Vec<Block> = self
            .logic
            .blocks
            .iter()
            .filter(|b| ids.contains(&b.id))
            .cloned()
            .collect();
        let has = |id: &str| blocks.iter().any(|b| b.id == id);
        let links = self
            .logic
            .links
            .iter()
            .filter(|l| has(&l.from.block) && has(&l.to.block))
            .cloned()
            .collect();
        let mut vars: Vec<Var> = Vec::new();
        for name in blocks.iter().filter_map(|b| b.var_name()) {
            if let Some(v) = self.logic.var(name) {
                if !vars.iter().any(|x| x.name == v.name) {
                    vars.push(v.clone());
                }
            }
        }
        let pos: Vec<(&str, BlockPos)> = blocks
            .iter()
            .filter_map(|b| Some((b.id.as_str(), *self.layout.blocks.get(&b.id)?)))
            .collect();
        let min_x = pos.iter().map(|p| p.1.x).fold(f32::INFINITY, f32::min);
        let min_y = pos.iter().map(|p| p.1.y).fold(f32::INFINITY, f32::min);
        let offsets = pos
            .iter()
            .map(|(id, p)| ((*id).to_owned(), (p.x - min_x, p.y - min_y)))
            .collect();
        Fragment {
            vars,
            blocks,
            links,
            offsets,
        }
    }

    /// Inserts a fragment with its top-left corner at `at` and returns the new ids.
    /// Taken ids get a numeric suffix; missing variables are declared, existing
    /// ones with the same name are reused.
    pub fn paste_fragment(
        &mut self,
        lib: &Library,
        frag: &Fragment,
        at: BlockPos,
    ) -> Result<Vec<String>, EditError> {
        for b in &frag.blocks {
            if b.var_name().is_none() && lib.get(&b.kind).is_none() {
                return Err(EditError::UnknownType(b.kind.clone()));
            }
            if !is_valid_ident(&b.id) {
                return Err(EditError::InvalidId(b.id.clone()));
            }
        }
        for v in &frag.vars {
            if self.logic.var(&v.name).is_none() {
                self.add_var(v.clone())?;
            }
        }
        let mut renamed = std::collections::BTreeMap::new();
        let mut ids = Vec::new();
        for b in &frag.blocks {
            if let Some(name) = b.var_name() {
                if self.logic.var(name).is_none() {
                    return Err(EditError::NoVar(name.to_owned()));
                }
            }
            let id = if self.logic.block(&b.id).is_none() {
                b.id.clone()
            } else {
                let base = b.id.trim_end_matches(|c: char| c.is_ascii_digit());
                let base = base.trim_end_matches('_');
                (2..)
                    .map(|n| format!("{base}_{n}"))
                    .find(|id| self.logic.block(id).is_none())
                    .expect("infinite range")
            };
            let (dx, dy) = frag.offsets.get(&b.id).copied().unwrap_or_default();
            self.logic.blocks.push(Block {
                id: id.clone(),
                ..b.clone()
            });
            self.layout.blocks.insert(
                id.clone(),
                BlockPos {
                    sheet: at.sheet,
                    x: at.x + dx,
                    y: at.y + dy,
                },
            );
            renamed.insert(b.id.clone(), id.clone());
            ids.push(id);
        }
        for l in &frag.links {
            let (Some(from), Some(to)) = (renamed.get(&l.from.block), renamed.get(&l.to.block))
            else {
                continue;
            };
            self.connect(
                lib,
                PortRef::new(from, &l.from.port),
                PortRef::new(to, &l.to.port),
            )?;
        }
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockDef, LibraryBlock, Logic, ProjectConfig};

    fn library() -> Library {
        let mut lib = Library::default();
        for (kind, extra) in [
            ("src", "[[output]]\nname = \"out\"\ntype = \"bool\"\n"),
            ("num", "[[output]]\nname = \"out\"\ntype = \"float\"\n"),
            (
                "sink",
                "[[input]]\nname = \"in\"\ntype = \"bool\"\n\
                 [[param]]\nname = \"pin\"\ntype = \"pin\"\n",
            ),
        ] {
            let def: BlockDef = toml::from_str(&format!(
                "[block]\ntype = \"{kind}\"\nname = \"{kind}\"\ngroup = \"Тест\"\n{extra}"
            ))
            .unwrap();
            lib.insert(LibraryBlock {
                def,
                dir: Default::default(),
                inline_templates: Default::default(),
            });
        }
        lib
    }

    fn project() -> Project {
        let config: ProjectConfig =
            toml::from_str("[project]\nname = \"t\"\n[board]\nfqbn = \"x:y:z\"\n").unwrap();
        Project {
            path: Default::default(),
            config,
            logic: Logic::default(),
            layout: Default::default(),
        }
    }

    const POS: BlockPos = BlockPos {
        sheet: 1,
        x: 0.0,
        y: 0.0,
    };

    #[test]
    fn add_generates_ids() {
        let lib = library();
        let mut p = project();
        assert_eq!(p.add_block(&lib, "src", POS).unwrap(), "src_1");
        assert_eq!(p.add_block(&lib, "src", POS).unwrap(), "src_2");
        p.remove_block("src_1").unwrap();
        assert_eq!(p.add_block(&lib, "src", POS).unwrap(), "src_1");
        assert!(p.add_block(&lib, "nope", POS).is_err());
        assert_eq!(p.layout.blocks.len(), 2);
    }

    #[test]
    fn connect_replaces_and_validates() {
        let lib = library();
        let mut p = project();
        let a = p.add_block(&lib, "src", POS).unwrap();
        let b = p.add_block(&lib, "src", POS).unwrap();
        let n = p.add_block(&lib, "num", POS).unwrap();
        let s = p.add_block(&lib, "sink", POS).unwrap();
        let input = PortRef::new(&s, "in");
        p.connect(&lib, PortRef::new(&a, "out"), input.clone())
            .unwrap();
        p.connect(&lib, PortRef::new(&b, "out"), input.clone())
            .unwrap();
        assert_eq!(p.logic.links.len(), 1);
        assert_eq!(p.logic.links[0].from.block, b);
        assert!(matches!(
            p.connect(&lib, PortRef::new(&n, "out"), input.clone()),
            Err(EditError::TypeMismatch { .. })
        ));
        assert!(p
            .connect(&lib, PortRef::new(&s, "in"), PortRef::new(&a, "out"))
            .is_err());
        assert!(p.disconnect(&input));
        assert!(!p.disconnect(&input));
    }

    #[test]
    fn constant_and_link_replace_each_other() {
        let lib = library();
        let mut p = project();
        let a = p.add_block(&lib, "src", POS).unwrap();
        let s = p.add_block(&lib, "sink", POS).unwrap();
        let input = PortRef::new(&s, "in");
        p.connect(&lib, PortRef::new(&a, "out"), input.clone())
            .unwrap();
        p.set_input_const(&lib, &input, Some(ParamValue::Bool(true)))
            .unwrap();
        assert!(p.logic.links.is_empty());
        assert!(matches!(
            p.set_input_const(&lib, &input, Some(ParamValue::Float(1.0))),
            Err(EditError::BadValue(_))
        ));
        p.connect(&lib, PortRef::new(&a, "out"), input.clone())
            .unwrap();
        assert!(p.logic.block(&s).unwrap().consts.is_empty());
    }

    #[test]
    fn only_logical_ports_invert() {
        let lib = library();
        let mut p = project();
        let a = p.add_block(&lib, "src", POS).unwrap();
        let n = p.add_block(&lib, "num", POS).unwrap();
        let s = p.add_block(&lib, "sink", POS).unwrap();
        p.set_port_inverted(&lib, &PortRef::new(&a, "out"), true, true)
            .unwrap();
        p.set_port_inverted(&lib, &PortRef::new(&s, "in"), false, true)
            .unwrap();
        assert!(p.logic.block(&a).unwrap().inverted.contains("out"));
        assert!(p.logic.block(&s).unwrap().inverted_inputs.contains("in"));
        assert!(matches!(
            p.set_port_inverted(&lib, &PortRef::new(&n, "out"), true, true),
            Err(EditError::NotBool(_))
        ));
        // Saved and loaded back unchanged.
        let text = toml::to_string(&p.logic).unwrap();
        assert_eq!(toml::from_str::<Logic>(&text).unwrap(), p.logic);
    }

    #[test]
    fn rename_and_remove_keep_links_consistent() {
        let lib = library();
        let mut p = project();
        let a = p.add_block(&lib, "src", POS).unwrap();
        let s = p.add_block(&lib, "sink", POS).unwrap();
        p.connect(&lib, PortRef::new(&a, "out"), PortRef::new(&s, "in"))
            .unwrap();
        p.rename_block(&a, "button").unwrap();
        assert_eq!(p.logic.links[0].from, PortRef::new("button", "out"));
        assert!(p.layout.blocks.contains_key("button"));
        assert_eq!(
            p.rename_block("button", &s),
            Err(EditError::DuplicateId(s.clone()))
        );
        assert!(p.rename_block("button", "1x").is_err());
        p.remove_block("button").unwrap();
        assert!(p.logic.links.is_empty());
    }

    #[test]
    fn variables() {
        let mut p = project();
        let var = |name: &str| Var {
            name: name.into(),
            ty: crate::VarType::Float,
            init: Some(ParamValue::Float(1.5)),
            description: String::new(),
            retain: false,
        };
        p.add_var(var("t")).unwrap();
        assert_eq!(
            p.add_var(var("t")),
            Err(EditError::DuplicateVar("t".into()))
        );
        assert!(p.add_var(var(" t")).is_err());
        let r = p.add_var_block("t", false, POS).unwrap();
        let w = p.add_var_block("t", true, POS).unwrap();
        assert_eq!((r.as_str(), w.as_str()), ("t_r1", "t_w1"));
        assert!(p.add_var_block("nope", false, POS).is_err());

        p.update_var("t", var("temp")).unwrap();
        assert!(p.logic.blocks.iter().all(|b| b.var_name() == Some("temp")));
        let mut bad = var("temp");
        bad.ty = crate::VarType::Bool;
        assert_eq!(
            p.update_var("temp", bad),
            Err(EditError::BadInit("temp".into()))
        );

        p.remove_var("temp").unwrap();
        assert!(p.logic.vars.is_empty() && p.logic.blocks.is_empty());
    }

    #[test]
    fn var_block_side_decides_access() {
        let lib = library();
        let mut p = project();
        p.add_var(Var {
            name: "on".into(),
            ty: crate::VarType::Bool,
            init: None,
            description: String::new(),
            retain: false,
        })
        .unwrap();
        let src = p.add_block(&lib, "src", POS).unwrap();
        let sink = p.add_block(&lib, "sink", POS).unwrap();
        let tag = p.add_var_block("on", false, POS).unwrap();
        // Link into the reader turns it into a writer.
        p.connect(&lib, PortRef::new(&src, "out"), PortRef::new(&tag, "in"))
            .unwrap();
        assert_eq!(p.logic.block(&tag).unwrap().kind, VAR_SET);
        // Now the output side is taken by the write.
        assert_eq!(
            p.connect(&lib, PortRef::new(&tag, "out"), PortRef::new(&sink, "in")),
            Err(EditError::VarBothSides(tag.clone()))
        );
        p.disconnect(&PortRef::new(&tag, "in"));
        p.connect(&lib, PortRef::new(&tag, "out"), PortRef::new(&sink, "in"))
            .unwrap();
        assert_eq!(p.logic.block(&tag).unwrap().kind, VAR_GET);
    }

    #[test]
    fn write_enable_per_block() {
        let lib = library();
        let mut p = project();
        p.add_var(Var {
            name: "on".into(),
            ty: crate::VarType::Bool,
            init: None,
            description: String::new(),
            retain: false,
        })
        .unwrap();
        let tag = p.add_var_block("on", false, POS).unwrap();
        let other = p.add_var_block("on", false, POS).unwrap();
        let inputs = |p: &Project, id: &str| {
            let b = p.logic.block(id).unwrap();
            lib.resolve(&p.logic, b).unwrap().def.inputs.len()
        };
        p.set_var_write_enable(&tag, true).unwrap();
        assert_eq!(p.logic.block(&tag).unwrap().kind, VAR_SET);
        assert_eq!(inputs(&p, &tag), 2);
        assert_eq!(p.logic.block(&other).unwrap().kind, VAR_GET);
        p.set_var_write_enable(&tag, false).unwrap();
        assert_eq!(inputs(&p, &tag), 1);
    }

    #[test]
    fn params() {
        let lib = library();
        let mut p = project();
        let s = p.add_block(&lib, "sink", POS).unwrap();
        p.set_param(&lib, &s, "pin", Some(ParamValue::Int(4)))
            .unwrap();
        assert_eq!(p.logic.blocks[0].params["pin"], ParamValue::Int(4));
        assert!(p
            .set_param(&lib, &s, "pin", Some(ParamValue::Bool(true)))
            .is_err());
        assert!(p.set_param(&lib, &s, "speed", None).is_err());
        p.set_param(&lib, &s, "pin", None).unwrap();
        assert!(p.logic.blocks[0].params.is_empty());
    }

    #[test]
    fn copy_paste_fragment() {
        let lib = library();
        let mut p = project();
        let a = p.add_block(&lib, "src", POS).unwrap();
        let b = p
            .add_block(&lib, "sink", BlockPos { x: 100.0, ..POS })
            .unwrap();
        p.connect(&lib, PortRef::new(&a, "out"), PortRef::new(&b, "in"))
            .unwrap();
        let frag = p.copy_fragment(&[a.clone(), b.clone()]);
        let text = frag.to_clipboard();
        assert_eq!(Fragment::from_clipboard(&text), Some(frag.clone()));
        assert!(Fragment::from_clipboard("{}").is_none());

        let at = BlockPos {
            sheet: 2,
            x: 40.0,
            y: 60.0,
        };
        let ids = p.paste_fragment(&lib, &frag, at).unwrap();
        assert_eq!(ids, ["src_2", "sink_2"]);
        assert_eq!(p.logic.links.len(), 2);
        assert_eq!(p.layout.blocks["sink_2"].x, 140.0);
        assert_eq!(p.layout.blocks["sink_2"].sheet, 2);

        // Into another project, keeping ids.
        let mut q = project();
        assert_eq!(q.paste_fragment(&lib, &frag, POS).unwrap(), [a, b]);
    }
}
