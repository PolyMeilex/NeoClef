use std::collections::HashMap;

use crate::{
    parsing::musicxml::{NumberLevel, PedalType},
    passes::{Control, MidiControl, Performance, PianoPedal, Tick},
};

const DOWN: u8 = 127;
const UP: u8 = 0;

const CHANNEL: u8 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PedalSpan {
    staff: u8,
    pedal: PianoPedal,
    down: Tick,
    up: Tick,
}

/// Turns `<pedal>` marks into pedal controller changes
pub fn pedals(perf: &Performance) -> Vec<MidiControl> {
    let mut marks: Vec<(Tick, u8, NumberLevel, PedalType)> = perf
        .controls
        .iter()
        .filter_map(|(tick, control)| match *control {
            Control::Pedal {
                staff,
                number,
                kind,
            } => Some((*tick, staff, number, kind)),
            _ => None,
        })
        .collect();

    // A `<backup>` can put a mark before an earlier one
    marks.sort_by_key(|&(tick, ..)| tick);

    let mut controls: Vec<MidiControl> = spans(&marks, perf.end)
        .iter()
        .flat_map(|span| {
            let press = MidiControl {
                tick: span.down,
                staff: span.staff,
                channel: CHANNEL,
                controller: span.pedal.controller(),
                value: DOWN,
            };
            let lift = MidiControl {
                tick: span.up,
                value: UP,
                ..press
            };
            [press, lift]
        })
        .collect();

    // A pedal change lifts the pedal before pressing it again
    controls.sort_by_key(|control| (control.tick, control.value == DOWN));

    controls
}

fn spans(marks: &[(Tick, u8, NumberLevel, PedalType)], end: Tick) -> Vec<PedalSpan> {
    struct Held {
        staff: u8,
        pedal: PianoPedal,
        since: Tick,
    }

    let mut spans = Vec::new();

    let mut held: HashMap<NumberLevel, Held> = HashMap::new();

    // What `resume` presses again
    let mut last_pedal: HashMap<NumberLevel, PianoPedal> = HashMap::new();

    let mut lift = |held: Held, tick: Tick| {
        // Lifted as soon as it's pressed, so it does nothing
        if tick <= held.since {
            return;
        }
        spans.push(PedalSpan {
            staff: held.staff,
            pedal: held.pedal,
            down: held.since,
            up: tick,
        });
    };

    for &(tick, staff, number, kind) in marks {
        let pedal = match kind {
            PedalType::Start | PedalType::Change => PianoPedal::Damper,
            PedalType::Sostenuto => PianoPedal::Sostenuto,
            PedalType::Resume => last_pedal
                .get(&number)
                .copied()
                .unwrap_or(PianoPedal::Damper),
            PedalType::Stop | PedalType::Discontinue => {
                if let Some(pedal) = held.remove(&number) {
                    lift(pedal, tick);
                }
                continue;
            }
            PedalType::Continue => continue,
        };

        let pedal = match (kind, held.remove(&number)) {
            (PedalType::Change, Some(changed)) => {
                let pedal = changed.pedal;
                lift(changed, tick);
                pedal
            }
            // Already down, nothing to press
            (_, Some(down)) => {
                held.insert(number, down);
                continue;
            }
            (_, None) => pedal,
        };

        last_pedal.insert(number, pedal);
        held.insert(
            number,
            Held {
                staff,
                pedal,
                since: tick,
            },
        );
    }

    let mut held: Vec<_> = held.into_values().collect();
    held.sort_by_key(|pedal| pedal.since);
    for pedal in held {
        lift(pedal, end);
    }

    spans
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    const SUSTAIN: u8 = PianoPedal::Damper.controller();
    const SOSTENUTO: u8 = PianoPedal::Sostenuto.controller();

    fn run(marks: &[(Tick, u8, NumberLevel, PedalType)]) -> Vec<(Tick, u8, u8)> {
        let perf = Performance {
            events: Vec::new(),
            controls: marks
                .iter()
                .map(|&(tick, staff, number, kind)| {
                    (
                        tick,
                        Control::Pedal {
                            staff,
                            number,
                            kind,
                        },
                    )
                })
                .collect(),
            tempo: BTreeMap::new(),
            end: 4800,
        };
        pedals(&perf)
            .iter()
            .map(|control| (control.tick, control.controller, control.value))
            .collect()
    }

    use PedalType::*;

    #[test]
    fn start_change_stop() {
        assert_eq!(
            run(&[(0, 2, 1, Start), (480, 2, 1, Change), (960, 2, 1, Stop)]),
            [
                (0, SUSTAIN, DOWN),
                (480, SUSTAIN, UP),
                (480, SUSTAIN, DOWN),
                (960, SUSTAIN, UP),
            ]
        );
    }

    #[test]
    fn unstopped_pedal_is_held_to_the_end() {
        assert_eq!(
            run(&[(480, 1, 1, Start)]),
            [(480, SUSTAIN, DOWN), (4800, SUSTAIN, UP)]
        );
    }

    #[test]
    fn sostenuto_and_numbers() {
        assert_eq!(
            run(&[
                (0, 1, 1, Start),
                (240, 1, 2, Sostenuto),
                (480, 1, 1, Stop),
                (960, 1, 2, Stop),
            ]),
            [
                (0, SUSTAIN, DOWN),
                (240, SOSTENUTO, DOWN),
                (480, SUSTAIN, UP),
                (960, SOSTENUTO, UP),
            ]
        );
    }

    #[test]
    fn discontinue_and_resume() {
        assert_eq!(
            run(&[
                (0, 1, 1, Sostenuto),
                (480, 1, 1, Discontinue),
                (960, 1, 1, Resume),
                (1440, 1, 1, Stop),
            ]),
            [
                (0, SOSTENUTO, DOWN),
                (480, SOSTENUTO, UP),
                (960, SOSTENUTO, DOWN),
                (1440, SOSTENUTO, UP),
            ]
        );
    }

    #[test]
    fn marks_pair_in_time_order() {
        // A `<backup>` puts the stop at 480 after the stop at 960, the pedal at 480 is
        // lifted as soon as it's pressed
        assert_eq!(
            run(&[
                (480, 1, 1, Start),
                (960, 1, 1, Stop),
                (480, 1, 1, Stop),
                (960, 1, 1, Start),
            ]),
            [(960, SUSTAIN, DOWN), (4800, SUSTAIN, UP)]
        );
    }
}
