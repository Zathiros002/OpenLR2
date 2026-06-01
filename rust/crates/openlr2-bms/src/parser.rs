//! BMS parser — tokenizer and line-by-line parser.
//!
//! ## BMS format quick reference
//!
//! BMS files use CRLF or LF line endings. Each line is either:
//! - A header: `#KEY VALUE`
//! - A channel data line: `#xxxnn:VVVV...`
//! - A comment or blank line.
//!
//! Channel data format:
//! - `xxx` = measure number (3-digit decimal, `000`–`999`).
//! - `nn` = channel number (2-digit Base36: `01`–`ZZ`).
//! - `:VVVV...` = values, each 2 hex chars (Base36 for BGM, Base62 for Extended).

use openlr2_core::{Beat, OpenLr2Error};
use std::collections::HashMap;
use std::path::Path;

use crate::chart::{
    BmsMeta, BmsParseOptions, Chart, Lane, Note, NoteKind, ResourceRef, ResourceTable,
};
use crate::timing::{build_timeline, BpmEvent, StopEvent, TimingEvent};

struct ParseState {
    meta: BmsMeta,
    wav_table: HashMap<usize, ResourceRef>,
    bmp_table: HashMap<usize, ResourceRef>,
    bpm_events: Vec<BpmEvent>,
    stop_events: Vec<StopEvent>,
    raw_notes: Vec<Vec<(Beat, String)>>,
    keymode: Option<u32>,
    lntype: u8,
    measure_resolutions: HashMap<u32, usize>,
}

