#![feature(allocator_api)]
#![allow(clippy::redundant_closure_call)]

#[macro_use]
extern crate scoped_tls;

macro_rules! dbg_each {
    ($($val:expr),+) => {
        (|| {
            $(
                dbg!($val);
            )+
        }) ()
    };
}

#[macro_use]
pub mod report;
pub mod benchmark;
pub mod cli;
pub mod datastr;
pub mod export;
pub mod graph;
pub mod import;
pub mod io;
pub mod link_speed_estimates;
pub mod shortest_path;

mod as_mut_slice;
mod as_slice;
mod in_range_option;
mod math;
mod sorted_search_slice_ext;
mod util;

// Use of a mod or pub mod is not actually necessary.
pub mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}
