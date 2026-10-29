use std::collections::{BTreeMap, BTreeSet};

use crate::passes::{NoteKind, Performance, Tick, round_tempo};

pub fn fermata(perf: &mut Performance) {
    let mut fermatas: BTreeMap<Tick, f64> = BTreeMap::new();
    let mut onsets = BTreeSet::new();

    for event in &perf.events {
        if event.kind == NoteKind::Grace {
            continue;
        }

        onsets.insert(event.on);
        if let Some(stretch) = event.fermata {
            let longest = fermatas.entry(event.on).or_insert(stretch);
            *longest = longest.max(stretch);
        }
    }

    for (start, stretch) in fermatas {
        let (_, &bps) = perf
            .tempo
            .range(..=start)
            .next_back()
            .expect("passes::tempo starts the tempo lane at tick 0");
        let stop = onsets
            .range(start + 1..)
            .next()
            .map_or(perf.end, |onset| (*onset).min(perf.end));

        perf.tempo.insert(start, round_tempo(bps / stretch));
        perf.tempo.insert(stop.saturating_sub(1), bps);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::passes::{NoteId, PEvent, TieFlags};

    fn event(on: Tick, off: Tick, kind: NoteKind, fermata: Option<f64>) -> PEvent {
        PEvent {
            id: NoteId {
                measure: 0,
                item: 0,
            },
            on,
            off,
            kind,
            pitch: None,
            staff: 1,
            tie: TieFlags::default(),
            dynamics: None,
            end_dynamics: None,
            fermata,
        }
    }

    #[test]
    fn slows_down_until_just_before_the_next_onset() {
        let mut perf = Performance {
            events: vec![
                event(0, 960, NoteKind::Regular, Some(2.0)),
                event(0, 960, NoteKind::Regular, Some(3.0)),
                event(960, 960, NoteKind::Grace, None),
                event(960, 1440, NoteKind::Rest, Some(1.5)),
            ],
            controls: Vec::new(),
            tempo: [(0, 1.0)].into(),
            end: 1440,
        };

        fermata(&mut perf);

        assert_eq!(
            perf.tempo,
            [
                (0, round_tempo(1.0 / 3.0)),
                (959, 1.0),
                (960, round_tempo(1.0 / 1.5)),
                (1439, 1.0),
            ]
            .into()
        );
    }
}
