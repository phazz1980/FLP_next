//! Tags over Modbus: the registers a slave project describes with its "Modbus read" and
//! "Modbus write" blocks (block id, table, value type, address), and the copies of them another
//! project keeps as "external tags". The master imports them by name from the slave's project
//! file, so no address is typed by hand.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::check::{DiagKind, Report, Severity};
use crate::{Logic, ParamValue, Project, VarType};

/// A tag of another project (`[[extern]]`): where it lives is taken from that project's file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalTag {
    pub name: String,
    /// The project file or folder the tag comes from, relative to this project's folder when
    /// possible.
    pub project: String,
    pub table: String,
    pub address: u16,
    #[serde(rename = "type")]
    pub ty: VarType,
}

/// A register of a project as another project sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedTag {
    pub name: String,
    pub table: String,
    pub address: u16,
    pub ty: VarType,
}

/// An MQTT topic a project publishes or subscribes to, as another project sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MqttTopic {
    pub topic: String,
    /// `string`, `int`, `float` or `bool`: the value type of a subscription; `string` for a
    /// publication, whose type comes from the connected output.
    pub kind: String,
    /// The source publishes it (otherwise it subscribes).
    pub published: bool,
}

/// What [`Logic::import_tags`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub added: Vec<String>,
    /// Tags whose address or type changed since the last transfer.
    pub updated: Vec<String>,
    /// Tags not taken, with the reason.
    pub conflicts: Vec<String>,
}

impl Logic {
    /// The registers this project serves: one tag per "Modbus read" / "Modbus write" block,
    /// named by the block id.
    pub fn published_tags(&self) -> Vec<PublishedTag> {
        self.blocks
            .iter()
            .filter_map(|b| {
                let default_table = match b.kind.as_str() {
                    "modbus_write" => "input",
                    "modbus_read" => "holding",
                    _ => return None,
                };
                let text = |name: &str, default: &str| match b.params.get(name) {
                    Some(ParamValue::Text(t)) => t.clone(),
                    _ => default.to_owned(),
                };
                let ty = match text("kind", "int").as_str() {
                    "bool" => VarType::Bool,
                    "float" => VarType::Float,
                    _ => VarType::Int,
                };
                let address = match b.params.get("reg") {
                    Some(ParamValue::Int(n)) => u16::try_from(*n).ok()?,
                    _ => 0,
                };
                Some(PublishedTag {
                    name: b.id.clone(),
                    table: text("table", default_table),
                    address,
                    ty,
                })
            })
            .collect()
    }

    /// The MQTT topics of this project, one per "MQTT publish" / "MQTT subscribe" block with a
    /// topic.
    pub fn mqtt_topics(&self) -> Vec<MqttTopic> {
        self.blocks
            .iter()
            .filter_map(|b| {
                let published = match b.kind.as_str() {
                    "mqtt_publish" => true,
                    "mqtt_subscribe" => false,
                    _ => return None,
                };
                let text = |name: &str| match b.params.get(name) {
                    Some(ParamValue::Text(t)) if !t.is_empty() => Some(t.clone()),
                    _ => None,
                };
                Some(MqttTopic {
                    topic: text("topic")?,
                    kind: if published {
                        "string".into()
                    } else {
                        text("kind").unwrap_or_else(|| "string".into())
                    },
                    published,
                })
            })
            .collect()
    }

    /// Settings of the MQTT client block (broker, port, login, ...): its parameters, or `None`
    /// when the project has no such block.
    pub fn mqtt_settings(&self) -> Option<BTreeMap<String, ParamValue>> {
        self.blocks
            .iter()
            .find(|b| b.kind == "mqtt")
            .map(|b| b.params.clone())
    }

    /// Takes tags of the project `source` as external tags. A tag whose name is a variable of
    /// this project, or an external tag of another project, is a conflict and is skipped; a tag
    /// taken before from the same project gets its address and type updated.
    pub fn import_tags(&mut self, source: &str, tags: &[PublishedTag]) -> ImportReport {
        let mut report = ImportReport::default();
        for tag in tags {
            if self.var(&tag.name).is_some() {
                report.conflicts.push(format!(
                    "«{}»: в проекте есть переменная с таким именем",
                    tag.name
                ));
                continue;
            }
            match self.externals.iter_mut().find(|e| e.name == tag.name) {
                Some(e) if e.project != source => report.conflicts.push(format!(
                    "«{}»: уже перенесён из проекта {}",
                    tag.name, e.project
                )),
                Some(e) => {
                    if e.table != tag.table || e.address != tag.address || e.ty != tag.ty {
                        e.table = tag.table.clone();
                        e.address = tag.address;
                        e.ty = tag.ty;
                        report.updated.push(tag.name.clone());
                    }
                }
                None => {
                    self.externals.push(ExternalTag {
                        name: tag.name.clone(),
                        project: source.to_owned(),
                        table: tag.table.clone(),
                        address: tag.address,
                        ty: tag.ty,
                    });
                    report.added.push(tag.name.clone());
                }
            }
        }
        report
    }
}

