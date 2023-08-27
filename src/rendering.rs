use std::path::PathBuf;

use log::debug;
use tiny_skia::IntSize;
use tiny_skia::Pixmap;
use usvg::fontdb;
use usvg::{NodeExt, NodeKind, Transform, TreeParsing, TreeTextToPath};

use crate::document::{self, Alignment, Arguments, Document, Horizontal, Orientation, Rotation, Vertical};

fn align(target: &usvg::Node, alignment: Alignment, tree: &mut usvg::Tree) -> Option<Transform> {
    let anchor = match alignment.relative_to {
        Some(relative_to) => tree.node_by_id(&relative_to)?,
        None => tree.root.clone(),
    };

    let target = target.calculate_bbox()?;
    let anchor = anchor.calculate_bbox()?;

    match alignment.orientation {
        Orientation::Horizontal(horizontal) => {
            let x = anchor.x() - target.x();
            let offset = match horizontal {
                Horizontal::Left => 0.,
                Horizontal::Center => (anchor.width() - target.width()) / 2.,
                Horizontal::Right => anchor.width() - target.width(),
            };
            Some(Transform::from_translate(x + offset, 0.))
        }
        Orientation::Vertical(vertical) => {
            let y = anchor.y() - target.y();
            let offset = match vertical {
                Vertical::Top => 0.,
                Vertical::Middle => (anchor.height() - target.height()) / 2.,
                Vertical::Bottom => anchor.height() - target.height(),
            };
            Some(Transform::from_translate(0., y + offset))
        }
    }
}

fn rotate(target: &usvg::Node, rotation: Rotation, tree: &mut usvg::Tree) -> Option<Transform> {
    let center = match rotation.center {
        Some(center) => tree.node_by_id(&center)?,
        None => target.clone(),
    };

    // rotation coordinates are absolute, correct for parent transformations:
    let abs = target.abs_transform();
    let (origin_x, origin_y) = (abs.tx, abs.ty);

    let center = center.calculate_bbox()?;
    let x = center.x() + center.width() / 2. - origin_x;
    let y = center.y() + center.height() / 2. - origin_y;

    Some(Transform::from_rotate_at(rotation.angle as f32, x, y))
}

pub fn perform(op: document::Operation, tree: &mut usvg::Tree) {
    let Some(target) = tree.node_by_id(&op.target) else {
        return;
    };

    let transform = match op.args {
        Arguments::Alignment(a) => align(&target, a, tree),
        Arguments::Rotation(r) => rotate(&target, r, tree),
    };

    if let Some(transform) = transform {
        let mut target = target.borrow_mut();
        let t = match *target {
            NodeKind::Group(ref mut e) => &mut e.transform,
            NodeKind::Path(ref mut e) => &mut e.transform,
            NodeKind::Image(ref mut e) => &mut e.transform,
            NodeKind::Text(ref mut e) => &mut e.transform,
        };
        *t = t.post_concat(transform);
    }
}

#[derive(Clone, Debug, Default)]
pub struct Configuration {
    pub base_dir: Option<PathBuf>,
    pub resources_dir: Option<PathBuf>,
    pub fonts_dir: Option<PathBuf>,
    pub system_fonts: bool,
    pub screen_size: Option<(u32, u32)>,
}

pub struct Renderer {
    opts: usvg::Options,
    fonts: fontdb::Database,
    screen_size: Option<IntSize>,
}

impl Renderer {
    pub fn from_config(c: Configuration) -> Renderer {
        let screen_size = c.screen_size.and_then(|(x, y)| IntSize::from_wh(x, y));

        let mut fonts = fontdb::Database::new();
        if c.system_fonts {
            fonts.load_system_fonts();
        }
        if let Some(fonts_dir) = c.fonts_dir.as_ref().or(c.base_dir.as_ref()) {
            fonts.load_fonts_dir(fonts_dir);
        }

        let opts = usvg::Options {
            resources_dir: c.resources_dir.or(c.base_dir),
            ..usvg::Options::default()
        };

        Renderer {
            opts,
            fonts,
            screen_size,
        }
    }

    pub fn render(&self, doc: Document) -> Result<Pixmap, anyhow::Error> {
        let rtree = {
            let (svg_data, operations) = doc.prepare()?;
            debug!("Rendering document with {} queued operations", operations.len());
            let mut tree = usvg::Tree::from_data(&svg_data, &self.opts)?;

            tree.convert_text(&self.fonts);

            for op in operations {
                perform(op, &mut tree);
            }

            resvg::Tree::from_usvg(&tree)
        };
        let pixmap_size = match self.screen_size {
            Some(size) => size,
            None => rtree.size.to_int_size(),
        };
        let mut pixmap =
            tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height()).expect("invalid bitmap size");
        rtree.render(Transform::default(), &mut pixmap.as_mut());
        Ok(pixmap)
    }
}
