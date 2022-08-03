//! Generating types from the opentype spec

use quote::quote;

mod error;
mod fields;
mod flags_enums;
mod parsing;
mod record;
mod table;

use parsing::{Item, Items};

pub use error::ErrorReport;

/// Codegeneration mode.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Generate parsing code
    Parse,
    /// Generate compilation code
    Compile,
}

pub fn generate_code(code_str: &str, mode: Mode) -> Result<String, syn::Error> {
    let tables = match mode {
        Mode::Parse => generate_parse_module(&code_str),
        Mode::Compile => generate_compile_module(&code_str),
    }?;
    // if this is not valid code just pass it through directly, and then we
    // can see the compiler errors
    let source_str = match rustfmt_wrapper::rustfmt(&tables) {
        Ok(s) => s,
        Err(_) => return Ok(tables.to_string()),
    };
    // convert doc comment attributes into normal doc comments
    let doc_comments = regex::Regex::new(r#"#\[doc = "(.*)"\]"#).unwrap();
    let source_str = doc_comments.replace_all(&source_str, "///$1");
    let newlines_before_docs = regex::Regex::new(r#"([;\}])\n( *)(///|pub|impl|#)"#).unwrap();
    let source_str = newlines_before_docs.replace_all(&source_str, "$1\n\n$2$3");

    // add newlines after top-level items
    let re2 = regex::Regex::new(r"\n\}").unwrap();
    let source_str = re2.replace_all(&source_str, "\n}\n\n");
    let source_str = rustfmt_wrapper::rustfmt(source_str).unwrap();

    Ok(format!(
        "\
    // THIS FILE IS AUTOGENERATED.\n\
    // Any changes to this file will be overwritten.\n\
    // For more information about how codegen works, see font-codegen/README.md\n\n{}",
        source_str,
    ))
}

pub fn generate_parse_module(code: &str) -> Result<proc_macro2::TokenStream, syn::Error> {
    let items: Items = syn::parse_str(code)?;
    items.sanity_check()?;
    let mut code = Vec::new();
    for item in &items.items {
        let item_code = match item {
            Item::Record(item) => record::generate(item)?,
            Item::Table(item) => table::generate(item)?,
            Item::Format(item) => table::generate_format_group(item)?,
            Item::RawEnum(item) => flags_enums::generate_raw_enum(&item),
            Item::Flags(item) => flags_enums::generate_flags(&item),
        };
        code.push(item_code);
    }

    Ok(quote! {
        #[allow(unused_imports)]
        use crate::parse_prelude::*;
        #(#code)*
    })
}

pub fn generate_compile_module(code: &str) -> Result<proc_macro2::TokenStream, syn::Error> {
    let items: Items = syn::parse_str(code)?;
    items.sanity_check()?;

    let code = items
        .items
        .iter()
        .map(|item| match item {
            Item::Record(item) => record::generate_compile(&item, &items.parse_module_path),
            Item::Table(item) => table::generate_compile(&item, &items.parse_module_path),
            Item::Format(item) => table::generate_format_compile(&item, &items.parse_module_path),
            Item::RawEnum(item) => Ok(flags_enums::generate_raw_enum_compile(&item)),
            Item::Flags(item) => Ok(flags_enums::generate_flags_compile(&item)),
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(quote! {
        #[allow(unused_imports)]
        use crate::compile_prelude::*;

        #( #code )*
    })
}

impl std::str::FromStr for Mode {
    type Err = miette::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "parse" => Ok(Self::Parse),
            "compile" => Ok(Self::Compile),
            other => Err(miette::Error::msg(format!(
                "expected one of 'parse' or 'compile' (found {other})"
            ))),
        }
    }
}
