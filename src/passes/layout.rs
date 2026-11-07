use std::collections::{BTreeMap, HashMap, HashSet};

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
    let mut voices = VoiceStaves::default();
    let mut graces = Graces {
        silent_appoggiaturas: policy.silent_appoggiaturas,
        ..Graces::default()
    };

    let mut laid: Vec<MeasureIR> = part
        .measure
        .iter()
        .enumerate()
        .map(|(index, content)| {
            let laid = measure(
                index,
                content,
                start,
                &mut attributes,
                policy,
                &mut voices,
                &mut graces,
            );
            start = laid.start + laid.duration;
            laid
        })
        .collect();

    if policy.staff_from_voice {
        let home = voices.home_staves();
        for (measure, voices) in laid.iter_mut().zip(&voices.notes) {
            for (note, voice) in measure.notes.iter_mut().zip(voices) {
                if let Some(&staff) = home.get(voice) {
                    note.staff = staff;
                }
            }
        }
    }

    laid
}

fn measure<'a>(
    index: usize,
    measure: &'a musicxml::Measure,
    start: Time,
    attributes: &mut Attributes,
    policy: &Policy,
    voice_staves: &mut VoiceStaves<'a>,
    graces: &mut Graces<'a>,
) -> MeasureIR {
    let mut laid = MeasureIR {
        start,
        duration: Time::ZERO,
        notes: Vec::new(),
        controls: Vec::new(),
    };

    let mut cursor = Time::ZERO;
    let mut chord_at = Time::ZERO;
    // How late the current chord sounds because of the appoggiaturas before it
    let mut chord_delay = Time::ZERO;
    let mut voices = Vec::new();

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
                let voice = note.voice.as_deref();

                let (at, dur) = if parts.kind == NoteKind::Grace {
                    // Placed by `graces.lead_into` once the note it leads into is known
                    graces.wait(laid.notes.len(), note, parts.chord);
                    (cursor, Time::ZERO)
                } else if parts.chord {
                    (chord_at + chord_delay, time(parts.duration) - chord_delay)
                } else {
                    let dur = time(parts.duration);
                    chord_at = cursor;
                    cursor += dur;
                    laid.duration = laid.duration.max(cursor);
                    chord_delay = graces.lead_into(&mut laid.notes, voice, chord_at, dur);
                    graces.set_previous(voice, note, parts.kind, dur);
                    (chord_at + chord_delay, dur - chord_delay)
                };

                voices.push((
                    Voice::new(note.voice.as_deref()),
                    !parts.chord && parts.kind != NoteKind::Grace,
                ));
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

    voice_staves.process_measure(&laid.notes, voices);
    // Graces after the last note of a measure aren't supported, see `Graces`
    graces.waiting.clear();
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

/// Even tho the spec says otherwise, a MusicXML voice `<voice>1</voice>` and `<voice>01</voice>` are usually treated as the same...
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Voice<'a> {
    Number(u64),
    Name(&'a str),
}

impl<'a> Voice<'a> {
    fn new(voice: Option<&'a str>) -> Self {
        match voice.map(str::trim) {
            None | Some("") => Self::Number(1),
            Some(voice) => voice
                .parse()
                .map_or_else(|_| Self::Name(voice), Self::Number),
        }
    }
}

#[derive(Debug, Default)]
struct Graces<'a> {
    /// The graces waiting for the note they lead into
    waiting: Vec<WaitingGrace>,
    /// The notated duration of the previous chord in each voice, a rest clears it
    previous: HashMap<Option<&'a str>, Time>,
    /// See `Policy::silent_appoggiaturas`
    silent_appoggiaturas: bool,
}

#[derive(Debug)]
struct WaitingGrace {
    /// Index in the measure's notes
    index: usize,
    /// Whether it starts a grace chord, `false` for its chord tones
    starts_chord: bool,
    notated: Time,
    slash: bool,
}

