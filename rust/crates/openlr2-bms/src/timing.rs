use openlr2_core::{Beat, Millis};
use serde::{Deserialize, Serialize};

/// A point in the BPM / STOP timeline.
///
/// Maps to: `BPMtiming` struct in `structure.h`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingEvent {
    /// BMS measure-based position.
    pub bms_time: Beat,
    /// Wall-clock position (ms), after applying all BPM changes and stops.
    pub real_time: Millis,
    pub kind: TimingEventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimingEventKind {
    /// BPM change: the tempo becomes `bpm` from this point forward.
    Bpm { bpm: f64 },
    /// STOP: the game pauses for `duration_ms` milliseconds at this point.
    Stop { duration_ms: Millis },
}

/// A raw BPM change declaration from the BMS `#BPMxx` channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BpmEvent {
    /// BMS measure-based position where the BPM change takes effect.
    pub bms_time: Beat,
    pub bpm: f64,
}

/// A raw STOP declaration from the BMS `#STOPxx` / `#STPxx` channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StopEvent {
    /// BMS measure-based position where the stop begins.
    pub bms_time: Beat,
    /// Duration of the stop in milliseconds.
    /// In BMS, a stop value of `N` means `192 / N` beats of pause
    /// (or, with `#STP` syntax, `N` ms directly).
    pub duration_ms: Millis,
}

/// Build a timing timeline from raw BPM events and stop events.
///
/// This is the Rust equivalent of the BPM timeline construction in
/// `LR2_bmsload.cpp` that produces `gameplay.bpmt_data[]`.
///
/// Algorithm:
/// 1. Sort all events by BMS time.
/// 2. Walk through the sorted timeline, accumulating real time:
///    - At each BPM change, update the current BPM.
///    - At each stop, add the stop duration to accumulated real time.
/// 3. Delta real time = (delta beats) / (current BPM) * 60000 ms.
pub fn build_timeline(bpm_events: &[BpmEvent], stop_events: &[StopEvent]) -> Vec<TimingEvent> {
    // Collect and sort all raw events.
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct RawEvent {
        bms_time: Beat,
        kind: RawEventKind,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum RawEventKind {
        Bpm { bpm: f64 },
        Stop { duration_ms: Millis },
    }

    let mut events: Vec<RawEvent> = Vec::new();

    // Always start with a BPM event if the first event is not at Beat(0.0).
    let mut has_initial_bpm = false;
    for e in bpm_events {
        if e.bms_time == Beat::ZERO {
            has_initial_bpm = true;
        }
        events.push(RawEvent {
            bms_time: e.bms_time,
            kind: RawEventKind::Bpm { bpm: e.bpm },
        });
    }
    for e in stop_events {
        events.push(RawEvent {
            bms_time: e.bms_time,
            kind: RawEventKind::Stop {
                duration_ms: e.duration_ms,
            },
        });
    }

    // If no initial BPM, default to 130 (BMS default).
    if !has_initial_bpm {
        events.push(RawEvent {
            bms_time: Beat::ZERO,
            kind: RawEventKind::Bpm { bpm: 130.0 },
        });
    }

    // Sort by BMS time.
    events.sort_by(|a, b| {
        a.bms_time
            .as_f64()
            .partial_cmp(&b.bms_time.as_f64())
            .unwrap()
    });

    let mut timeline: Vec<TimingEvent> = Vec::new();
    let mut current_bpm: f64 = 130.0;
    let mut accumulated_real: f64 = 0.0;
    let mut last_bms_time: f64 = 0.0;

    for event in &events {
        let db = event.bms_time.as_f64() - last_bms_time;
        if db > 0.0 {
            // Advance real time by the beat delta at the current BPM.
            accumulated_real += db / current_bpm * 60_000.0;
        }

        match event.kind {
            RawEventKind::Bpm { bpm } => {
                current_bpm = bpm;
                timeline.push(TimingEvent {
                    bms_time: event.bms_time,
                    real_time: Millis(accumulated_real),
                    kind: TimingEventKind::Bpm { bpm },
                });
                last_bms_time = event.bms_time.as_f64();
            }
            RawEventKind::Stop { duration_ms } => {
                timeline.push(TimingEvent {
                    bms_time: event.bms_time,
                    real_time: Millis(accumulated_real),
                    kind: TimingEventKind::Stop { duration_ms },
                });
                accumulated_real += duration_ms.as_f64();
                last_bms_time = event.bms_time.as_f64();
            }
        }
    }

    timeline
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_bpm_no_stops() {
        let bpm = vec![BpmEvent {
            bms_time: Beat(0.0),
            bpm: 120.0,
        }];
        let stops = vec![];
        let tl = build_timeline(&bpm, &stops);
        assert_eq!(tl.len(), 1);
        assert_eq!(tl[0].bms_time, Beat(0.0));
        assert_eq!(tl[0].real_time, Millis(0.0));
        match tl[0].kind {
            TimingEventKind::Bpm { bpm } => assert!((bpm - 120.0).abs() < 0.01),
            _ => panic!("expected Bpm"),
        }
    }

    #[test]
    fn bpm_change_mid_song() {
        let bpm = vec![
            BpmEvent {
                bms_time: Beat(0.0),
                bpm: 120.0,
            },
            BpmEvent {
                bms_time: Beat(4.0),
                bpm: 240.0,
            },
        ];
        let stops = vec![];
        let tl = build_timeline(&bpm, &stops);
        assert_eq!(tl.len(), 2);
        // At 120 BPM, 4 beats = 2000 ms.
        assert!((tl[1].real_time.as_f64() - 2000.0).abs() < 0.1);
        match tl[1].kind {
            TimingEventKind::Bpm { bpm } => assert!((bpm - 240.0).abs() < 0.01),
            _ => panic!("expected Bpm"),
        }
    }

    #[test]
    fn stop_delays_time() {
        let bpm = vec![BpmEvent {
            bms_time: Beat(0.0),
            bpm: 120.0,
        }];
        let stops = vec![StopEvent {
            bms_time: Beat(0.0),
            duration_ms: Millis(1000.0),
        }];
        let tl = build_timeline(&bpm, &stops);
        assert_eq!(tl.len(), 2);
    }
}
