use std::collections::BTreeMap;

use crate::passes::{MeasureIR, PEvent, Performance, Time, Visit};

pub fn unroll(measures: &[MeasureIR], order: &[Visit]) -> Performance {
    let mut perf = Performance {
        events: Vec::new(),
        controls: Vec::new(),
        tempo: BTreeMap::new(),
        end: 0,
    };

    let mut start = Time::ZERO;
    for visit in order {
        let measure = &measures[visit.measure];

        for note in &measure.notes {
            let event = PEvent {
                id: note.id,
                on: (start + note.at).to_ticks(),
                off: (start + note.at + note.dur).to_ticks(),
                kind: note.kind,
                pitch: note.pitch.clone(),
                staff: note.staff,
                tie: note.tie,
                dynamics: note.dynamics,
                end_dynamics: note.end_dynamics,
                fermata: note.fermata,
            };
            perf.end = perf.end.max(event.off);
            perf.events.push(event);
        }

        for (at, control) in &measure.controls {
            perf.controls
                .push(((start + *at).to_ticks(), control.clone()));
        }

        start += measure.duration;
        perf.end = perf.end.max(start.to_ticks());
    }

    perf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::passes::{Control, NoteEv, NoteId, NoteKind, TieFlags};

    fn quarters(value: f64) -> Time {
        Time::from_divisions(value, 1.0)
    }

    fn measure(duration: f64, notes: &[(f64, f64)]) -> MeasureIR {
        MeasureIR {
            start: Time::ZERO,
            duration: quarters(duration),
            notes: notes
                .iter()
                .enumerate()
                .map(|(item, &(at, dur))| NoteEv {
                    id: NoteId { measure: 0, item },
                    at: quarters(at),
                    dur: quarters(dur),
                    kind: NoteKind::Rest,
                    pitch: None,
                    staff: 1,
                    tie: TieFlags::default(),
                    dynamics: None,
                    end_dynamics: None,
                    fermata: None,
                })
                .collect(),
            controls: vec![(quarters(at_end(notes)), Control::Tempo(60.0))],
        }
    }

    fn at_end(notes: &[(f64, f64)]) -> f64 {
        notes.last().map_or(0.0, |(at, dur)| at + dur)
    }

    #[test]
    fn visits_follow_each_other() {
        let measures = [measure(2.0, &[(0.0, 2.0)]), measure(1.0, &[(0.5, 2.0)])];
        let perf = unroll(
            &measures,
            &[
                Visit { measure: 1 },
                Visit { measure: 0 },
                Visit { measure: 1 },
            ],
        );

        let ticks: Vec<_> = perf
            .events
            .iter()
            .map(|event| (event.on, event.off))
            .collect();
        assert_eq!(ticks, [(240, 1200), (480, 1440), (1680, 2640)]);

        let controls: Vec<_> = perf.controls.iter().map(|(tick, _)| *tick).collect();
        assert_eq!(controls, [1200, 1440, 2640]);

        assert_eq!(perf.end, 2640);
    }
}
