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
use crate::timing::{build_timeline, BpmEvent, StopDuration, StopEvent, TimingEvent};

struct ParseState {
    meta: BmsMeta,
    wav_table: HashMap<usize, ResourceRef>,
    bmp_table: HashMap<usize, ResourceRef>,
    bpm_table: HashMap<usize, f64>,
    stop_table: HashMap<usize, f64>,
    bpm_events: Vec<BpmEvent>,
    stop_events: Vec<StopEvent>,
    raw_notes: Vec<Vec<(Beat, String)>>,
    keymode: Option<u32>,
    lntype: u8,
    measure_resolutions: HashMap<u32, usize>,
    measure_lengths: HashMap<u32, f64>,
}

pub fn parse_bms(
    content: &str,
    filepath: &Path,
    _options: &BmsParseOptions,
) -> Result<Chart, OpenLr2Error> {
    let mut state = ParseState::new(filepath);
    let mut channel_lines = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('#') {
            continue;
        }
        let line = line.strip_suffix('\r').unwrap_or(line);

        if let Some(rest) = line.strip_prefix("#WAV") {
            parse_resource_line(rest, &mut state.wav_table, 62)?;
        } else if let Some(rest) = line.strip_prefix("#BMP") {
            parse_resource_line(rest, &mut state.bmp_table, 62)?;
        } else if let Some(rest) = line.strip_prefix("#STOP") {
            state.parse_stop_line(rest)?;
        } else if let Some(rest) = line.strip_prefix("#STP") {
            state.parse_stop_ms_line(rest)?;
        } else if let Some(rest) = line.strip_prefix("#BPM") {
            state.parse_bpm_line(rest)?;
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
            channel_lines.push(line.to_string());
        }
    }

    for line in &channel_lines {
        state.parse_measure_length_line(line)?;
    }
    for line in &channel_lines {
        state.parse_channel_line(line)?;
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
            bpm_table: HashMap::new(),
            stop_table: HashMap::new(),
            bpm_events: Vec::new(),
            stop_events: Vec::new(),
            raw_notes: vec![Vec::new(); 20],
            keymode: None,
            lntype: 1,
            measure_resolutions: HashMap::new(),
            measure_lengths: HashMap::new(),
        }
    }

    fn parse_stop_line(&mut self, rest: &str) -> Result<(), OpenLr2Error> {
        let (idx, value) = parse_indexed_value(rest, 36)?;
        self.stop_table.insert(idx, value);
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
                duration: StopDuration::Millis(openlr2_core::Millis(val_ms)),
            });
        }
        Ok(())
    }

    fn parse_bpm_line(&mut self, rest: &str) -> Result<(), OpenLr2Error> {
        if let Ok((idx, bpm)) = parse_indexed_value(rest, 36) {
            self.bpm_table.insert(idx, bpm);
            self.update_bpm_range(bpm);
            return Ok(());
        }

        if let Some(value) = rest.split_whitespace().next() {
            if let Ok(bpm) = value.parse::<f64>() {
                self.update_bpm_range(bpm);
                self.bpm_events.push(BpmEvent {
                    bms_time: Beat(0.0),
                    bpm,
                });
            }
        }
        Ok(())
    }

    fn update_bpm_range(&mut self, bpm: f64) {
        self.meta.max_bpm = self.meta.max_bpm.max(bpm);
        self.meta.min_bpm = if self.meta.min_bpm == 0.0 {
            bpm
        } else {
            self.meta.min_bpm.min(bpm)
        };
    }

    fn parse_measure_length_line(&mut self, line: &str) -> Result<(), OpenLr2Error> {
        let Some((measure, channel, values_str)) = parse_channel_parts(line)? else {
            return Ok(());
        };
        if channel == 2 {
            if let Ok(ratio) = values_str.trim().parse::<f64>() {
                if ratio > 0.0 {
                    self.measure_lengths.insert(measure, ratio);
                }
            }
        }
        Ok(())
    }

    fn parse_channel_line(&mut self, line: &str) -> Result<(), OpenLr2Error> {
        let Some((measure, channel, values_str)) = parse_channel_parts(line)? else {
            return Ok(());
        };
        if channel == 2 {
            return Ok(());
        }

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
            let beat = self.beat_at(measure, i, num_values);

            if channel == 3 {
                if let Ok(bpm) = u32::from_str_radix(val_str, 16) {
                    let bpm_f = if bpm > 0 { bpm as f64 } else { 130.0 };
                    self.update_bpm_range(bpm_f);
                    self.bpm_events.push(BpmEvent {
                        bms_time: Beat(beat),
                        bpm: bpm_f,
                    });
                }
            } else if channel == 8 {
                if let Some(idx) = base_decode(val_str, 36) {
                    if let Some(&bpm) = self.bpm_table.get(&idx) {
                        self.update_bpm_range(bpm);
                        self.bpm_events.push(BpmEvent {
                            bms_time: Beat(beat),
                            bpm,
                        });
                    }
                }
            } else if channel == 9 {
                if let Some(idx) = base_decode(val_str, 36) {
                    if let Some(&stop_value) = self.stop_table.get(&idx) {
                        if stop_value <= 0.0 {
                            continue;
                        }
                        self.stop_events.push(StopEvent {
                            bms_time: Beat(beat),
                            duration: StopDuration::BmsValue(stop_value),
                        });
                    }
                } else if let Ok(val) = u32::from_str_radix(val_str, 16) {
                    self.stop_events.push(StopEvent {
                        bms_time: Beat(beat),
                        duration: StopDuration::BmsValue(val as f64),
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

    fn beat_at(&self, measure: u32, index: usize, resolution: usize) -> f64 {
        let start = (0..measure)
            .map(|m| self.measure_length_beats(m))
            .sum::<f64>();
        start + (index as f64) / (resolution as f64) * self.measure_length_beats(measure)
    }

    fn measure_length_beats(&self, measure: u32) -> f64 {
        4.0 * self.measure_lengths.get(&measure).copied().unwrap_or(1.0)
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
                    let note_val = base_decode(val_str, 62).unwrap_or(0);
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

fn parse_channel_parts(line: &str) -> Result<Option<(u32, usize, &str)>, OpenLr2Error> {
    let body = &line[1..];
    if body.len() < 6 {
        return Ok(None);
    }
    let measure_str = &body[0..3];
    let channel_str = &body[3..5];
    let measure: u32 = measure_str
        .parse()
        .map_err(|_| OpenLr2Error::BmsParse(format!("invalid measure: {}", measure_str)))?;

    let channel = parse_channel_code(channel_str)
        .ok_or_else(|| OpenLr2Error::BmsParse(format!("invalid channel: {}", channel_str)))?;

    let colon_pos = body[5..].find(':').map(|p| p + 5).unwrap_or(body.len());
    if colon_pos >= body.len() {
        return Ok(None);
    }

    Ok(Some((measure, channel, &body[colon_pos + 1..])))
}

fn parse_indexed_value(rest: &str, base: u32) -> Result<(usize, f64), OpenLr2Error> {
    let idx_end = rest
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(rest.len());
    if idx_end == 0 || idx_end == rest.len() {
        return Err(OpenLr2Error::BmsParse("missing indexed value".to_string()));
    }

    let idx_str = &rest[..idx_end];
    let value_str = rest[idx_end..].trim();
    if value_str.is_empty() {
        return Err(OpenLr2Error::BmsParse(format!(
            "missing value for index {}",
            idx_str
        )));
    }

    let idx = base_decode(idx_str, base)
        .ok_or_else(|| OpenLr2Error::BmsParse(format!("invalid indexed key: {}", idx_str)))?;
    let value = value_str
        .parse::<f64>()
        .map_err(|_| OpenLr2Error::BmsParse(format!("invalid indexed value: {}", value_str)))?;
    Ok((idx, value))
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

fn parse_channel_code(channel: &str) -> Option<usize> {
    if channel.chars().all(|c| c.is_ascii_digit()) {
        channel.parse().ok()
    } else {
        base_decode(channel, 36)
    }
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
            match event.kind {
                crate::timing::TimingEventKind::Bpm { bpm } => {
                    last_bpm = bpm;
                }
                crate::timing::TimingEventKind::Stop { duration_ms } => {
                    last_real += duration_ms.as_f64();
                }
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
        assert_eq!(base_decode("zz", 62), Some(3843));
        assert_eq!(base_decode("10", 62), Some(62));
    }

    #[test]
    fn channel_to_lane_7k() {
        assert_eq!(parse_channel_code("11"), Some(11));
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
        assert_eq!(chart.total_notes, 4);
        assert_eq!(chart.lanes[0].notes[0].bms_time, Beat(4.0));
    }

    #[test]
    fn scales_measures_to_quarter_note_beats() {
        let bms = "#BPM 120\n#00211:01\n";
        let path = std::path::Path::new("test.bms");
        let opts = BmsParseOptions::default();
        let chart = parse_bms(bms, path, &opts).unwrap();

        assert_eq!(chart.lanes[0].notes[0].bms_time, Beat(8.0));
    }

    #[test]
    fn parse_indexed_bpm_and_stop_channels() {
        let bms = "#BPM 120\n#BPM01 180\n#STOP01 192\n#00108:01\n#00109:01\n";
        let path = std::path::Path::new("test.bms");
        let opts = BmsParseOptions::default();
        let chart = parse_bms(bms, path, &opts).unwrap();

        assert!(chart
            .timing
            .iter()
            .any(|event| matches!(event.kind, crate::timing::TimingEventKind::Bpm { bpm } if (bpm - 180.0).abs() < 0.01)));
        assert!(chart
            .timing
            .iter()
            .any(|event| matches!(event.kind, crate::timing::TimingEventKind::Stop { duration_ms } if (duration_ms.as_f64() - 1333.333333).abs() < 0.01)));
    }

    #[test]
    fn stop_delays_following_note_time() {
        let bms = "#BPM 120\n#STOP01 192\n#00109:01\n#00111:0001\n";
        let path = std::path::Path::new("test.bms");
        let opts = BmsParseOptions::default();
        let chart = parse_bms(bms, path, &opts).unwrap();

        assert!((chart.lanes[0].notes[0].real_time.as_f64() - 5000.0).abs() < 0.01);
    }

    #[test]
    fn decodes_note_resource_ids_as_base62() {
        let bms = "#BPM 120\n#WAV10 kick.wav\n#WAVG0 clap.wav\n#00111:10G0\n";
        let path = std::path::Path::new("test.bms");
        let opts = BmsParseOptions::default();
        let chart = parse_bms(bms, path, &opts).unwrap();

        assert_eq!(chart.lanes[0].notes[0].keysound_id, Some(62));
        assert_eq!(chart.lanes[0].notes[1].keysound_id, Some(992));
    }

    #[test]
    fn applies_measure_length_channel_to_beat_positions() {
        let bms = "#BPM 120\n#00102:0.5\n#00111:01\n#00211:01\n";
        let path = std::path::Path::new("test.bms");
        let opts = BmsParseOptions::default();
        let chart = parse_bms(bms, path, &opts).unwrap();

        assert_eq!(chart.lanes[0].notes[0].bms_time, Beat(4.0));
        assert_eq!(chart.lanes[0].notes[1].bms_time, Beat(6.0));
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
