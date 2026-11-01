use crate::dsp::MusicDna;
use crossbeam_channel::{Receiver, Sender, bounded};
use std::thread;

#[derive(Debug, Clone)]
pub struct DirectorInput {
    pub source_epoch: u64,
    pub dna: MusicDna,
    pub energy: f32,
    pub bass: f32,
    pub transient: f32,
}

#[derive(Debug, Clone)]
pub struct Direction {
    pub source_epoch: u64,
    pub scene: usize,
    pub hue: f32,
    pub complexity: f32,
    pub glow: f32,
    pub note: String,
}

pub struct AsyncDirector {
    input: Sender<DirectorInput>,
    output: Receiver<Direction>,
}

/// A local ML or language-model implementation can replace this backend.
/// The worker owns it: model loading and inference never enter the UI/GPU loop.
pub trait DirectorBackend: Send + 'static {
    fn direct(&mut self, input: &DirectorInput) -> Direction;
}

struct LocalRules;

impl DirectorBackend for LocalRules {
    fn direct(&mut self, input: &DirectorInput) -> Direction {
        direct(input)
    }
}

impl AsyncDirector {
    pub fn new() -> Self {
        Self::with_backend(LocalRules)
    }

    pub fn with_backend(mut backend: impl DirectorBackend) -> Self {
        let (input_tx, input_rx) = bounded::<DirectorInput>(2);
        let (output_tx, output_rx) = bounded::<Direction>(2);
        thread::Builder::new()
            .name("creative-director".into())
            .spawn(move || {
                while let Ok(mut input) = input_rx.recv() {
                    while let Ok(newer) = input_rx.try_recv() {
                        input = newer;
                    }
                    let direction = backend.direct(&input);
                    let _ = output_tx.try_send(direction);
                }
            })
            .expect("director worker should start");
        Self {
            input: input_tx,
            output: output_rx,
        }
    }

    pub fn submit(&self, input: DirectorInput) {
        let _ = self.input.try_send(input);
    }

    pub fn latest(&self) -> Option<Direction> {
        self.output.try_iter().last()
    }
}

fn direct(input: &DirectorInput) -> Direction {
    let scene = if input.energy < 0.17 {
        0
    } else if input.dna.danceability > 0.58 {
        3
    } else if input.dna.brightness > 0.52 {
        1
    } else {
        2
    };
    let note = match scene {
        0 => "Opening space for a restrained, slow passage",
        1 => "Unfolding brighter harmonic geometry",
        2 => "Building a cinematic plasma horizon",
        _ => "Locking the world to the rhythmic drive",
    };
    Direction {
        source_epoch: input.source_epoch,
        scene,
        hue: (0.68 + input.dna.brightness * 0.42 + input.bass * 0.08).fract(),
        complexity: (0.38 + input.transient * 0.75 + input.energy * 0.3).clamp(0.3, 1.4),
        glow: (0.42 + input.energy * 0.75 + input.dna.brightness * 0.24).clamp(0.35, 1.45),
        note: note.into(),
    }
}
