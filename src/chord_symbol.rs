use crate::musicxml::{DegreeType, Harmony, HarmonyChordRoot, HarmonyKind, Step};

/// ret: MIDI keys of a chord symbol
pub fn resolve(harmony: &Harmony) -> Option<Vec<u8>> {
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
        let alter = degree.alter.round() as i32;

        let Ok(value) = u8::try_from(degree.value) else {
            continue;
        };

        if !(1..=13).contains(&value) {
            continue;
        }

        if !(-2..=2).contains(&alter) {
            continue;
        }

        // Per spec https://musicxml.formats.music/musicxml-reference/elements/degree-alter/
        // I believe the we should use dominant intervals,
        // but this causes mismatch with MuseScore output, not sure yet why, but I'll follow my
        // understanding of the spec for now
        let base = match degree.kind {
            DegreeType::Add => dominant_interval(value),
            // Relative to the chord's own degree, if it has one
            DegreeType::Alter => tones
                .iter()
                .find(|(d, _)| *d == value)
                .map_or_else(|| dominant_interval(value), |(_, interval)| *interval),
            DegreeType::Subtract => 0,
        };

        tones.retain(|(d, _)| *d != value);

        if degree.kind != DegreeType::Subtract {
            tones.push((value, base + alter));
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

fn dominant_interval(degree: u8) -> i32 {
    let intervals = [0, 2, 4, 5, 7, 9, 10];
    intervals[usize::from(degree - 1) % intervals.len()]
}

/// ret: (degree, semitones above the root)
fn kind_tones(kind: HarmonyKind) -> &'static [(u8, i32)] {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::XmlStream;

    /// ret: keys of a chord with a single degree
    fn resolve_degree(
        root: &str,
        kind: &str,
        value: u32,
        alter: i32,
        degree_type: &str,
    ) -> Vec<u8> {
        let src = format!(
            "<harmony>
              <root><root-step>{root}</root-step></root>
              <kind>{kind}</kind>
              <degree>
                <degree-value>{value}</degree-value>
                <degree-alter>{alter}</degree-alter>
                <degree-type>{degree_type}</degree-type>
              </degree>
            </harmony>"
        );
        let mut stream = XmlStream::new("test", &src);
        let harmony = stream.required::<Harmony>(b"harmony").unwrap();
        resolve(&harmony).unwrap()
    }

    #[test]
    fn sus4_add_seventh_is_dominant() {
        // G7sus4: G, C, D, F
        assert_eq!(
            resolve_degree("G", "suspended-fourth", 7, 0, "add"),
            [43, 60, 62, 65, 67]
        );
    }

    #[test]
    fn add_is_relative_to_dominant() {
        // C7: C, E, G, Bb
        assert_eq!(
            resolve_degree("C", "major", 7, 0, "add"),
            [36, 60, 64, 67, 70]
        );
    }

    #[test]
    fn alter_is_relative_to_kind() {
        // Cdim(#5): C, Eb, G
        assert_eq!(
            resolve_degree("C", "diminished", 5, 1, "alter"),
            [36, 60, 63, 67]
        );
    }

    #[test]
    fn alter_missing_degree_is_relative_to_dominant() {
        // C(b7) where the chord has no 7th: C, E, G, A
        assert_eq!(
            resolve_degree("C", "major", 7, -1, "alter"),
            [36, 60, 64, 67, 69]
        );
    }
}
