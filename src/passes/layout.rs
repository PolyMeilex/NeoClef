use crate::{
    parsing::musicxml::{self, MeasureItem},
    passes::{Control, MeasureIR, NoteEv, NoteId, NoteKind, Policy, TieFlags, Time},
};

struct Attributes {
    divisions: f64,
    staves: u32,
}

pub fn layout(part: &musicxml::Part, policy: &Policy) -> Vec<MeasureIR> {
    let mut attributes = Attributes {
        divisions: 1.0,
        staves: 1,
    };

    let mut start = Time::ZERO;
    part.measure
        .iter()
        .enumerate()
        .map(|(index, content)| {
            let laid = measure(index, content, start, &mut attributes, policy);
            start = laid.start + laid.duration;
            laid
        })
        .collect()
}

fn measure(
    index: usize,
    measure: &musicxml::Measure,
    start: Time,
    attributes: &mut Attributes,
    policy: &Policy,
) -> MeasureIR {
    let mut laid = MeasureIR {
        start,
        duration: Time::ZERO,
        notes: Vec::new(),
        controls: Vec::new(),
    };

    let mut cursor = Time::ZERO;
    let mut chord_at = Time::ZERO;

    for (item, content) in measure.content.iter().enumerate() {
        let divisions = attributes.divisions;
        let time = |value: f64| Time::from_divisions(value, divisions);

        match content {
            MeasureItem::Attributes(new) => {
                if let Some(divisions) = new.divisions {
                    attributes.divisions = divisions;
                }
                if let Some(staves) = new.staves {
                    attributes.staves = staves;
                }
            }
            MeasureItem::Note(note) => {
                let parts = NoteParts::new(note);
                let dur = time(parts.duration);

                let at = if parts.kind == NoteKind::Grace {
                    cursor
                } else if parts.chord {
                    chord_at
                } else {
                    chord_at = cursor;
                    cursor += dur;
                    laid.duration = laid.duration.max(cursor);
                    chord_at
                };

                laid.notes.push(NoteEv {
                    id: NoteId {
                        measure: index,
                        item,
                    },
                    at,
                    dur,
                    kind: parts.kind,
                    pitch: match parts.pitch {
                        musicxml::NoteKindStatePitchKind::Pitch(pitch) => Some(pitch.clone()),
                        _ => None,
                    },
                    staff: note.staff.unwrap_or(1),
                    tie: TieFlags {
                        start: parts
                            .tie
                            .iter()
                            .any(|t| t.kind == musicxml::StartStop::Start),
                        stop: parts
                            .tie
                            .iter()
                            .any(|t| t.kind == musicxml::StartStop::Stop),
                    },
                    dynamics: note.dynamics,
                    end_dynamics: note.end_dynamics,
                    fermata: fermata_stretch(note),
                });
            }
            MeasureItem::Backup(backup) => {
                cursor = (cursor - time(backup.duration)).max(Time::ZERO);
            }
            MeasureItem::Forward(forward) => {
                cursor += time(forward.duration);
                laid.duration = laid.duration.max(cursor);
            }
            MeasureItem::Print(_) => {}
            MeasureItem::Barline(_) => {}
            MeasureItem::Harmony(harmony) => {
                if let Some(keys) = super::chord_symbols::resolve(harmony) {
                    let offset = harmony
                        .offset
                        .as_ref()
                        .filter(|offset| offset.sound || policy.harmony_offset_always_sounds)
                        .map_or(Time::ZERO, |offset| time(offset.value));

                    let staff = harmony
                        .staff
                        .filter(|staff| (1..=attributes.staves).contains(&u32::from(*staff)))
                        .unwrap_or(1);

                    laid.controls
                        .push((cursor + offset, Control::ChordSymbol { staff, keys }));
                }
            }
            MeasureItem::Direction(direction) => {
                if let Some(sound) = direction.sound.as_ref()
                    && let Some(tempo) = sound.tempo
                {
                    let offset = sound
                        .offset
                        .as_ref()
                        .filter(|_| !policy.ignore_sound_offset)
                        .or(direction
                            .offset
                            .as_ref()
                            .filter(|offset| offset.sound || policy.direction_offset_always_sounds))
                        .map_or(Time::ZERO, |offset| time(offset.value));

                    laid.controls.push((cursor + offset, Control::Tempo(tempo)));
                }
            }
        }
    }

    laid
}

struct NoteParts<'a> {
    kind: NoteKind,
    chord: bool,
    pitch: &'a musicxml::NoteKindStatePitchKind,
    duration: f64,
    tie: &'a [musicxml::Tie],
}

impl<'a> NoteParts<'a> {
    fn new(note: &'a musicxml::Note) -> Self {
        use musicxml::{NoteKindState, NoteKindStatePitchKind};

        match &note.kind {
            NoteKindState::Regular(msg) => Self {
                kind: match msg.kind {
                    NoteKindStatePitchKind::Pitch(_) => NoteKind::Regular,
                    NoteKindStatePitchKind::Unpitched(_) => NoteKind::Unpitched,
                    NoteKindStatePitchKind::Rest(_) => NoteKind::Rest,
                },
                chord: msg.chord.is_some(),
                pitch: &msg.kind,
                duration: msg.duration,
                tie: &msg.tie,
            },
            NoteKindState::Cue(msg) => Self {
                kind: NoteKind::Cue,
                chord: msg.chord.is_some(),
                pitch: &msg.kind,
                duration: msg.duration,
                tie: &msg.tie,
            },
            NoteKindState::Grace(msg) => Self {
                kind: NoteKind::Grace,
                chord: msg.chord.is_some(),
                pitch: &msg.kind,
                duration: 0.0,
                tie: &msg.tie,
            },
            NoteKindState::GraceCue(msg) => Self {
                kind: NoteKind::Grace,
                chord: msg.chord.is_some(),
                pitch: &msg.kind,
                duration: 0.0,
                tie: &[],
            },
        }
    }
}