pub fn parse_bms(
    content: &str,
    filepath: &Path,
    _options: &BmsParseOptions,
) -> Result<Chart, OpenLr2Error> {
    let mut state = ParseState::new(filepath);

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('#') {
            continue;
        }
        let line = line.strip_suffix('\r').unwrap_or(line);

        if let Some(rest) = line.strip_prefix("#WAV") {
            parse_resource_line(rest, &mut state.wav_table, 36)?;
        } else if let Some(rest) = line.strip_prefix("#BMP") {
            parse_resource_line(rest, &mut state.bmp_table, 36)?;
        } else if let Some(rest) = line.strip_prefix("#STOP") {
            state.parse_stop_line(rest)?;
        } else if let Some(rest) = line.strip_prefix("#STP") {
            state.parse_stop_ms_line(rest)?;
        } else if let Some(rest) = line.strip_prefix("#BPM") {
            if let Some(value) = rest.split_whitespace().next() {
                state.meta.max_bpm = state.meta.max_bpm.max(value.parse().unwrap_or(0.0));
                state.meta.min_bpm = if state.meta.min_bpm == 0.0 {
                    value.parse().unwrap_or(130.0)
                } else {
                    state.meta.min_bpm.min(value.parse().unwrap_or(0.0))
                };
                if let Ok(bpm) = value.parse::<f64>() {
                    state.bpm_events.push(BpmEvent {
                        bms_time: Beat(0.0),
                        bpm,
                    });
                }
            }
        } else if let Some(rest) = line.strip_prefix("#TITLE") {
            state.meta.title = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("#SUBTITLE") {
            state.meta.subtitle = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("#ARTIST") {
            state.meta.artist = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("#SUBARTIST") {
            state.meta.subartist = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("#GENRE") {
            state.meta.genre = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("#PLAYER") {
            let val: u32 = rest.trim().parse().unwrap_or(1);
            state.keymode = Some(player_to_keymode(val));
        } else if let Some(rest) = line.strip_prefix("#LNTYPE") {
            state.lntype = rest.trim().parse().unwrap_or(0);
        } else if line.len() >= 6 {
            state.parse_channel_line(line)?;
        }
    }

    Ok(state.build_chart())
}

impl ParseState {
    fn new(filepath: &Path) -> Self {
        Self {
            meta: BmsMeta {
                filepath: filepath.to_path_buf(),
                max_bpm: 0.0,
                min_bpm: 0.0,
                ..Default::default()
            },
            wav_table: HashMap::new(),
            bmp_table: HashMap::new(),
            bpm_events: Vec::new(),
            stop_events: Vec::new(),
            raw_notes: vec![Vec::new(); 20],
            keymode: None,
            lntype: 1,
            measure_resolutions: HashMap::new(),
        }
    }

    fn parse_stop_line(&mut self, rest: &str) -> Result<(), OpenLr2Error> {
        let idx_end = rest
            .find(|c: char| !c.is_ascii_alphanumeric())
            .unwrap_or(rest.len());
        let val_str = rest[idx_end..].trim();
        let val: f64 = val_str.parse().unwrap_or(0.0);
        let duration_ms = if val > 0.0 {
            192.0 / val * 1000.0 / 2.0
        } else {
            0.0
        };
        if duration_ms > 0.0 {
            self.stop_events.push(StopEvent {
                bms_time: Beat(0.0),
                duration_ms: openlr2_core::Millis(duration_ms),
            });
        }
        Ok(())
    }

    fn parse_stop_ms_line(&mut self, rest: &str) -> Result<(), OpenLr2Error> {
        let parts: Vec<&str> = rest.splitn(2, char::is_whitespace).collect();
        if parts.len() < 2 {
            return Ok(());
        }
        let val_ms: f64 = parts[1].parse().unwrap_or(0.0);
        if val_ms > 0.0 {
            self.stop_events.push(StopEvent {
                bms_time: Beat(0.0),
                duration_ms: openlr2_core::Millis(val_ms),
            });
        }
        Ok(())
    }

    fn parse_channel_line(&mut self, line: &str) -> Result<(), OpenLr2Error> {
        let body = &line[1..];
        if body.len() < 6 {
            return Ok(());
        }
        let measure_str = &body[0..3];
        let channel_str = &body[3..5];
        let measure: u32 = measure_str
            .parse()
            .map_err(|_| OpenLr2Error::BmsParse(format!("invalid measure: {}", measure_str)))?;

        let channel = base_decode(channel_str, 36)
            .ok_or_else(|| OpenLr2Error::BmsParse(format!("invalid channel: {}", channel_str)))?;

        let colon_pos = body[5..].find(':').map(|p| p + 5).unwrap_or(body.len());
        if colon_pos >= body.len() {
            return Ok(());
        }
        let values_str = &body[colon_pos + 1..];
        let num_values = values_str.len() / 2;
        if num_values == 0 {
            return Ok(());
        }
        self.measure_resolutions.insert(measure, num_values);

        for (i, chunk) in values_str.as_bytes().chunks(2).enumerate() {
            if chunk.len() < 2 {
                continue;
            }
            let val_str = std::str::from_utf8(chunk).unwrap_or("00");
            if val_str == "00" {
                continue;
            }
            let beat = (measure as f64 - 1.0) + (i as f64) / (num_values as f64);

            if channel == 3 || channel == 8 {
                if let Ok(bpm) = u32::from_str_radix(val_str, 16) {
                    let bpm_f = if bpm > 0 { bpm as f64 } else { 130.0 };
                    self.meta.max_bpm = self.meta.max_bpm.max(bpm_f);
                    self.meta.min_bpm = if self.meta.min_bpm == 0.0 {
                        bpm_f
                    } else {
                        self.meta.min_bpm.min(bpm_f)
                    };
                    self.bpm_events.push(BpmEvent {
                        bms_time: Beat(beat),
                        bpm: bpm_f,
                    });
                }
            } else if channel == 9 {
                let val = u32::from_str_radix(val_str, 16).unwrap_or(0);
                if val > 0 {
                    let duration_ms = 192.0 / val as f64 * 1000.0 / 2.0;
                    self.stop_events.push(StopEvent {
                        bms_time: Beat(beat),
                        duration_ms: openlr2_core::Millis(duration_ms),
                    });
                }
            } else if let Some(lane) = channel_to_lane(channel) {
                if lane < self.raw_notes.len() {
                    self.raw_notes[lane].push((Beat(beat), val_str.to_string()));
                }
            }
        }
        Ok(())
    }

    fn build_chart(self) -> Chart {
        let keymode_u32 = self.keymode.unwrap_or(7);
        let keymode =
            openlr2_core::KeyMode::try_from(keymode_u32).unwrap_or(openlr2_core::KeyMode::Keys7);
        let total_lanes = keymode.total_lanes();

        let timing = build_timeline(&self.bpm_events, &self.stop_events);
        let total_play_time = timing
            .last()
            .map(|t| t.real_time)
            .unwrap_or(openlr2_core::Millis::ZERO);

        let mut lanes: Vec<Lane> = Vec::new();
        let mut total_notes: u32 = 0;
        for i in 0..total_lanes.min(self.raw_notes.len()) {
            let mut raw = self.raw_notes[i].clone();
            raw.sort_by(|a, b| a.0.as_f64().partial_cmp(&b.0.as_f64()).unwrap());

            let notes: Vec<Note> = raw
                .iter()
                .map(|(bms_time, val_str)| {
                    let real_time = bms_time_to_real(*bms_time, &timing);
                    let note_val = u32::from_str_radix(val_str, 16).unwrap_or(0);
                    Note {
                        kind: NoteKind::Tap,
                        bms_time: *bms_time,
                        real_time,
                        keysound_id: if note_val > 0 {
                            Some(note_val as usize)
                        } else {
                            None
                        },
                        bga_id: None,
                    }
                })
                .collect();

            if !notes.is_empty() {
                total_notes += notes.len() as u32;
            }
            lanes.push(Lane { index: i, notes });
        }

        Chart {
            metadata: self.meta,
            keymode,
            lanes,
            timing,
            resources: ResourceTable {
                wav: self.wav_table,
                bmp: self.bmp_table,
            },
            total_notes,
            total_play_time,
        }
    }
}

// ---- Free functions (shared helpers) ----

fn player_to_keymode(player: u32) -> u32 {
    match player {
        1 => 7,
        2 => 14,
        3 => 9,
        4 => 10,
        5 => 5,
        _ => 7,
    }
}

fn parse_resource_line(
    rest: &str,
    table: &mut HashMap<usize, ResourceRef>,
    base: u32,
) -> Result<(), OpenLr2Error> {
    let parts: Vec<&str> = rest.splitn(2, |c: char| c.is_whitespace()).collect();
    if parts.len() < 2 {
        return Ok(());
    }
    let idx_str = parts[0];
    let path = parts[1].trim();
    let index = base_decode(idx_str, base)
        .ok_or_else(|| OpenLr2Error::BmsParse(format!("invalid resource index: {}", idx_str)))?;

    table.insert(
        index,
        ResourceRef {
            index,
            path: path.to_string(),
        },
    );
    Ok(())
}

fn base_decode(s: &str, base: u32) -> Option<usize> {
    let mut val: usize = 0;
    for ch in s.chars() {
        let digit = match ch {
            '0'..='9' => ch as u32 - '0' as u32,
            'A'..='Z' => 10 + ch as u32 - 'A' as u32,
            'a'..='z' => 36 + ch as u32 - 'a' as u32,
            _ => return None,
        };
        if digit >= base {
            return None;
        }
        val = val * (base as usize) + digit as usize;
    }
    Some(val)
}

fn channel_to_lane(channel: usize) -> Option<usize> {
    match channel {
        11..=17 => Some(channel - 11),
        18 | 19 => Some(0),
        51..=57 => Some(channel - 51),
        58 | 59 => Some(0),
        21..=27 => Some(channel - 14),
        28 | 29 => Some(7),
        _ => None,
    }
}

fn bms_time_to_real(bms_time: Beat, timeline: &[TimingEvent]) -> openlr2_core::Millis {
    let mut last_bpm = 130.0;
    let mut last_bms = 0.0;
    let mut last_real = 0.0;

    for event in timeline {
        if event.bms_time.as_f64() <= bms_time.as_f64() {
            last_bms = event.bms_time.as_f64();
            last_real = event.real_time.as_f64();
            if let crate::timing::TimingEventKind::Bpm { bpm } = event.kind {
                last_bpm = bpm;
            }
        } else {
            break;
        }
    }

    let db = bms_time.as_f64() - last_bms;
    openlr2_core::Millis(last_real + db / last_bpm * 60_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base36_decode() {
        assert_eq!(base_decode("01", 36), Some(1));
        assert_eq!(base_decode("0Z", 36), Some(35));
        assert_eq!(base_decode("10", 36), Some(36));
    }

    #[test]
    fn channel_to_lane_7k() {
        assert_eq!(channel_to_lane(11), Some(0));
        assert_eq!(channel_to_lane(15), Some(4));
        assert_eq!(channel_to_lane(17), Some(6));
        assert_eq!(channel_to_lane(18), Some(0));
        assert_eq!(channel_to_lane(1), None);
    }

    #[test]
    fn parse_minimal_bms() {
        let bms = "#TITLE test\n#PLAYER 1\n#GENRE test\n#BPM 150\n#00111:01010101\n";
        let path = std::path::Path::new("test.bms");
        let opts = BmsParseOptions::default();
        let chart = parse_bms(bms, path, &opts).unwrap();
        assert_eq!(chart.metadata.title, "test");
        assert_eq!(chart.keymode, openlr2_core::KeyMode::Keys7);
    }

    #[test]
    fn detect_keymode_from_player() {
        let opts = BmsParseOptions::default();
        let path = std::path::Path::new("test.bms");

        assert_eq!(
            parse_bms("#PLAYER 5\n#BPM 120\n", path, &opts)
                .unwrap()
                .keymode,
            openlr2_core::KeyMode::Keys5
        );
        assert_eq!(
            parse_bms("#PLAYER 3\n#BPM 120\n", path, &opts)
                .unwrap()
                .keymode,
            openlr2_core::KeyMode::Keys9
        );
        assert_eq!(
            parse_bms("#PLAYER 2\n#BPM 120\n", path, &opts)
                .unwrap()
                .keymode,
            openlr2_core::KeyMode::Keys14
        );
    }
}
