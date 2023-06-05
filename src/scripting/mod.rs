use std::env::VarError;
use std::path::PathBuf;

use log::debug;
use rhai::plugin::*;
use rhai::{EvalAltResult, Scope};

use crate::document::Document;

mod datetime;
mod document;
mod fetch;

fn env(s: &str) -> String {
    match std::env::var(s) {
        Ok(s) => s,
        Err(VarError::NotPresent) => String::new(),
        Err(VarError::NotUnicode(s)) => s.to_string_lossy().to_string(),
    }
}

pub struct Script {
    file: PathBuf,
    engine: rhai::Engine,
}

impl Script {
    pub fn new(file: PathBuf) -> Self {
        let mut engine = rhai::Engine::new();

        let datetime = exported_module!(datetime::datetime);
        let timedelta = exported_module!(datetime::timedelta);
        let alignment = exported_module!(document::alignment);
        engine.register_static_module("datetime", datetime.into());
        engine.register_static_module("timedelta", timedelta.into());
        engine.register_static_module("alignment", alignment.into());

        let document = exported_module!(document::globals);
        let fetch = exported_module!(fetch::globals);
        engine.register_global_module(document.into());
        engine.register_global_module(fetch.into());
        engine.register_fn("env", env);

        Script { file, engine }
    }

    pub fn run_with_document(&self, doc: Document) -> Result<Document, Box<EvalAltResult>> {
        const NAME: &str = "document";

        debug!("Running script: {:?}", self.file.to_string_lossy());

        let mut scope = Scope::new();
        scope.push(NAME, doc);
        self.engine.run_file_with_scope(&mut scope, self.file.to_path_buf())?;

        scope.get_value(NAME).ok_or("document invalidated".into())
    }
}
