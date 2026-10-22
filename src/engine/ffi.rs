//! Bindings geradas pelo bindgen a partir dos cabeçalhos do libmlt.
//!
//! Os lints de estilo C ficam liberados só neste módulo: o código gerado não
//! segue as convenções de nomenclatura do Rust.
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code)]
#![allow(clippy::all, clippy::pedantic)]

include!(concat!(env!("OUT_DIR"), "/mlt_bindings.rs"));
