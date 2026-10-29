use std::{collections::BTreeMap, time::Duration};

use crate::passes::{MidiNote, TICKS_PER_QUARTER_NOTE, Tick};

const SECOND: Duration = Duration::from_secs(1);

type TrackEvents = BTreeMap<Tick, Vec<midly::TrackEventKind<'static>>>;

pub fn encode(tempo: &BTreeMap<Tick, f64>, notes: &[MidiNote]) -> midly::Smf<'static> {
    let staves = notes
        .iter()
        .map(|note| note.staff)
        .max()
        .unwrap_or(1)
        .max(1);
    let mut tracks: Vec<TrackEvents> = vec![TrackEvents::new(); staves as usize];

    for (&tick, &bps) in tempo {
        if bps <= 0.0 {
            continue;
        }
        let microseconds_per_quarter_note = (SECOND.as_micros() as f64 / bps).round() as u32;

        tracks[0]
            .entry(tick)
            .or_default()
            .push(midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(
                microseconds_per_quarter_note.into(),
            )));
    }

    for note in notes {
        let events = &mut tracks[note.staff.max(1) as usize - 1];

        if !note.tie.stop {
            events
                .entry(note.on)
                .or_default()
                .push(midly::TrackEventKind::Midi {
                    channel: note.channel.into(),
                    message: midly::MidiMessage::NoteOn {
                        key: note.key.into(),
                        vel: note.vel.into(),
                    },
                });
        }

        if !note.tie.start {
            events
                .entry(note.off)
                .or_default()
                .push(midly::TrackEventKind::Midi {
                    channel: note.channel.into(),
                    message: midly::MidiMessage::NoteOff {
                        key: note.key.into(),
                        vel: note.off_vel.into(),
                    },
                });
        }
    }

    midly::Smf {
        header: midly::Header {
            format: midly::Format::Parallel,
            timing: midly::Timing::Metrical(midly::num::u15::new(TICKS_PER_QUARTER_NOTE)),
        },
        tracks: tracks.into_iter().map(delta_encode).collect(),
    }
}

fn order(kind: &midly::TrackEventKind) -> u8 {
    match kind {
        midly::TrackEventKind::Midi {
            message: midly::MidiMessage::NoteOff { .. },
            ..
        } => 1,
        midly::TrackEventKind::Midi {
            message: midly::MidiMessage::NoteOn { .. },
            ..
        } => 2,
        _ => 0,
    }
}

fn delta_encode(events: TrackEvents) -> Vec<midly::TrackEvent<'static>> {
    let mut track = vec![];

    let mut prev = 0;
    for (position, mut events) in events {
        let mut delta = position - prev;
        prev = position;

        events.sort_by_key(order);
        for kind in events {
            track.push(midly::TrackEvent {
                delta: (delta as u32).into(),
                kind,
            });
            delta = 0;
        }
    }

    track.push(midly::TrackEvent {
        delta: 1.into(),
        kind: midly::TrackEventKind::Meta(midly::MetaMessage::EndOfTrack),
    });

    track
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::passes::{DEFAULT_VELOCITY, TieFlags};

    fn note(on: Tick, off: Tick, key: u8, staff: u8) -> MidiNote {
        MidiNote {
            id: None,
            on,
            off,
            key,
            vel: DEFAULT_VELOCITY,
            off_vel: 0,
            tie: TieFlags::default(),
            staff,
            channel: 0,
        }
    }

    fn kinds(track: &[midly::TrackEvent<'static>]) -> Vec<(u32, midly::TrackEventKind<'static>)> {
        track
            .iter()
            .map(|event| (u32::from(event.delta), event.kind))
            .collect()
    }

    fn on(key: u8) -> midly::TrackEventKind<'static> {
        midly::TrackEventKind::Midi {
            channel: 0.into(),
            message: midly::MidiMessage::NoteOn {
                key: key.into(),
                vel: 80.into(),
            },
        }
    }

    fn off(key: u8) -> midly::TrackEventKind<'static> {
        midly::TrackEventKind::Midi {
            channel: 0.into(),
            message: midly::MidiMessage::NoteOff {
                key: key.into(),
                vel: 0.into(),
            },
        }
    }

    #[test]
    fn note_off_before_note_on_within_a_tick() {
        let smf = encode(
            &BTreeMap::from([(0, 2.0)]),
            &[note(480, 960, 60, 1), note(0, 480, 60, 1)],
        );

        assert_eq!(
            kinds(&smf.tracks[0]),
            [
                (
                    0,
                    midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(500_000.into()))
                ),
                (0, on(60)),
                (480, off(60)),
                (0, on(60)),
                (480, off(60)),
                (
                    1,
                    midly::TrackEventKind::Meta(midly::MetaMessage::EndOfTrack)
                ),
            ]
        );
    }

    #[test]
    fn unpaired_ties_leave_out_events() {
        let mut first = note(0, 480, 60, 1);
        first.tie.start = true;
        let mut second = note(480, 960, 60, 1);
        second.tie.stop = true;

        let smf = encode(&BTreeMap::new(), &[first, second]);

        assert_eq!(
            kinds(&smf.tracks[0]),
            [
                (0, on(60)),
                (960, off(60)),
                (
                    1,
                    midly::TrackEventKind::Meta(midly::MetaMessage::EndOfTrack)
                ),
            ]
        );
    }

    #[test]
    fn one_track_per_staff() {
        let smf = encode(&BTreeMap::new(), &[note(0, 480, 60, 3)]);

        assert_eq!(smf.tracks.len(), 3);
        assert_eq!(kinds(&smf.tracks[2])[..2], [(0, on(60)), (480, off(60))]);
    }
}
