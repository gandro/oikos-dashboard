use std::{env, fs, path::PathBuf};

use quote::ToTokens;
use syn::parse_quote;

struct KeyCodes {
    idents: Vec<syn::Ident>,
    codes: Vec<syn::Expr>,
    count: syn::Expr,
}

fn parse_event_codes(header: &str) -> KeyCodes {
    let bindings = bindgen::builder()
        .header(header)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .allowlist_var("(KEY|BTN)_.*")
        .blocklist_item("(KEY)_(RESERVED|MIN_INTERESTING|MAX)")
        .disable_header_comment()
        .generate()
        .expect("Unable to parse input event codes header");

    let f = syn::parse_file(&bindings.to_string()).expect("Failed to parse bindgen output");
    let mut idents = vec![];
    let mut codes = vec![];
    let mut count = None;
    for item in f.items {
        let syn::Item::Const(c) = item else {
            panic!("Failed to parse unexpected bindgen item: {}", item.to_token_stream())
        };

        if c.ident == "KEY_CNT" {
            count = Some(*c.expr);
        } else {
            idents.push(c.ident);
            codes.push(*c.expr);
        }
    }
    let count = count.expect("KEY_CNT not found");

    KeyCodes { idents, codes, count }
}

fn generate_file(keycodes: KeyCodes) -> syn::File {
    let KeyCodes { idents, codes, count } = keycodes;
    let names: Vec<String> = idents.iter().map(|i| i.to_string()).collect();
    parse_quote!(
        use std::fmt;
        use std::str::FromStr;

        #[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
        pub struct KeyCode(pub u16);

        impl KeyCode {
            pub const COUNT: usize = #count;

            #(
                #[allow(dead_code)]
                pub const #idents: KeyCode = KeyCode(#codes);
            )*

            pub fn code(&self) -> u16 {
                self.0
            }
        }

        impl From<u16> for KeyCode {
            fn from(code: u16) -> Self {
                KeyCode(code)
            }
        }

        #[derive(Debug, Copy, Clone)]
        pub struct ParseKeyCodeError;

        impl fmt::Display for ParseKeyCodeError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "Unknown key code name or invalid numeric code")
            }
        }

        impl std::error::Error for ParseKeyCodeError {}

        impl FromStr for KeyCode {
            type Err = ParseKeyCodeError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    #(#names => Ok(KeyCode(#codes)),)*
                    c => Ok(KeyCode(c.parse().map_err(|_| ParseKeyCodeError)?)),
                }
            }
        }

        impl fmt::Display for KeyCode {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                #[allow(unreachable_patterns)]
                match self.0 {
                    #(#codes => write!(f, #names),)*
                    c => write!(f, "KEY_UNKNOWN({})", c),
                }
            }
        }
    )
}

fn main() {
    let keycodes = parse_event_codes("include/linux/input-event-codes.h");
    let f = generate_file(keycodes);

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Missing OUT_DIR env variable"));
    fs::write(out_dir.join("input-event-codes.rs"), prettyplease::unparse(&f))
        .expect("Failed to write input-event-codes.rs");
}
