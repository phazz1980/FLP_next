//! Copies of tags. A tag is a block with a board pin (an input or an output); it
//! may stand on the sheets more than once. Every extra image is a block with
//! `tag = "<tag id>"`: its type, parameters and expander repeat the tag's, its
//! links and constants are its own. A copy of an input gives the tag's value, a
//! copy of an output writes the pin itself (the last one in execution order wins).

use crate::edit::EditError;
use crate::library::{Library, ParamKind};
use crate::{BlockPos, Logic, Project};

impl Logic {
    /// Id of the tag behind `id`: the tag for a copy, `id` itself otherwise.
    pub fn tag_of<'a>(&'a self, id: &'a str) -> &'a str {
        self.block(id).and_then(|b| b.tag.as_deref()).unwrap_or(id)
    }

    /// Ids of the copies of the tag `id`, in schema order.
    pub fn tag_copies<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.blocks
            .iter()
            .filter(move |b| b.tag.as_deref() == Some(id))
            .map(|b| b.id.as_str())
    }

    /// Gives every copy the type, parameters and expander of its tag. Copies of a
    /// missing tag, or of another copy, are left as they are (the check reports them).
    pub fn sync_tags(&mut self) {
        for i in 0..self.blocks.len() {
            let Some(tag) = self.blocks[i].tag.clone() else {
                continue;
            };
            let Some(source) = self.block(&tag).filter(|b| b.tag.is_none()) else {
                continue;
            };
            let (kind, params, expander) = (
                source.kind.clone(),
                source.params.clone(),
                source.expander.clone(),
            );
            let copy = &mut self.blocks[i];
            copy.kind = kind;
            copy.params = params;
            copy.expander = expander;
        }
    }
}

impl Project {
    /// Whether `id` is a tag: a block with a board pin (an input or an output), which
    /// stays in the project tree when it is not on a sheet.
    pub fn is_tag(&self, lib: &Library, id: &str) -> bool {
        self.logic.block(id).is_some_and(|b| {
            b.tag.is_none()
                && lib
                    .resolve(&self.logic, b)
                    .is_some_and(|l| l.def.params.iter().any(|p| p.kind == ParamKind::Pin))
        })
    }

    /// Tags that are not used: neither they nor their copies stand on a sheet or
    /// have links, constants or bindings. They are kept in the tree, skipped by the
    /// empty-input check and left out of the generated code.
    pub fn unused_tags(&self, lib: &Library) -> Vec<String> {
        self.logic
            .blocks
            .iter()
            .filter(|b| self.is_tag(lib, &b.id))
            .filter(|b| {
                std::iter::once(b.id.as_str())
                    .chain(self.logic.tag_copies(&b.id))
                    .all(|id| {
                        let block = self.logic.block(id).expect("listed above");
                        !self.layout.blocks.contains_key(id)
                            && block.consts.is_empty()
                            && block.bound.is_empty()
                            && !self
                                .logic
                                .links
                                .iter()
                                .any(|l| l.from.block == id || l.to.block == id)
                    })
            })
            .map(|b| b.id.clone())
            .collect()
    }

    /// Takes one image of a tag off the sheets. A copy goes away; the tag itself
    /// gives its place to its first copy, or, without copies, loses its position,
    /// links, constants and inversions but stays in the project tree.
    pub fn take_off_board(&mut self, id: &str) -> Result<(), EditError> {
        let index = self.block_index(id)?;
        if self.logic.blocks[index].tag.is_some() {
            return self.remove_block(id);
        }
        if self.hand_tag_to_copy(id) {
            return Ok(());
        }
        let block = &mut self.logic.blocks[index];
        block.consts.clear();
        block.inverted.clear();
        block.inverted_inputs.clear();
        block.bound.clear();
        self.logic
            .links
            .retain(|l| l.from.block != id && l.to.block != id);
        self.layout.blocks.remove(id);
        let prefix = format!("{id}.");
        self.layout.wires.retain(|k, _| !k.starts_with(&prefix));
        Ok(())
    }

    /// Removes the tag behind `id` from the project together with all its copies.
    pub fn remove_tag(&mut self, id: &str) -> Result<(), EditError> {
        let tag = self.logic.tag_of(id).to_owned();
        let copies: Vec<String> = self.logic.tag_copies(&tag).map(str::to_owned).collect();
        for copy in copies {
            self.remove_block(&copy)?;
        }
        self.remove_block(&tag)
    }

    /// Places one more image of the tag `id` (or of the tag behind a copy) at `pos`.
    /// Returns the id of the copy: `<tag>_2`, `<tag>_3`...
    pub fn add_tag_copy(&mut self, id: &str, pos: BlockPos) -> Result<String, EditError> {
        let tag = self.logic.tag_of(id).to_owned();
        let source = self
            .logic
            .block(&tag)
            .ok_or_else(|| EditError::NoBlock(tag.clone()))?;
        let mut copy = source.clone();
        copy.consts.clear();
        copy.inverted.clear();
        copy.inverted_inputs.clear();
        copy.bound.clear();
        copy.id = (2..)
            .map(|n| format!("{tag}_{n}"))
            .find(|id| self.logic.block(id).is_none())
            .expect("infinite range");
        copy.tag = Some(tag);
        let id = copy.id.clone();
        self.logic.blocks.push(copy);
        self.layout.blocks.insert(id.clone(), pos);
        Ok(id)
    }