/// How much longer a note or rest with a fermata lasts
fn fermata_stretch(note: &musicxml::Note) -> Option<f64> {
    use musicxml::FermataShape;

    note.notations
        .iter()
        .flat_map(|notations| &notations.fermata)
        .map(|fermata| match fermata.shape {
            FermataShape::DoubleAngled => 1.25,
            FermataShape::Angled | FermataShape::HalfCurve => 1.5,
            FermataShape::Normal | FermataShape::Curlew => 2.0,
            FermataShape::Square | FermataShape::DoubleDot => 3.0,
            FermataShape::DoubleSquare => 4.0,
        })
        .reduce(f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::parser::XmlStream;

    fn lay_out(measures: &str, musescore_compat: bool) -> Vec<MeasureIR> {
        let src = format!(r#"<part id="P1">{measures}</part>"#);
        let parsed = XmlStream::new("test", &src)
            .required::<musicxml::Part>(b"part")
            .unwrap();
        layout(&parsed, &Policy::new(musescore_compat))
    }

    fn quarters(value: f64) -> Time {
        Time::from_divisions(value, 1.0)
    }

    fn notes(measure: &MeasureIR) -> Vec<(Time, Time)> {
        measure
            .notes
            .iter()
            .map(|note| (note.at, note.dur))
            .collect()
    }

    fn note(content: &str, duration: u32) -> String {
        format!("<note>{content}<duration>{duration}</duration></note>")
    }

    const C4: &str = "<pitch><step>C</step><octave>4</octave></pitch>";

    #[test]
    fn chord_tones_share_the_start_and_keep_their_duration() {
        let measures = lay_out(
            &format!(
                "<measure number=\"1\"><attributes><divisions>2</divisions></attributes>{}{}{}</measure>",
                note(C4, 2),
                note(&format!("<chord/>{C4}"), 4),
                note(C4, 2),
            ),
            false,
        );

        assert_eq!(
            notes(&measures[0]),
            [
                (quarters(0.0), quarters(1.0)),
                (quarters(0.0), quarters(2.0)),
                (quarters(1.0), quarters(1.0)),
            ]
        );
        assert_eq!(measures[0].duration, quarters(2.0));
    }

    #[test]
    fn backup_stops_at_the_measure_start_and_the_next_measure_starts_after_the_longest_voice() {
        let measures = lay_out(
            &format!(
                "<measure number=\"1\">{}<backup><duration>2</duration></backup>{}{}</measure>
                 <measure number=\"2\">{}</measure>",
                note(C4, 1),
                note(C4, 1),
                note(C4, 1),
                note(C4, 1),
            ),
            false,
        );

        assert_eq!(
            notes(&measures[0]),
            [
                (quarters(0.0), quarters(1.0)),
                (quarters(0.0), quarters(1.0)),
                (quarters(1.0), quarters(1.0)),
            ]
        );
        assert_eq!(measures[0].duration, quarters(2.0));
        assert_eq!(measures[1].start, quarters(2.0));
    }

    #[test]
    fn graces_take_no_time_unpitched_and_cues_do() {
        let measures = lay_out(
            &format!(
                "<measure number=\"1\">{}{}{}<forward><duration>1</duration></forward>{}</measure>",
                format_args!("<note><grace/>{C4}</note>"),
                note("<unpitched/>", 1),
                note(&format!("<cue/>{C4}"), 1),
                note("<rest/>", 1),
            ),
            false,
        );

        let kinds: Vec<_> = measures[0].notes.iter().map(|note| note.kind).collect();
        assert_eq!(
            kinds,
            [
                NoteKind::Grace,
                NoteKind::Unpitched,
                NoteKind::Cue,
                NoteKind::Rest
            ]
        );
        assert_eq!(
            notes(&measures[0]),
            [
                (quarters(0.0), quarters(0.0)),
                (quarters(0.0), quarters(1.0)),
                (quarters(1.0), quarters(1.0)),
                (quarters(3.0), quarters(1.0)),
            ]
        );
    }

    #[test]
    fn tempo_offset_follows_the_spec_unless_musescore_compat() {
        let measure = format!(
            "<measure number=\"1\">{}
              <direction>
                <direction-type><words>a</words></direction-type>
                <offset>-1</offset>
                <sound tempo=\"60\"/>
              </direction>
              <direction>
                <direction-type><words>b</words></direction-type>
                <offset sound=\"yes\">-1</offset>
                <sound tempo=\"70\"/>
              </direction>
              <direction>
                <direction-type><words>c</words></direction-type>
                <offset>-1</offset>
                <sound tempo=\"80\"><offset>1</offset></sound>
              </direction>
            </measure>",
            note(C4, 2),
        );

        assert_eq!(
            lay_out(&measure, false)[0].controls,
            [
                (quarters(2.0), Control::Tempo(60.0)),
                (quarters(1.0), Control::Tempo(70.0)),
                (quarters(3.0), Control::Tempo(80.0)),
            ]
        );
        assert_eq!(
            lay_out(&measure, true)[0].controls,
            [
                (quarters(1.0), Control::Tempo(60.0)),
                (quarters(1.0), Control::Tempo(70.0)),
                (quarters(1.0), Control::Tempo(80.0)),
            ]
        );
    }
}
