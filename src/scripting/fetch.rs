use std::any::Any;

use log::debug;
use rhai::plugin::*;
use rhai::{self, Dynamic, EvalAltResult, ImmutableString, NativeCallContext};

trait Extract {
    fn extract<T: Any>(&mut self, key: &str) -> Result<Option<T>, Box<EvalAltResult>>;
}

impl Extract for rhai::Map {
    fn extract<T: Any>(&mut self, key: &str) -> Result<Option<T>, Box<EvalAltResult>> {
        if let Some(val) = self.remove(key) {
            let val_type = val.type_name();
            match val.try_cast() {
                Some(s) => return Ok(Some(s)),
                None => {
                    return Err(Box::new(EvalAltResult::ErrorMismatchDataType(
                        std::any::type_name::<T>().to_string(),
                        val_type.to_string(),
                        Position::NONE,
                    )))
                }
            }
        }
        Ok(None)
    }
}

#[export_module]
pub mod globals {
    #[rhai_fn(name = "fetch", return_raw, global)]
    pub fn fetch(context: NativeCallContext, path: &str) -> Result<Dynamic, Box<EvalAltResult>> {
        fetch_with_opts(context, path, rhai::Map::new())
    }

    #[rhai_fn(name = "fetch", return_raw, global)]
    pub fn fetch_with_opts(
        context: NativeCallContext,
        path: &str,
        mut opts: rhai::Map,
    ) -> Result<Dynamic, Box<EvalAltResult>> {
        let mut req = match opts.extract::<ImmutableString>("method")? {
            Some(method) => ureq::request(&method, path),
            None => ureq::get(path),
        };

        if let Some(headers) = opts.extract::<rhai::Map>("headers")? {
            for (k, v) in headers.into_iter() {
                req = req.set(&k, &*v.into_immutable_string()?);
            }
        }

        debug!("Fetching {:?}", path);
        let resp = match opts.extract::<ImmutableString>("data")? {
            Some(data) => req.send_string(&data),
            None => req.call(),
        }
        .map_err(|e| e.to_string())?;

        let status = Dynamic::from_int(resp.status() as i64);
        let status_text = Dynamic::from(resp.status_text().to_string());
        let str = resp.into_string().map_err(|e| e.to_string())?;
        match opts
            .extract::<ImmutableString>("response_type")?
            .as_ref()
            .map(|s| s.as_str())
        {
            Some("json") | None => context.engine().parse_json(str, true).map(Dynamic::from),
            Some("string") => Ok(Dynamic::from(str)),
            Some("status") => {
                let mut result = rhai::Map::new();
                result.insert("status".into(), status);
                result.insert("status_text".into(), status_text);
                result.insert("data".into(), Dynamic::from(str));
                Ok(Dynamic::from_map(result))
            }
            Some(r) => Err(format!("response_type not supported: {}", r).into()),
        }
    }
}
