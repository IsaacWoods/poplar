#![feature(
    let_chains,
    try_trait_v2,
    allocator_api,
    str_from_raw_parts,
    arbitrary_self_types_pointers,
    unsigned_signed_diff
)]

pub mod lex;
pub mod object;
pub mod parse;
pub mod vm;