    /// Before the tag `id` is removed: if it has copies, the first one gives the
    /// tag its place and links and goes away, so the tag stays under its name.
    /// Returns whether the tag stays.
    pub(crate) fn hand_tag_to_copy(&mut self, id: &str) -> bool {
        let Some(copy) = self.logic.tag_copies(id).next().map(str::to_owned) else {
            return false;
        };
        self.logic
            .links
            .retain(|l| l.from.block != id && l.to.block != id);
        let prefix = format!("{id}.");
        self.layout.wires.retain(|k, _| !k.starts_with(&prefix));
        let index = self.logic.blocks.iter().position(|b| b.id == copy);
        let copy_block = self.logic.blocks.remove(index.expect("copy found above"));
        if let Some(tag) = self.logic.blocks.iter_mut().find(|b| b.id == id) {
            tag.consts = copy_block.consts;
            tag.inverted = copy_block.inverted;
            tag.inverted_inputs = copy_block.inverted_inputs;
            tag.bound = copy_block.bound;
        }
        for link in &mut self.logic.links {
            for end in [&mut link.from, &mut link.to] {
                if end.block == copy {
                    end.block = id.to_owned();
                }
            }
        }
        match self.layout.blocks.remove(&copy) {
            Some(pos) => self.layout.blocks.insert(id.to_owned(), pos),
            None => self.layout.blocks.remove(id),
        };
        let copy_prefix = format!("{copy}.");
        let moved: Vec<String> = self
            .layout
            .wires
            .keys()
            .filter(|k| k.starts_with(&copy_prefix))
            .cloned()
            .collect();
        for key in moved {
            if let Some(route) = self.layout.wires.remove(&key) {
                self.layout
                    .wires
                    .insert(format!("{id}.{}", &key[copy_prefix.len()..]), route);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Block, Link, ParamValue, PortRef};

    fn at(x: f32) -> BlockPos {
        BlockPos {
            sheet: 1,
            x,
            y: 0.0,
        }
    }

    fn project() -> Project {
        let config: crate::ProjectConfig = toml::from_str(
            "[project]
name = \"t\"
[board]
fqbn = \"x:y:z\"
",
        )
        .unwrap();
        let mut p = Project {
            path: Default::default(),
            config,
            logic: Logic::default(),
            layout: Default::default(),
        };
        p.logic.blocks.push(Block {
            id: "btn".into(),
            kind: "digital_in".into(),
            params: [("pin".to_owned(), ParamValue::Int(4))].into(),
            ..Default::default()
        });
        p.logic.blocks.push(Block {
            id: "not".into(),
            kind: "not".into(),
            ..Default::default()
        });
        p.layout.blocks.insert("btn".into(), at(0.0));
        p
    }

    #[test]
    fn copy_follows_tag() {
        let mut p = project();
        let pos = at(100.0);
        assert_eq!(p.add_tag_copy("btn", pos).unwrap(), "btn_2");
        assert_eq!(p.add_tag_copy("btn_2", pos).unwrap(), "btn_3");
        assert_eq!(p.logic.block("btn_3").unwrap().tag.as_deref(), Some("btn"));
        p.logic.blocks[0]
            .params
            .insert("pin".into(), ParamValue::Int(5));
        p.logic.sync_tags();
        assert_eq!(
            p.logic.block("btn_2").unwrap().params.get("pin"),
            Some(&ParamValue::Int(5))
        );
        assert_eq!(p.logic.tag_of("btn_2"), "btn");
        assert_eq!(p.logic.tag_copies("btn").count(), 2);
        let text = toml::to_string(&p.logic).unwrap();
        assert!(text.contains("tag = \"btn\""), "{text}");
        assert_eq!(toml::from_str::<Logic>(&text).unwrap(), p.logic);
    }

    #[test]
    fn removing_tag_keeps_it_in_place_of_copy() {
        let mut p = project();
        let pos = at(300.0);
        p.add_tag_copy("btn", pos).unwrap();
        p.logic.links.push(Link {
            from: PortRef::new("btn_2", "out"),
            to: PortRef::new("not", "in"),
        });
        p.remove_block("btn").unwrap();
        assert!(p.logic.block("btn_2").is_none());
        assert_eq!(p.logic.links[0].from, PortRef::new("btn", "out"));
        assert_eq!(p.layout.blocks["btn"].x, 300.0);
        // Without copies the tag goes away.
        p.remove_block("btn").unwrap();
        assert!(p.logic.block("btn").is_none());
    }

    #[test]
    fn taking_off_board_keeps_tag() {
        let mut p = project();
        p.add_tag_copy("btn", at(300.0)).unwrap();
        p.logic.links.push(Link {
            from: PortRef::new("btn", "out"),
            to: PortRef::new("not", "in"),
        });
        // The copy takes the tag's place, the tag's own links go.
        p.take_off_board("btn").unwrap();
        assert!(p.logic.block("btn_2").is_none());
        assert_eq!(p.layout.blocks["btn"].x, 300.0);
        assert!(p.logic.links.is_empty());
        // The last image leaves the sheet, the tag stays in the project.
        p.take_off_board("btn").unwrap();
        assert!(p.logic.block("btn").is_some());
        assert!(!p.layout.blocks.contains_key("btn"));
        p.add_tag_copy("btn", at(0.0)).unwrap();
        p.remove_tag("btn_2").unwrap();
        assert!(p.logic.block("btn").is_none());
        assert!(p.logic.block("btn_2").is_none());
    }

    #[test]
    fn rename_moves_copies() {
        let mut p = project();
        p.add_tag_copy("btn", at(0.0)).unwrap();
        p.rename_block("btn", "button").unwrap();
        assert_eq!(
            p.logic.block("btn_2").unwrap().tag.as_deref(),
            Some("button")
        );
    }
}
