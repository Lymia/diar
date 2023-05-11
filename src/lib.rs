#![deny(unused_must_use)]

#[macro_use]
extern crate tracing;

pub mod compress;
mod errors;
mod names;

pub use errors::*;
