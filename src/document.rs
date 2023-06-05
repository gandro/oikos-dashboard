use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::{io, vec};

use elementtree::Element;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] io::Error),
    #[error("XML error")]
    Xml(#[from] elementtree::Error),
    #[error("Selection has been invalidated")]
    SelectionInvalidated,
    #[error("Bug: encountered node without id attribute")]
    UnlabeledNode,
    #[error("Encountered node with duplicated id attribute: `{0}`")]
    DuplicatedId(String),
    #[error("Operation not supported on empty selection")]
    EmptySelection,
    #[error("Operation can only be performed on singelton selection")]
    SingletonRequired,
}

#[derive(Clone, Debug)]
struct Path {
    path: Vec<usize>,
    target: String,
}

impl Path {
    fn new(path: Vec<usize>, target: String) -> Self {
        Path { path, target }
    }

    fn push_child(&self, idx: usize, child: &Element) -> Result<Self, Error> {
        let target = child.get_attr("id").ok_or(Error::UnlabeledNode)?.to_owned();

        let mut path = self.path.clone();
        path.push(idx);

        Ok(Path { path, target })
    }

    fn resolve_in<'a>(&self, root: &'a Element) -> Result<&'a Element, Error> {
        let mut node = root;
        for idx in &self.path {
            node = node.get_child(*idx).ok_or(Error::SelectionInvalidated)?;
        }

        match node.get_attr("id") {
            Some(target) if target == self.target => Ok(node),
            Some(_) => Err(Error::SelectionInvalidated),
            None => Err(Error::UnlabeledNode),
        }
    }

    fn resolve_in_mut<'a>(&self, root: &'a mut Element) -> Result<&'a mut Element, Error> {
        let mut node = root;
        for idx in &self.path {
            node = node.get_child_mut(*idx).ok_or(Error::SelectionInvalidated)?;
        }

        match node.get_attr("id") {
            Some(target) if target == self.target => Ok(node),
            Some(_) => Err(Error::SelectionInvalidated),
            None => Err(Error::UnlabeledNode),
        }
    }
}

pub type ElementId = String;

#[derive(Debug, Clone)]
pub struct Operation {
    pub target: ElementId,
    pub args: Arguments,
}

#[derive(Debug, Clone)]
pub enum Arguments {
    Rotation(Rotation),
    Alignment(Alignment),
}

#[derive(Debug, Clone)]
pub struct Alignment {
    pub orientation: Orientation,
    pub relative_to: Option<ElementId>,
}

#[derive(Copy, Clone, Debug)]
pub enum Horizontal {
    Left,
    Center,
    Right,
}

#[derive(Copy, Clone, Debug)]
pub enum Vertical {
    Top,
    Middle,
    Bottom,
}

#[derive(Debug, Copy, Clone)]
pub enum Orientation {
    Horizontal(Horizontal),
    Vertical(Vertical),
}

#[derive(Debug, Clone)]
pub struct Rotation {
    pub angle: f64,
    pub center: Option<ElementId>,
}

#[derive(Debug)]
struct Shared {
    root: Element,
    ops: Vec<Operation>,
}

#[derive(Clone, Debug)]
pub struct Document {
    shared: Rc<RefCell<Shared>>,
    selection: Rc<Vec<Path>>,
}

fn label_nodes<'root>(root: &'root mut Element) -> Result<String, Error> {
    // first pass: collect all known element ids and detect duplicates
    let mut known_ids = HashSet::<String>::new();
    let mut queue = vec![&*root];
    while let Some(node) = queue.pop() {
        if let Some(id) = node.get_attr("id") {
            if let Some(conflict) = known_ids.take(id) {
                return Err(Error::DuplicatedId(conflict.to_owned()));
            }
            known_ids.insert(id.to_string());
        }

        for child in node.children() {
            queue.push(child);
        }
    }

    // second pass: assign randomly generated id to any unlabeled nodes
    let mut queue = vec![&mut *root];
    while let Some(node) = queue.pop() {
        if node.get_attr("id").is_none() {
            let id = loop {
                let id = format!("id{}", rand::random::<u32>());
                if known_ids.insert(id.clone()) {
                    break id;
                }
            };
            node.set_attr("id", id);
        }

        for child in node.children_mut() {
            queue.push(child);
        }
    }

    root.get_attr("id").map(String::from).ok_or(Error::UnlabeledNode)
}

