use crate::passes::{MeasureIR, Visit};

/// Might look useless, but the idea is to use this for repeats latter down the line
pub fn playback_order(measures: &[MeasureIR]) -> Vec<Visit> {
    (0..measures.len())
        .map(|measure| Visit { measure })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::passes::Time;

    #[test]
    fn every_measure_once() {
        let measure = MeasureIR {
            start: Time::ZERO,
            duration: Time::ZERO,
            notes: Vec::new(),
            controls: Vec::new(),
        };

        assert_eq!(
            playback_order(&[measure.clone(), measure]),
            [Visit { measure: 0 }, Visit { measure: 1 }]
        );
    }
}