impl<'a> Graces<'a> {
    fn wait(&mut self, index: usize, note: &musicxml::Note, chord: bool) {
        // Cue graces don't sound, so they don't take time from their note either
        let musicxml::NoteKindState::Grace(state) = &note.kind else {
            return;
        };
        let grace = &state.grace;
        let slash = grace.slash == Some(true);

        self.waiting.push(WaitingGrace {
            index,
            starts_chord: !chord,
            // MuseScore reads a grace without a `<type>` as an eighth
            notated: notated_duration(note).unwrap_or(Time::from_divisions(1.0, 2.0)),
            slash,
        });

        // Like MuseScore, this grace and the ones before it are graces after the previous note
        let steals_previous = grace.steal_time_previous.is_some() && !slash;
        if grace.steal_time_following.is_none() && (steals_previous || ends_slur_or_trill(note)) {
            self.waiting.clear();
        }
    }

    fn set_previous(
        &mut self,
        voice: Option<&'a str>,
        note: &musicxml::Note,
        kind: NoteKind,
        dur: Time,
    ) {
        if kind == NoteKind::Rest {
            self.previous.remove(&voice);
        } else {
            self.previous
                .insert(voice, notated_duration(note).unwrap_or(dur));
        }
    }

    /// Places the waiting graces before the note at `at`, returns how late that note starts
    fn lead_into(
        &mut self,
        notes: &mut [NoteEv],
        voice: Option<&'a str>,
        at: Time,
        dur: Time,
    ) -> Time {
        let waiting = std::mem::take(&mut self.waiting);
        let Some(first) = waiting.first() else {
            return Time::ZERO;
        };
        let chords = waiting
            .iter()
            .filter(|grace| grace.starts_chord)
            .count()
            .max(1);

        // When the first grace chord starts, how long each one lasts, and the note's delay
        let (start, each, delay) = if first.slash {
            let total = match self.previous.get(&voice) {
                Some(&previous) => first.notated.min(previous * 0.5),
                None => first.notated,
            };
            let each = (total * (1.0 / chords as f64)).floor_to_ticks();
            (at - total, each, Time::ZERO)
        } else {
            // MuseScore works in whole thousandths of the note
            let delay = (first.notated.ratio(dur) * 1000.0).floor().min(500.0);
            let each = (delay / chords as f64).floor();
            let thousandths = |n: f64| (dur * (n / 1000.0)).floor_to_ticks();
            (at, thousandths(each), thousandths(delay))
        };

        // No duration, so `sounding_notes` leaves them out
        let each = if !first.slash && self.silent_appoggiaturas {
            Time::ZERO
        } else {
            each
        };

        let mut chord = 0;
        for (i, grace) in waiting.iter().enumerate() {
            if grace.starts_chord && i > 0 {
                chord += 1;
            }
            let note = &mut notes[grace.index];
            note.at = start + each * chord as f64;
            note.dur = each;
        }

        delay
    }
}

/// Whether a note ends a slur (without starting one) or a trill's wavy line
fn ends_slur_or_trill(note: &musicxml::Note) -> bool {
    use musicxml::{Ornament, StartStopContinue};

    let slurs = || note.notations.iter().flat_map(|notations| &notations.slurs);
    let slur_stops = slurs().any(|slur| slur.kind == StartStopContinue::Stop);
    let slur_starts = slurs().any(|slur| slur.kind == StartStopContinue::Start);
    let wavy_line_stops = note
        .notations
        .iter()
        .flat_map(|notations| &notations.ornaments)
        .flat_map(|ornaments| &ornaments.items)
        .any(|ornament| {
            matches!(ornament, Ornament::WavyLine(line) if line.kind == StartStopContinue::Stop)
        });

    (slur_stops && !slur_starts) || wavy_line_stops
}

/// The duration a note's `<type>` and `<dot>`s show, ignoring tuplets
fn notated_duration(note: &musicxml::Note) -> Option<Time> {
    let quarters = match note.note_type.as_deref()? {
        "maxima" => 32.0,
        "long" => 16.0,
        "breve" => 8.0,
        "whole" => 4.0,
        "half" => 2.0,
        "quarter" => 1.0,
        "eighth" => 0.5,
        "16th" => 0.25,
        "32nd" => 0.125,
        "64th" => 1.0 / 16.0,
        "128th" => 1.0 / 32.0,
        "256th" => 1.0 / 64.0,
        "512th" => 1.0 / 128.0,
        "1024th" => 1.0 / 256.0,
        _ => return None,
    };
    // Each dot adds half of the previous value
    let dotted = quarters * (2.0 - 0.5f64.powi(note.dot as i32));
    Some(Time::from_divisions(dotted, 1.0))
}