impl Document {
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, Error> {
        Document::new(Element::from_reader(io::Cursor::new(bytes))?)
    }

    fn new(mut root: Element) -> Result<Self, Error> {
        let root_label = label_nodes(&mut root)?;
        let root_selection = Path::new(vec![], root_label);
        Ok(Document {
            shared: Rc::new(RefCell::new(Shared {
                root: root,
                ops: Vec::new(),
            })),
            selection: Rc::new(vec![root_selection]),
        })
    }

    pub fn select_by_attr(&self, key: &str, value: &str) -> Result<Self, Error> {
        let path = self.select_nodes(|n| n.get_attr(key).map(|a| a == value).unwrap_or(false))?;
        Ok(Document {
            shared: self.shared.clone(),
            selection: Rc::new(path),
        })
    }

    pub fn text(&self, s: &str) -> Result<(), Error> {
        let mut shared = self.shared.borrow_mut();
        for node in &*self.selection {
            let elem = node.resolve_in_mut(&mut shared.root)?;
            elem.retain_children(|_| false);
            elem.set_text(s);
        }
        Ok(())
    }

    pub fn attr(&self, key: &str, value: &str) -> Result<(), Error> {
        let mut shared = self.shared.borrow_mut();
        for node in &*self.selection {
            node.resolve_in_mut(&mut shared.root)?.set_attr(key, value);
        }
        Ok(())
    }

    pub fn push_alignment(&self, orientation: Orientation, relative_to: Option<&Document>) -> Result<(), Error> {
        if self.selection.is_empty() {
            return Ok(());
        }

        let relative_to = match relative_to.map(|doc| doc.selection.as_slice()) {
            Some(&[]) => return Err(Error::EmptySelection),
            Some(&[ref s]) => Some(&s.target),
            Some(_) => return Err(Error::SingletonRequired),
            None => None,
        };

        let mut shared = self.shared.borrow_mut();
        for node in &*self.selection {
            shared.ops.push(Operation {
                target: node.target.to_owned(),
                args: Arguments::Alignment(Alignment {
                    orientation: orientation,
                    relative_to: relative_to.cloned(),
                }),
            });
        }

        Ok(())
    }

    pub fn push_rotation(&self, angle: f64, center: Option<&Document>) -> Result<(), Error> {
        if self.selection.is_empty() {
            return Ok(());
        }

        let center = match center.map(|doc| doc.selection.as_slice()) {
            Some(&[]) => return Err(Error::EmptySelection),
            Some(&[ref s]) => Some(&s.target),
            Some(_) => return Err(Error::SingletonRequired),
            None => None,
        };

        let mut shared = self.shared.borrow_mut();
        for node in &*self.selection {
            shared.ops.push(Operation {
                target: node.target.to_owned(),
                args: Arguments::Rotation(Rotation {
                    angle: angle,
                    center: center.cloned(),
                }),
            });
        }

        Ok(())
    }

    fn select_nodes(&self, predicate: impl Fn(&Element) -> bool) -> Result<Vec<Path>, Error> {
        let mut result: Vec<Path> = Vec::new();

        let shared = self.shared.borrow();
        for path in &*self.selection {
            let source = path.resolve_in(&shared.root)?;

            let mut stack: Vec<(&Element, Path)> = Vec::new();
            stack.push((&source, path.clone()));

            while !stack.is_empty() {
                let (node, path) = stack.pop().unwrap();

                if predicate(node) {
                    result.push(path.clone());
                }

                for (idx, child) in node.children().enumerate() {
                    let path = path.push_child(idx, child)?;
                    stack.push((&child, path));
                }
            }
        }

        Ok(result)
    }

    pub fn prepare(&self) -> Result<(Vec<u8>, Vec<Operation>), Error> {
        let mut buf = Vec::new();

        self.shared.borrow().root.to_writer_with_options(
            &mut buf,
            elementtree::WriteOptions::new()
                .set_autopad_comments(false)
                .set_line_separator("")
                .set_perform_indent(false),
        )?;

        let ops = self.shared.borrow().ops.clone();

        Ok((buf, ops))
    }
}
