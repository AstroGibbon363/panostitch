//! PanoStitch: panorama stitching (`stitcher`) and star stacking (`stacker`)
//! for landscape astrophotography. The window lives in the `gui` module and
//! the `panostitch` binary; `panostack` stacks from the command line.

pub mod image;
pub mod io;
pub mod stitcher;
pub mod stacker;
pub mod gui;

pub use image::{Gray, Img, Rgb};
