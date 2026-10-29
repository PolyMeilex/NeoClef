use crate::passes::{Control, Performance, round_tempo};

const DEFAULT_BPS: f64 = 120.0 / 60.0;

pub fn tempo(perf: &mut Performance) {
    for (tick, control) in &perf.controls {
        if let Control::Tempo(qpm) = control {
            perf.tempo.insert(*tick, round_tempo(qpm / 60.0));
        }
    }

    let first = perf.tempo.entry(0).or_insert(0.0);
    if *first <= 0.0 {
        *first = DEFAULT_BPS;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn tempo_lane(controls: Vec<(usize, Control)>) -> BTreeMap<usize, f64> {
        let mut perf = Performance {
            events: Vec::new(),
            controls,
            tempo: BTreeMap::new(),
            end: 0,
        };
        tempo(&mut perf);
        perf.tempo
    }

    #[test]
    fn defaults_to_120_bpm() {
        assert_eq!(tempo_lane(Vec::new()), [(0, 2.0)].into());
        assert_eq!(
            tempo_lane(vec![(0, Control::Tempo(0.0)), (480, Control::Tempo(90.0))]),
            [(0, 2.0), (480, 1.5)].into()
        );
    }

    #[test]
    fn later_control_at_the_same_tick_wins() {
        assert_eq!(
            tempo_lane(vec![
                (0, Control::Tempo(100.0)),
                (0, Control::Tempo(60.0)),
                (
                    0,
                    Control::ChordSymbol {
                        staff: 1,
                        keys: Vec::new()
                    }
                ),
                (480, Control::Tempo(70.0)),
            ]),
            [(0, 1.0), (480, round_tempo(70.0 / 60.0))].into()
        );
    }
}
