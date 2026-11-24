use std::collections::BTreeSet;

use crate::parsing::musicxml::{self, MeasureItem};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnsupportedFeature {
    /// `<repeat>`
    Repeats,
    /// `<ending>`
    Endings,
    /// `<barline segno>`, `<barline coda>`
    SegnoCoda,
    /// `<barline><fermata>`
    BarlineFermata,
    /// `<tie time-only>`
    TieTimeOnly,
    /// `<grace>` after a note
    GracesAfterNote,
    /// `<grace steal-time-previous>`, `<grace steal-time-following>`, `<grace make-time>`
    GraceTiming,
    /// `<unpitched>`
    UnpitchedNotes,
    /// Non-integer `<alter>`
    Microtones,
    /// `<trill-mark>`, `<shake>`, `<wavy-line>`
    Trills,
    /// `<turn>`, `<delayed-turn>`, `<inverted-turn>`, `<delayed-inverted-turn>`, `<vertical-turn>`, `<inverted-vertical-turn>`
    Turns,
    /// `<mordent>`, `<inverted-mordent>`
    Mordents,
    /// `<tremolo>`
    Tremolo,
    /// `<schleifer>`, `<haydn>`, `<other-ornament>`
    OtherOrnaments,
    /// `<arpeggiate>`
    Arpeggio,
    /// `<glissando>`, `<slide>`
    Glissando,
    /// `<staccato>`
    Staccato,
    /// `<dynamics>`
    DynamicsMarks,
    /// `<harmony>` with multiple chords
    StackedChordSymbols,
    /// `<midi-instrument>`
    MidiInstrument,
}

impl std::fmt::Display for UnsupportedFeature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Default)]
pub struct UnsupportedFeatures(BTreeSet<UnsupportedFeature>);

impl UnsupportedFeatures {
    pub(crate) fn insert(&mut self, feature: UnsupportedFeature) {
        self.0.insert(feature);
    }

    pub fn as_set(&self) -> &BTreeSet<UnsupportedFeature> {
        &self.0
    }
}

pub fn unsupported(
    score: &musicxml::ScorePartwise,
    part: &musicxml::Part,
    found: &mut UnsupportedFeatures,
) {
    use UnsupportedFeature as F;

    if score
        .part_list
        .score_part
        .iter()
        .any(|part| !part.midi_instrument.is_empty())
    {
        found.insert(F::MidiInstrument);
    }

    for item in part.measure.iter().flat_map(|measure| &measure.content) {
        match item {
            MeasureItem::Note(note) => self::note(note, found),
            MeasureItem::Barline(barline) => {
                if barline.repeat.is_some() {
                    found.insert(F::Repeats);
                }
                if barline.ending.is_some() {
                    found.insert(F::Endings);
                }
                if barline.segno || barline.coda {
                    found.insert(F::SegnoCoda);
                }
                if !barline.fermata.is_empty() {
                    found.insert(F::BarlineFermata);
                }
                if barline.wavy_line.is_some() {
                    found.insert(F::Trills);
                }
            }
            MeasureItem::Direction(direction) => {
                if direction
                    .direction_type
                    .iter()
                    .any(|kind| matches!(kind, musicxml::DirectionType::Dynamics(_)))
                {
                    found.insert(F::DynamicsMarks);
                }
            }
            MeasureItem::Harmony(harmony) => {
                if harmony.chords.len() > 1 {
                    found.insert(F::StackedChordSymbols);
                }
            }
            MeasureItem::Print(_)
            | MeasureItem::Attributes(_)
            | MeasureItem::Backup(_)
            | MeasureItem::Forward(_) => {}
        }
    }
}

fn note(note: &musicxml::Note, found: &mut UnsupportedFeatures) {
    use UnsupportedFeature as Feature;
    use musicxml::{NoteKindState, NoteKindStatePitchKind, Ornament};

    let (kind, tie) = match &note.kind {
        NoteKindState::Regular(state) => (&state.kind, state.tie.as_slice()),
        NoteKindState::Cue(_) => return,
        NoteKindState::Grace(state) => {
            grace(&state.grace, found);
            (&state.kind, state.tie.as_slice())
        }
        NoteKindState::GraceCue(_) => return,
    };

    match kind {
        NoteKindStatePitchKind::Unpitched(_) => {
            found.insert(Feature::UnpitchedNotes);
        }
        NoteKindStatePitchKind::Pitch(pitch) => {
            if pitch.alter.is_some_and(|alter| alter.fract() != 0.0) {
                found.insert(Feature::Microtones);
            }
        }
        NoteKindStatePitchKind::Rest(_) => {}
    }

    if tie.iter().any(|tie| tie.time_only.is_some()) {
        found.insert(Feature::TieTimeOnly);
    }

    for notations in &note.notations {
        if !notations.glissando.is_empty() || !notations.slide.is_empty() {
            found.insert(Feature::Glissando);
        }

        if !notations.arpeggiate.is_empty() {
            found.insert(Feature::Arpeggio);
        }

        if !notations.dynamics.is_empty() {
            found.insert(Feature::DynamicsMarks);
        }

        if notations
            .articulations
            .iter()
            .any(|articulations| articulations.staccato > 0)
        {
            found.insert(Feature::Staccato);
        }

        for ornament in notations.ornaments.iter().flat_map(|o| &o.items) {
            found.insert(match ornament {
                Ornament::TrillMark | Ornament::Shake | Ornament::WavyLine(_) => Feature::Trills,
                Ornament::Turn
                | Ornament::DelayedTurn
                | Ornament::InvertedTurn
                | Ornament::DelayedInvertedTurn
                | Ornament::VerticalTurn
                | Ornament::InvertedVerticalTurn => Feature::Turns,
                Ornament::Mordent | Ornament::InvertedMordent => Feature::Mordents,
                Ornament::Tremolo(_) => Feature::Tremolo,
                Ornament::Schleifer | Ornament::Haydn | Ornament::Other(_) => {
                    Feature::OtherOrnaments
                }
            });
        }
    }
}

fn grace(grace: &musicxml::Grace, found: &mut UnsupportedFeatures) {
    if grace.steal_time_previous.is_some()
        || grace.steal_time_following.is_some()
        || grace.make_time.is_some()
    {
        found.insert(UnsupportedFeature::GraceTiming);
    }
}