/// The path to store for `source`: relative to `base` (the folder of this project) when it
/// lies below it, with forward slashes.
pub fn stored_path(base: &Path, source: &Path) -> String {
    let rel = source.strip_prefix(base).unwrap_or(source);
    rel.to_string_lossy().replace('\\', "/")
}

/// Loads the published tags of the project at `source` (read only).
pub fn load_published(source: &Path) -> crate::Result<Vec<PublishedTag>> {
    Ok(Project::load(source)?.logic.published_tags())
}

/// Loads the MQTT topics of the project at `source` (read only).
pub fn load_mqtt_topics(source: &Path) -> crate::Result<Vec<MqttTopic>> {
    Ok(Project::load(source)?.logic.mqtt_topics())
}

/// Loads the MQTT client settings of the project at `source` (read only).
pub fn load_mqtt_settings(source: &Path) -> crate::Result<Option<BTreeMap<String, ParamValue>>> {
    Ok(Project::load(source)?.logic.mqtt_settings())
}

/// Checks the external tags against their projects: the project opens, the tag is still
/// published and its type is the same; a moved address is a warning (a new transfer fixes it).
pub fn check_externals(project: &Project, report: &mut Report) {
    for e in &project.logic.externals {
        let path: PathBuf = project.dir().join(&e.project);
        let found = match load_published(&path) {
            Ok(tags) => tags,
            Err(err) => {
                report.push(
                    DiagKind::ExternalTag,
                    Severity::Error,
                    None,
                    None,
                    format!(
                        "внешний тег «{}»: проект {} не открылся ({err})",
                        e.name, e.project
                    ),
                );
                continue;
            }
        };
        let (severity, message) = match found.iter().find(|t| t.name == e.name) {
            None => (
                Severity::Error,
                format!(
                    "внешний тег «{}»: в проекте {} его больше нет",
                    e.name, e.project
                ),
            ),
            Some(t) if t.ty != e.ty => (
                Severity::Error,
                format!(
                    "внешний тег «{}»: в проекте {} тип теперь «{}», а здесь «{}»",
                    e.name,
                    e.project,
                    t.ty.name(),
                    e.ty.name()
                ),
            ),
            Some(t) if t.table != e.table || t.address != e.address => (
                Severity::Warning,
                format!(
                    "внешний тег «{}»: адрес в проекте {} изменился, перенесите теги заново",
                    e.name, e.project
                ),
            ),
            Some(_) => continue,
        };
        report.push(DiagKind::ExternalTag, severity, None, None, message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Block;

    fn block(id: &str, kind: &str, params: &[(&str, ParamValue)]) -> Block {
        let mut b: Block = toml::from_str(&format!(
            "id = \"{id}\"
type = \"{kind}\""
        ))
        .unwrap();
        for (k, v) in params {
            b.params.insert((*k).into(), v.clone());
        }
        b
    }

    #[test]
    fn tags_come_from_modbus_blocks() {
        let mut l = Logic::default();
        l.blocks.push(block(
            "level",
            "modbus_write",
            &[
                ("kind", ParamValue::Text("float".into())),
                ("reg", ParamValue::Int(4)),
            ],
        ));
        l.blocks.push(block("cmd", "modbus_read", &[]));
        l.blocks.push(block("led", "digital_out", &[]));
        let tags = l.published_tags();
        assert_eq!(tags.len(), 2);
        assert_eq!(
            tags[0],
            PublishedTag {
                name: "level".into(),
                table: "input".into(),
                address: 4,
                ty: VarType::Float
            }
        );
        assert_eq!(tags[1].table, "holding");
    }

    #[test]
    fn mqtt_topics_come_from_mqtt_blocks() {
        let mut l = Logic::default();
        l.blocks.push(block(
            "out",
            "mqtt_publish",
            &[("topic", ParamValue::Text("home/t".into()))],
        ));
        l.blocks.push(block(
            "inp",
            "mqtt_subscribe",
            &[
                ("topic", ParamValue::Text("home/set".into())),
                ("kind", ParamValue::Text("float".into())),
            ],
        ));
        l.blocks.push(block("empty", "mqtt_publish", &[]));
        let t = l.mqtt_topics();
        assert_eq!(t.len(), 2);
        assert!(t[0].published && t[0].topic == "home/t");
        assert!(!t[1].published && t[1].kind == "float");
    }

    #[test]
    fn import_adds_updates_and_reports_conflicts() {
        let mut l = Logic::default();
        l.vars.push(crate::Var {
            name: "pump".into(),
            ty: VarType::Bool,
            init: None,
            description: String::new(),
            retain: false,
            service: false,
        });
        let tag = |n: &str, a: u16| PublishedTag {
            name: n.into(),
            table: "holding".into(),
            address: a,
            ty: VarType::Int,
        };
        let r = l.import_tags("slave", &[tag("t1", 0), tag("pump", 1)]);
        assert_eq!(r.added, ["t1"]);
        assert_eq!(r.conflicts.len(), 1);
        let r = l.import_tags("slave", &[tag("t1", 5)]);
        assert_eq!(r.updated, ["t1"]);
        assert_eq!(l.externals[0].address, 5);
        let r = l.import_tags("other", &[tag("t1", 5)]);
        assert_eq!(r.conflicts.len(), 1);
    }
}
