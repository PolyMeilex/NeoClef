use crate::musicxml::{DegreeType, Harmony, HarmonyChordRoot, HarmonyKind, Step};

/// ret: MIDI keys of a chord symbol
pub fn realize(harmony: &Harmony) -> Option<Vec<u8>> {
    // Like MuseScore, play only the first of stacked chords (V of II), and only if all have roots
    let mut roots = harmony.chords.iter().map(|chord| match &chord.root {
        HarmonyChordRoot::Root(root)
            if chord.kind.value != HarmonyKind::None && root.step.text.as_deref() != Some("") =>
        {
            pitch_class(root.step.step, root.alter.unwrap_or(0.0))
        }
        _ => None,
    });

    let root = roots.next()??;
    if roots.any(|root| root.is_none()) {
        return None;
    }

    let chord = harmony.chords.first()?;

    let mut tones = kind_tones(chord.kind.value).to_vec();

    for degree in &chord.degrees {
        let value = degree.value as i32;
        let alter = degree.alter.round() as i32;

        // MuseScore's importer drops these
        if !(1..=13).contains(&value) || !(-2..=2).contains(&alter) {
            continue;
        }

        // The 7th of a sus chord is the dominant one, `7sus4`
        let suspended = matches!(
            chord.kind.value,
            HarmonyKind::SuspendedSecond | HarmonyKind::SuspendedFourth
        );

        let alter = if suspended && value == 7 && degree.kind == DegreeType::Add {
            alter - 1
        } else {
            alter
        };

        tones.retain(|(d, _)| *d != value);
        if degree.kind != DegreeType::Subtract {
            tones.push((value, degree_interval(value) + alter));
        }
    }

    let root = root.rem_euclid(12);
    let bass = chord
        .bass
        .as_ref()
        .and_then(|bass| pitch_class(bass.step, bass.alter.unwrap_or(0.0)));

    let mut keys = vec![bass.unwrap_or(root) + 36, root + 60];
    keys.extend(
        tones
            .iter()
            .map(|(_, interval)| (root + interval).rem_euclid(12) + 60),
    );

    keys.sort();

    Some(
        keys.into_iter()
            .map(|key| key.clamp(0, 127) as u8)
            .collect(),
    )
}

fn pitch_class(step: Step, alter: f64) -> Option<i32> {
    let alter = alter.round() as i32;
    if !(-2..=2).contains(&alter) {
        return None;
    }
    let base = match step {
        Step::C => 0,
        Step::D => 2,
        Step::E => 4,
        Step::F => 5,
        Step::G => 7,
        Step::A => 9,
        Step::B => 11,
    };
    Some(base + alter)
}

/// Semitones from the root to an unaltered degree of a major scale
fn degree_interval(degree: i32) -> i32 {
    [0, 2, 4, 5, 7, 9, 11][((degree - 1) % 7) as usize]
}

/// ret: (degree, semitones above the root)
fn kind_tones(kind: HarmonyKind) -> &'static [(i32, i32)] {
    use HarmonyKind::*;
    match kind {
        Major | Pedal | Italian => &[(3, 4), (5, 7)],
        Minor => &[(3, 3), (5, 7)],
        Augmented => &[(3, 4), (5, 8)],
        Diminished => &[(3, 3), (5, 6)],
        Dominant => &[(3, 4), (5, 7), (7, 10)],
        MajorSeventh => &[(3, 4), (5, 7), (7, 11)],
        MinorSeventh => &[(3, 3), (5, 7), (7, 10)],
        DiminishedSeventh => &[(3, 3), (5, 6), (7, 9)],
        AugmentedSeventh => &[(3, 4), (5, 8), (7, 10)],
        HalfDiminished => &[(3, 3), (5, 6), (7, 10)],
        MajorMinor => &[(3, 3), (5, 7), (7, 11)],
        MajorSixth => &[(3, 4), (5, 7), (6, 9)],
        MinorSixth => &[(3, 3), (5, 7), (6, 9)],
        DominantNinth => &[(3, 4), (5, 7), (7, 10), (9, 2)],
        MajorNinth => &[(3, 4), (5, 7), (7, 11), (9, 2)],
        MinorNinth => &[(3, 3), (5, 7), (7, 10), (9, 2)],
        Dominant11th => &[(3, 4), (5, 7), (7, 10), (9, 2), (11, 5)],
        Major11th => &[(3, 4), (5, 7), (7, 11), (9, 2), (11, 5)],
        Minor11th => &[(3, 3), (5, 7), (7, 10), (9, 2), (11, 5)],
        Dominant13th => &[(3, 4), (5, 7), (7, 10), (9, 2), (11, 5), (13, 9)],
        Major13th => &[(3, 4), (5, 7), (7, 11), (9, 2), (11, 5), (13, 9)],
        Minor13th => &[(3, 3), (5, 7), (7, 10), (9, 2), (11, 5), (13, 9)],
        SuspendedSecond => &[(2, 2), (5, 7)],
        SuspendedFourth => &[(4, 5), (5, 7)],
        Power => &[(5, 7)],
        // MuseScore doesn't understand these and plays just the root
        Neapolitan | French | German | Tristan | Other | None => &[],
    }
}
