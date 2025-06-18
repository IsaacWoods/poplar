#![no_std]

extern crate alloc;

use alloc::{string::String, vec::Vec};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct SeedConfig {
    pub user_tasks: Vec<String>,
}
