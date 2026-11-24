use std::path::{Path, PathBuf};

use clap::Parser;
use midly::{MidiMessage, Smf, TrackEventKind};
use pretty_assertions::Comparison;

#[derive(Parser)]
struct Args {
    a: PathBuf,
    b: PathBuf,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct NoteOn {
    track: usize,
    tick: u32,
    channel: u8,
    key: u8,
}

impl std::fmt::Debug for NoteOn {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "track={} tick={} ch={} key={}",
            self.track, self.tick, self.channel, self.key
        )
    }
}

fn load(path: &Path) -> Smf<'static> {
    let bytes = std::fs::read(path).unwrap();
    Smf::parse(&bytes).unwrap().make_static()
}

fn note_ons(smf: &Smf) -> Vec<NoteOn> {
    let mut notes = Vec::new();
    for (track, events) in smf.tracks.iter().enumerate() {
        let mut tick = 0;
        for event in events {
            tick += event.delta.as_int();
            if let TrackEventKind::Midi {
                channel,
                message: MidiMessage::NoteOn { key, vel },
            } = event.kind
                && vel > 0
            {
                notes.push(NoteOn {
                    track,
                    tick,
                    channel: channel.as_int(),
                    key: key.as_int(),
                });
            }
        }
    }
    notes
}

fn main() -> Result<(), ()> {
    let args = Args::parse();
    let a = load(&args.a);
    let b = load(&args.b);

    let mut a = note_ons(&a);
    let mut b = note_ons(&b);
    a.sort();
    b.sort();

    if a != b {
        println!("{}", Comparison::new(&a, &b));
        Err(())
    } else {
        Ok(())
    }
}
