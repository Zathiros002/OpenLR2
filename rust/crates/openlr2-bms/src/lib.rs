//! openlr2-bms — BMS/PMS chart parser.
//!
//! Parses `.bms` / `.bme` / `.pms` files and produces a `Chart` struct.
//!
//! ## Migration source
//!
//! The C++ reference implementation lives in:
//! - `LR2/LR2_bmsload.cpp`  — `ParseBmsFile()`, BPM timeline, lane splitting
//! - `LR2/structure.h`      — `gameplay`, `LaneStruct`, `NoteStruct`, `BPMtiming`
//!
//! ## Supported features (planned)
//!
//! - [ ] `#PLAYER` / `#GENRE` / `#TITLE` / `#ARTIST` / `#BPM` headers
//! - [ ] `#WAVxx` / `#BMPxx` resource tables (Base36 / Base62)
//! - [ ] Channel data (`#xxxnn`): BGM, BPM changes, visible/invisible notes, LN, mines
//! - [ ] `#STOPxx` / `#STPxx`
//! - [ ] `#LNTYPE` / `#LNOBJ`
//! - [ ] `#RANDOM` / `#IF`
//! - [ ] BPM timeline construction (BPM changes + stops → real-time offsets)
//! - [ ] Lane splitting (SP ↔ DP, scratch side detection)
//! - [ ] Chart hash computation (MD5 over sorted note data)
//! - [ ] CP932 / UTF-8 path resolution for `#WAV` / `#BMP` resources

pub mod chart;
pub mod parser;
pub mod timing;

pub use chart::BmsParseOptions;
pub use chart::{BmsMeta, Chart, Lane, Note, NoteKind, ResourceRef};
pub use timing::{BpmEvent, StopEvent, TimingEvent, TimingEventKind};
