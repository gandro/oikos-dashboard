use rhai::plugin::*;

use crate::document::{Document, Horizontal, Orientation, Vertical};

#[export_module]
pub mod alignment {
    pub const LEFT: Horizontal = Horizontal::Left;
    pub const CENTER: Horizontal = Horizontal::Center;
    pub const RIGHT: Horizontal = Horizontal::Right;
    pub const TOP: Vertical = Vertical::Top;
    pub const MIDDLE: Vertical = Vertical::Middle;
    pub const BOTTOM: Vertical = Vertical::Bottom;
}

#[export_module]
pub mod globals {
    pub type Document = super::Document;

    #[rhai_fn(return_raw, global)]
    pub fn id(doc: &mut Document, id: &str) -> Result<Document, Box<EvalAltResult>> {
        doc.select_by_attr("id", id).map_err(|e| e.to_string().into())
    }

    #[rhai_fn(return_raw, global)]
    pub fn class(doc: &mut Document, class: &str) -> Result<Document, Box<EvalAltResult>> {
        doc.select_by_attr("class", class).map_err(|e| e.to_string().into())
    }

    #[rhai_fn(return_raw, global)]
    pub fn text(doc: &mut Document, text: &str) -> Result<Document, Box<EvalAltResult>> {
        doc.text(text).map_err(|e| e.to_string())?;
        Ok(doc.clone())
    }

    #[rhai_fn(return_raw, global)]
    pub fn visible(doc: &mut Document, visible: bool) -> Result<Document, Box<EvalAltResult>> {
        doc.attr(
            "visibility",
            match visible {
                true => "visible",
                false => "hidden",
            },
        )
        .map_err(|e| e.to_string())?;
        Ok(doc.clone())
    }

    #[rhai_fn(name = "align", return_raw, global)]
    pub fn align_horizontal(doc: &mut Document, horizontal: Horizontal) -> Result<Document, Box<EvalAltResult>> {
        doc.push_alignment(Orientation::Horizontal(horizontal), None)
            .map_err(|e| e.to_string())?;

        Ok(doc.clone())
    }

    #[rhai_fn(name = "align", return_raw, global)]
    pub fn align_vertical(doc: &mut Document, vertical: Vertical) -> Result<Document, Box<EvalAltResult>> {
        doc.push_alignment(Orientation::Vertical(vertical), None)
            .map_err(|e| e.to_string())?;

        Ok(doc.clone())
    }

    #[rhai_fn(return_raw, global)]
    pub fn align(
        doc: &mut Document,
        horizontal: Horizontal,
        vertical: Vertical,
    ) -> Result<Document, Box<EvalAltResult>> {
        doc.push_alignment(Orientation::Horizontal(horizontal), None)
            .map_err(|e| e.to_string())?;
        doc.push_alignment(Orientation::Vertical(vertical), None)
            .map_err(|e| e.to_string())?;

        Ok(doc.clone())
    }

    #[rhai_fn(name = "align_with", return_raw, global)]
    pub fn align_with_horizontal(
        doc: &mut Document,
        relative_to: Document,
        horizontal: Horizontal,
    ) -> Result<Document, Box<EvalAltResult>> {
        doc.push_alignment(Orientation::Horizontal(horizontal), Some(&relative_to))
            .map_err(|e| e.to_string())?;

        Ok(doc.clone())
    }

    #[rhai_fn(name = "align_with", return_raw, global)]
    pub fn align_with_vertical(
        doc: &mut Document,
        relative_to: Document,
        vertical: Vertical,
    ) -> Result<Document, Box<EvalAltResult>> {
        doc.push_alignment(Orientation::Vertical(vertical), Some(&relative_to))
            .map_err(|e| e.to_string())?;

        Ok(doc.clone())
    }

    #[rhai_fn(return_raw, global)]
    pub fn align_with(
        doc: &mut Document,
        relative_to: Document,
        horizontal: Horizontal,
        vertical: Vertical,
    ) -> Result<Document, Box<EvalAltResult>> {
        doc.push_alignment(Orientation::Horizontal(horizontal), Some(&relative_to))
            .map_err(|e| e.to_string())?;
        doc.push_alignment(Orientation::Vertical(vertical), Some(&relative_to))
            .map_err(|e| e.to_string())?;

        Ok(doc.clone())
    }

    #[rhai_fn(return_raw, global)]
    pub fn rotate(doc: &mut Document, angle: f64) -> Result<Document, Box<EvalAltResult>> {
        doc.push_rotation(angle, None).map_err(|e| e.to_string())?;

        Ok(doc.clone())
    }

    #[rhai_fn(return_raw, global)]
    pub fn rotate_at(doc: &mut Document, center: Document, angle: f64) -> Result<Document, Box<EvalAltResult>> {
        doc.push_rotation(angle, Some(&center)).map_err(|e| e.to_string())?;

        Ok(doc.clone())
    }
}
