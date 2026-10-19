//! Minimal attribute-only XML tree for the job file. Text nodes are ignored (the format
//! keeps all data in attributes). The `Employees` subtree is skipped without reading its
//! attributes, so credentials never enter memory.

use std::collections::BTreeMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::OstError;

/// Element names whose whole subtree is dropped unread.
const SKIPPED: &[&str] = &["Employees"];

#[derive(Debug, Default)]
pub struct Node {
    pub name: String,
    pub attrs: BTreeMap<String, String>,
    pub children: Vec<Node>,
}

impl Node {
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |c| c.name == name)
    }
}

fn open(e: &BytesStart) -> Result<Node, OstError> {
    let mut node = Node {
        name: String::from_utf8_lossy(e.name().as_ref()).into_owned(),
        ..Node::default()
    };
    for a in e.attributes() {
        let a = a.map_err(|err| OstError::Xml(err.to_string()))?;
        let key = String::from_utf8_lossy(a.key.as_ref()).into_owned();
        // Lossy decode: the job file may hold stray non-UTF-8 bytes in free-text fields.
        let raw = String::from_utf8_lossy(&a.value).into_owned();
        let val = quick_xml::escape::unescape(&raw)
            .map_err(|err| OstError::Xml(err.to_string()))?
            .into_owned();
        node.attrs.insert(key, val);
    }
    Ok(node)
}

pub fn parse(bytes: &[u8]) -> Result<Node, OstError> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let mut reader = Reader::from_reader(bytes);
    let mut buf = Vec::new();
    let mut skip_buf = Vec::new();
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    loop {
        let ev = reader
            .read_event_into(&mut buf)
            .map_err(|e| OstError::Xml(e.to_string()))?;
        match ev {
            Event::Start(e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if SKIPPED.contains(&name.as_str()) {
                    reader
                        .read_to_end_into(e.name(), &mut skip_buf)
                        .map_err(|err| OstError::Xml(err.to_string()))?;
                } else {
                    stack.push(open(&e)?);
                }
            }
            Event::Empty(e) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if !SKIPPED.contains(&name.as_str()) {
                    attach(&mut stack, &mut root, open(&e)?);
                }
            }
            Event::End(_) => {
                let node = stack
                    .pop()
                    .ok_or_else(|| OstError::Xml("unbalanced end tag".into()))?;
                attach(&mut stack, &mut root, node);
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    if !stack.is_empty() {
        return Err(OstError::Xml("unexpected end of file".into()));
    }
    root.ok_or_else(|| OstError::Xml("no root element".into()))
}

fn attach(stack: &mut [Node], root: &mut Option<Node>, node: Node) {
    match stack.last_mut() {
        Some(parent) => parent.children.push(node),
        None => *root = Some(node),
    }
}
