//! Schema editing operations shared by the editor and (later) the AI commands.
//!
//! Every operation keeps `logic` and `layout` consistent and rejects edits that
//! would make the schema structurally invalid (unknown types, wrong port
//! directions, incompatible types). Semantic problems (unconnected inputs,
//! loops) are left to [`crate::check`].

use crate::ident::is_valid_ident;
use crate::{Block, BlockPos, Library, Link, ParamValue, PortRef, Project};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    #[error("нет блока «{0}»")]
    NoBlock(String),
    #[error("неизвестный тип блока «{0}»")]
    UnknownType(String),
    #[error("недопустимый id «{0}»: только латиница, цифры и «_», первой — буква")]
    InvalidId(String),
    #[error("id «{0}» уже занят")]
    DuplicateId(String),
    #[error("у блока «{block}» нет выхода «{port}»")]
    NoOutput { block: String, port: String },
    #[error("у блока «{block}» нет входа «{port}»")]
    NoInput { block: String, port: String },
    #[error("выход {from} нельзя подать на вход {to}: не совпадают типы")]
    TypeMismatch { from: PortRef, to: PortRef },
    #[error("у блока «{block}» нет параметра «{param}»")]
    NoParam { block: String, param: String },
    #[error("значение не подходит к параметру «{0}»")]
    BadValue(String),
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
            params: Default::default(),
        });
        self.layout.blocks.insert(id.clone(), pos);
        Ok(id)
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
        self.logic.links.push(Link { from, to });
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
        if value.as_ref().is_some_and(|v| !param.kind.accepts(v)) {
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
            root: Default::default(),
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
}