/// MuseScore inspired voice to staff mapping
///
/// MuseScore gives each voice of a part one home staff, the one it has the most chords and rests on
#[derive(Debug, Default)]
struct VoiceStaves<'a> {
    /// The voice of each laid note, per measure
    notes: Vec<Vec<Voice<'a>>>,
    /// Chords and rests per voice and staff
    counts: HashMap<Voice<'a>, BTreeMap<u8, usize>>,
    overlapping: HashSet<Voice<'a>>,
}

impl<'a> VoiceStaves<'a> {
    fn home_staves(&self) -> HashMap<Voice<'a>, u8> {
        self.counts
            .iter()
            .filter(|(voice, _)| !self.overlapping.contains(voice))
            .filter_map(|(voice, counts)| {
                let max = counts.values().max()?;
                let (&staff, _) = counts.iter().find(|(_, count)| *count == max)?;
                Some((voice.clone(), staff))
            })
            .collect()
    }

    fn process_measure(&mut self, notes: &[NoteEv], voices: Vec<(Voice<'a>, bool)>) {
        let mut spans: HashMap<&Voice, Vec<(u8, Time, Time)>> = HashMap::new();
        for (note, (voice, counted)) in notes.iter().zip(&voices) {
            if *counted && note.dur > Time::ZERO {
                *self
                    .counts
                    .entry(voice.clone())
                    .or_default()
                    .entry(note.staff)
                    .or_default() += 1;
                spans
                    .entry(voice)
                    .or_default()
                    .push((note.staff, note.at, note.at + note.dur));
            }
        }

        for (voice, spans) in spans {
            let overlaps = spans
                .iter()
                .any(|a| spans.iter().any(|b| a.0 != b.0 && a.1 < b.2 && b.1 < a.2));
            if overlaps {
                self.overlapping.insert(voice.clone());
            }
        }

        self.notes
            .push(voices.into_iter().map(|(voice, _)| voice).collect());
    }
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
        // The grace (an appoggiatura, an eighth without a `<type>`) delays the unpitched note
        assert_eq!(
            notes(&measures[0]),
            [
                (quarters(0.0), quarters(0.5)),
                (quarters(0.5), quarters(0.5)),
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

    #[test]
    fn cross_staff_notes_stay_on_their_voice_home_staff_if_musescore_compat() {
        let on = |voice: u8, staff: u8, chord: &str| {
            format!(
                "<note>{chord}{C4}<duration>1</duration><voice>{voice}</voice><staff>{staff}</staff></note>"
            )
        };
        // Voice 1 is mostly on staff 1 and crosses to staff 2. Voice 2 is mostly on staff 2,
        // its chord tones and graces on staff 1 don't count. Voice 3 is on both staves at the
        // same time, so it has no home staff.
        let measure = format!(
            "<measure number=\"1\"><attributes><staves>2</staves></attributes>
               {}{}{}<backup><duration>3</duration></backup>
               {}{}{}{}<backup><duration>2</duration></backup>
               {}<backup><duration>1</duration></backup>{}
             </measure>",
            on(1, 1, ""),
            on(1, 2, ""),
            on(1, 1, ""),
            on(2, 2, ""),
            on(2, 1, "<chord/>"),
            format_args!("<note><grace/>{C4}<voice>2</voice><staff>1</staff></note>"),
            on(2, 2, ""),
            on(3, 1, ""),
            on(3, 2, ""),
        );
        let staves = |musescore_compat| -> Vec<u8> {
            lay_out(&measure, musescore_compat)[0]
                .notes
                .iter()
                .map(|note| note.staff)
                .collect()
        };

        assert_eq!(staves(false), [1, 2, 1, 2, 1, 1, 2, 1, 2]);
        assert_eq!(staves(true), [1, 1, 1, 2, 2, 2, 2, 1, 2]);
    }
}
