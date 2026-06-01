use openlr2_core::{Beat, KeyMode, Millis, OpenLr2Error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::timing::TimingEvent;

/// Parsed BMS chart with all note data and metadata.
///
/// Maps to: `gameplay` struct (partial) in `structure.h` after `ParseBmsFile()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chart {
    pub metadata: BmsMeta,
    pub keymode: KeyMode,
    pub lanes: Vec<Lane>,
    pub timing: Vec<TimingEvent>,
    pub resources: ResourceTable,
    pub total_notes: u32,
    pub total_play_time: Millis,
}

/// BMS metadata parsed from `#TITLE`, `#ARTIST`, etc. headers.
///
/// Maps to: `BMSMETA` struct in `structure.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BmsMeta {
    pub title: String,
    pub subtitle: String,
    pub artist: String,
    pub subartist: String,
    pub genre: String,
    /// MD5 hash of the BMS file content (used as primary song identifier).
    pub hash: String,
    pub filepath: PathBuf,
    pub stagefile: Option<PathBuf>,
    pub banner: Option<PathBuf>,
    pub back_bmp: Option<PathBuf>,
    pub max_bpm: f64,
    pub min_bpm: f64,
    pub level: Option<u32>,
    pub difficulty: Option<u32>,
    pub has_long_note: bool,
    pub has_random: bool,
}

/// A single lane (column) of notes.
///
/// Maps to: `LaneStruct` in `structure.h` (one element of `bmsobj_note[]`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lane {
    /// Lane index (0-based, up to `keymode.total_lanes() - 1`).
    pub index: usize,
    /// Sorted list of notes in this lane (by BMS time).
    pub notes: Vec<Note>,
}

/// A single note in a lane.
///
/// Maps to: `NoteStruct` in `structure.h`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub kind: NoteKind,
    /// BMS measure-based time.
    pub bms_time: Beat,
    /// Wall-clock time (milliseconds from song start), computed from BPM timeline.
    pub real_time: Millis,
    /// Key sound resource index (into `ResourceTable::wav`), if any.
    pub keysound_id: Option<usize>,
    /// BGA/BMP resource index, if any (for BGA channel notes).
    pub bga_id: Option<usize>,
}

/// The kind of a note.
///
/// Maps to the differentiation in `ProcSinglenote()` / `ProcLongnote()`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum NoteKind {
    /// Standard tap note.
    Tap,
    /// Start of a long note.
    LongStart {
        /// Real time when the long note ends (computed from the paired `#LNOBJ` end).
        end_time: Millis,
    },
    /// End of a long note.
    LongEnd,
    /// Landmine note — hitting it deducts HP.
    Mine { damage: f64 },
    /// Background music note (no player interaction required).
    Bgm,
    /// BGA change note.
    Bga,
}

/// Points to a resource declared in the BMS `#WAVxx` / `#BMPxx` tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRef {
    /// Index in the resource table (Base36 or Base62 decoded).
    pub index: usize,
    /// File path relative to the BMS file's directory.
    pub path: String,
}

/// Resource table — all `#WAV` and `#BMP` declarations.
///
/// Maps to: `keysound[SLOTS]` and BGA handles in the C++ code.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceTable {
    /// Key sound resources (`#WAVxx`), indexed by Base36/Base62 index.
    pub wav: HashMap<usize, ResourceRef>,
    /// Bitmap/BGA resources (`#BMPxx`), indexed by Base36/Base62 index.
    pub bmp: HashMap<usize, ResourceRef>,
}

/// Options controlling BMS parsing behavior.
#[derive(Debug, Clone)]
pub struct BmsParseOptions {
    /// If true, attempt CP932 decoding for non-UTF-8 headers.
    pub try_cp932: bool,
    /// If true, load and parse BGA declarations.
    pub parse_bga: bool,
    /// Scratch side preference (0 = left, 1 = right, 2 = auto-detect).
    pub scratch_side: u8,
}

impl Default for BmsParseOptions {
    fn default() -> Self {
        Self {
            try_cp932: true,
            parse_bga: false,
            scratch_side: 2, // auto
        }
    }
}

impl Chart {
    /// Parse a BMS file from the given path.
    pub fn from_file(
        path: impl AsRef<std::path::Path>,
        options: BmsParseOptions,
    ) -> Result<Self, OpenLr2Error> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).or_else(|_| {
            // Try reading as bytes for CP932 fallback
            let bytes = std::fs::read(path)?;
            if options.try_cp932 {
                openlr2_core::encoding::cp932_to_utf8(&bytes)
            } else {
                String::from_utf8(bytes)
                    .map_err(|e| OpenLr2Error::Encoding(format!("invalid utf-8: {}", e)))
            }
        })?;

        crate::parser::parse_bms(&content, path, &options)
    }
}
