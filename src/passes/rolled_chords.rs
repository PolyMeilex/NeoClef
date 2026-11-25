use std::collections::HashMap;

use crate::{
    parsing::musicxml::NumberLevel,
    passes::{Performance, Tick, sounding_notes::midi_note_number},
};

/// Ticks between the notes of a roll at 120 BPM
const STEP: Tick = 64;
const DEFAULT_BPS: f64 = 2.0;

/// Plays rolled chords one note after another, each note still ending with the chord
pub fn rolled_chords(perf: &mut Performance) {
    // Simultaneous notes with the same number are one roll
    let mut chords: HashMap<(NumberLevel, Tick), Vec<usize>> = HashMap::new();
    for (index, event) in perf.events.iter().enumerate() {
        if let (Some(roll), Some(_)) = (event.roll, &event.pitch) {
            chords
                .entry((roll.number, event.on))
                .or_default()
                .push(index);
        }
    }

    for ((_, on), mut chord) in chords {
        let key = |index: &usize| midi_note_number(perf.events[*index].pitch.as_ref().unwrap());
        chord.sort_by_key(key);
        if perf.events[chord[0]].roll.is_some_and(|roll| roll.down) {
            chord.reverse();
        }

        let top = chord.iter().max_by_key(|index| key(index)).unwrap();
        let length = (perf.events[*top].off - on).max(1);

        // Fit the roll into the chord
        let mut step = STEP;
        while step > 0 && step * chord.len() > length {
            step = 2 * step / 3;
        }

        // The same time apart at any tempo
        let bps = perf
            .tempo
            .range(..=on)
            .next_back()
            .map_or(DEFAULT_BPS, |(_, bps)| *bps);

        for (nth, index) in chord.into_iter().enumerate() {
            // MuseScore works in whole thousandths of the note
            let thousandths = ((step * nth * 1000 / length) as f64 * bps / DEFAULT_BPS) as Tick;
            let event = &mut perf.events[index];
            event.on += (event.off - event.on) * thousandths.min(1000) / 1000;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{
        parsing::musicxml,
        passes::{NoteId, NoteKind, PEvent, Roll, TieFlags},
    };

    fn note(octave: u8, on: Tick, off: Tick, down: bool) -> PEvent {
        let id = NoteId {
            measure: 0,
            item: octave.into(),
        };
        PEvent {
            id,
            on,
            off,
            kind: NoteKind::Regular,
            pitch: Some(musicxml::Pitch {
                step: musicxml::Step::C,
                alter: None,
                octave,
            }),
            staff: 1,
            tie: TieFlags::default(),
            dynamics: None,
            end_dynamics: None,
            fermata: None,
            roll: Some(Roll { number: 1, down }),
        }
    }

    fn roll(notes: Vec<PEvent>, bps: f64) -> Vec<(u8, Tick, Tick)> {
        let mut perf = Performance {
            events: notes,
            controls: Vec::new(),
            tempo: BTreeMap::from([(0, bps)]),
            end: 0,
        };
        rolled_chords(&mut perf);
        perf.events
            .iter()
            .map(|event| (event.pitch.as_ref().unwrap().octave, event.on, event.off))
            .collect()
    }

    #[test]
    fn rolls_up_and_keeps_note_ends() {
        let chord = vec![
            note(5, 0, 960, false),
            note(3, 0, 960, false),
            note(4, 0, 960, false),
        ];
        // 64 ticks are 66 thousandths of the chord, so 63 ticks
        assert_eq!(roll(chord, 2.0), [(5, 127, 960), (3, 0, 960), (4, 63, 960)]);
    }

    #[test]
    fn rolls_down() {
        let chord = vec![note(3, 0, 960, true), note(4, 0, 960, true)];
        assert_eq!(roll(chord, 2.0), [(3, 63, 960), (4, 0, 960)]);
    }

    #[test]
    fn shrinks_to_fit_and_follows_the_tempo() {
        // 3 * 64 > 120, so 42, then 3 * 42 > 120, so 28
        let chord = vec![
            note(3, 0, 120, false),
            note(4, 0, 120, false),
            note(5, 0, 120, false),
        ];
        assert_eq!(roll(chord, 2.0), [(3, 0, 120), (4, 27, 120), (5, 55, 120)]);

        // Twice as fast, twice the ticks
        let chord = vec![note(3, 0, 960, false), note(4, 0, 960, false)];
        assert_eq!(roll(chord, 4.0), [(3, 0, 960), (4, 126, 960)]);
    }

    #[test]
    fn same_number_rolls_together_across_staves() {
        let mut bass = note(2, 0, 960, false);
        bass.staff = 2;
        let mut separate = note(6, 0, 960, false);
        separate.roll = Some(Roll {
            number: 2,
            down: false,
        });
        let chord = vec![note(4, 0, 960, false), bass, separate];
        assert_eq!(roll(chord, 2.0), [(4, 63, 960), (2, 0, 960), (6, 0, 960)]);
    }
}
