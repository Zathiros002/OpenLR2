use serde::{Deserialize, Serialize};

/// Milliseconds — the primary wall-clock time unit in OpenLR2.
///
/// Used for: audio playback position, timer lapses, input timing.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Millis(pub f64);

// f64 does not implement Eq, but for our purposes Millis can be Eq in the
// sense of structural equality within golden test epsilon bounds.
impl Eq for Millis {}

/// Microseconds — high-precision timer unit (e.g. QPC on Windows).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Micros(pub f64);

/// Beat count — position in the BMS timing grid.
///
/// One beat = one quarter note.  1000 beats = 1 measure * 1000 (LR2 convention).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Beat(pub f64);

/// BMS time — the internal LR2 time representation.
///
/// Maps to the integer `timing` parameter passed through `ProcNoteOnTiming` etc.
/// Equivalent to `GetTimeLapse(41, &timer1)` in the C++ code.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct BmsTime(pub f64);

impl Millis {
    pub const ZERO: Self = Self(0.0);

    pub fn as_f64(self) -> f64 {
        self.0
    }
}

impl Beat {
    pub const ZERO: Self = Self(0.0);

    pub fn as_f64(self) -> f64 {
        self.0
    }
}

impl BmsTime {
    pub const ZERO: Self = Self(0.0);

    pub fn as_f64(self) -> f64 {
        self.0
    }
}

impl Micros {
    pub const ZERO: Self = Self(0.0);
}

impl std::ops::Sub for Millis {
    type Output = Millis;
    fn sub(self, rhs: Self) -> Self::Output {
        Millis(self.0 - rhs.0)
    }
}

impl std::ops::Add for Millis {
    type Output = Millis;
    fn add(self, rhs: Self) -> Self::Output {
        Millis(self.0 + rhs.0)
    }
}
