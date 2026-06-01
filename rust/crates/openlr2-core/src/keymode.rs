/// BMS key mode — the number of playable lanes.
///
/// Mirrors the keymode values found in `structure.h` / `gameplay.keymode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum KeyMode {
    /// 5 keys
    Keys5 = 5,
    /// 7 keys (standard)
    Keys7 = 7,
    /// 9 keys
    Keys9 = 9,
    /// 10 keys (5K DP)
    Keys10 = 10,
    /// 14 keys (7K DP)
    Keys14 = 14,
}

impl KeyMode {
    /// Number of lanes for a single player side.
    pub fn lanes_per_player(self) -> usize {
        match self {
            KeyMode::Keys5 => 5,
            KeyMode::Keys7 => 7,
            KeyMode::Keys9 => 9,
            KeyMode::Keys10 => 5,
            KeyMode::Keys14 => 7,
        }
    }

    /// Whether this key mode is double-play (two-player setup on one chart).
    pub fn is_double_play(self) -> bool {
        matches!(self, KeyMode::Keys10 | KeyMode::Keys14)
    }

    /// Total lane count.
    pub fn total_lanes(self) -> usize {
        match self {
            KeyMode::Keys5 => 5,
            KeyMode::Keys7 => 7,
            KeyMode::Keys9 => 9,
            KeyMode::Keys10 => 10,
            KeyMode::Keys14 => 14,
        }
    }
}

impl TryFrom<u32> for KeyMode {
    type Error = crate::OpenLr2Error;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            5 => Ok(KeyMode::Keys5),
            7 => Ok(KeyMode::Keys7),
            9 => Ok(KeyMode::Keys9),
            10 => Ok(KeyMode::Keys10),
            14 => Ok(KeyMode::Keys14),
            _ => Err(crate::OpenLr2Error::BmsParse(format!(
                "unsupported keymode: {}",
                value
            ))),
        }
    }
}
