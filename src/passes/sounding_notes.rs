use crate::{
    parsing::musicxml,
    passes::{DEFAULT_VELOCITY, MidiNote, NoteKind, Performance},
};

pub fn sounding_notes(perf: &Performance) -> Vec<MidiNote> {
    perf.events
        .iter()
        .filter(|event| event.kind == NoteKind::Regular)
        .filter_map(|event| {
            let pitch = event.pitch.as_ref()?;
            Some(MidiNote {
                id: Some(event.id),
                on: event.on,
                off: event.off,
                key: midi_note_number(pitch),
                vel: event.dynamics.map_or(DEFAULT_VELOCITY, |dynamics| {
                    dynamics_to_velocity(dynamics).max(1)
                }),
                off_vel: event.end_dynamics.map_or(0, dynamics_to_velocity),
                tie: event.tie,
                staff: event.staff,
                channel: 0,
            })
        })
        .collect()
}

const MIDI_FORTE_VELOCITY: f64 = 90.0;

fn dynamics_to_velocity(dynamics: f64) -> u8 {
    (dynamics / 100.0 * MIDI_FORTE_VELOCITY)
        .round()
        .clamp(0.0, 127.0) as u8
}

fn midi_note_number(pitch: &musicxml::Pitch) -> u8 {
    use musicxml::Step;
    let base = match pitch.step {
        Step::C => 0,
        Step::D => 2,
        Step::E => 4,
        Step::F => 5,
        Step::G => 7,
        Step::A => 9,
        Step::B => 11,
    };

    // No microtones for now
    let alter = pitch.alter.unwrap_or(0.0).round() as i32;

    (((pitch.octave + 1) * 12 + base) as i32 + alter) as u8
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::passes::{NoteId, PEvent, TieFlags};

    fn event(kind: NoteKind, pitch: Option<musicxml::Pitch>, dynamics: Option<f64>) -> PEvent {
        PEvent {
            id: NoteId {
                measure: 0,
                item: 0,
            },
            on: 0,
            off: 480,
            kind,
            pitch,
            staff: 1,
            tie: TieFlags::default(),
            dynamics,
            end_dynamics: Some(50.0),
            fermata: None,
        }
    }

    fn pitch(step: musicxml::Step, alter: Option<f64>, octave: u8) -> Option<musicxml::Pitch> {
        Some(musicxml::Pitch {
            step,
            alter,
            octave,
        })
    }

    #[test]
    fn only_regular_notes_sound() {
        use musicxml::Step;

        let perf = Performance {
            events: vec![
                event(NoteKind::Regular, pitch(Step::C, None, 4), None),
                event(NoteKind::Regular, pitch(Step::F, Some(1.0), 3), Some(0.1)),
                event(NoteKind::Cue, pitch(Step::C, None, 4), None),
                event(NoteKind::Grace, pitch(Step::C, None, 4), None),
                event(NoteKind::Rest, None, None),
                event(NoteKind::Unpitched, None, None),
            ],
            controls: Vec::new(),
            tempo: BTreeMap::new(),
            end: 480,
        };

        let notes: Vec<_> = sounding_notes(&perf)
            .iter()
            .map(|note| (note.key, note.vel, note.off_vel))
            .collect();
        assert_eq!(notes, [(60, DEFAULT_VELOCITY, 45), (54, 1, 45)]);
    }
}
