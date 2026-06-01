//! openlr2-core — foundational types shared across the OpenLR2 Rust runtime.
//!
//! This crate defines:
//! - Error types (`OpenLr2Error`)
//! - Time model (`Millis`, `Micros`, `Beat`, `BmsTime`)
//! - Key mode enum (`KeyMode`)
//! - Resource handle types (opaque newtypes)
//! - Encoding utilities (CP932 ↔ UTF-8)

pub mod encoding;
pub mod error;
pub mod keymode;
pub mod resource;
pub mod time;

pub use error::OpenLr2Error;
pub use keymode::KeyMode;
pub use resource::{FontId, SoundId, TextureId};
pub use time::{Beat, BmsTime, Micros, Millis};
