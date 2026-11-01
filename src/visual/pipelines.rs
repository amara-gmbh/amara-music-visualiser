//! Finite shader preparation, outside the frame and renderer locks.
use super::{HISTORY_FORMAT, WORLD_SHADER_SOURCE, wgpu};
use crossbeam_channel::{Receiver, Sender, bounded};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

const PROCEDURAL: [usize; 12] = [0, 1, 2, 3, 8, 9, 10, 11, 13, 14, 15, 16];
pub(super) const COUNT: usize =
    1 + PROCEDURAL.len() + PROCEDURAL.len() * (PROCEDURAL.len() - 1) / 2;

pub(super) fn world_mask(world: usize) -> u32 {
    if PROCEDURAL.contains(&world) {
        1 << world
    } else {
        0
    }
}

pub(super) fn frame_mask(from: f32, to: f32, amount: f32) -> u32 {
    let destination = world_mask(to as usize);
    if amount >= 1.0 || from == to {
        destination
    } else {
        world_mask(from as usize) | destination
    }
}

pub(super) fn masks(initial: u32) -> Vec<u32> {
    let mut result = vec![0];
    result.extend(PROCEDURAL.map(world_mask));
    for (i, a) in PROCEDURAL.into_iter().enumerate() {
        for b in &PROCEDURAL[i + 1..] {
            result.push(world_mask(a) | world_mask(*b));
        }
    }
    if let Some(index) = result.iter().position(|mask| *mask == initial) {
        result.swap(0, index);
    }
    result
}

fn has_bridge(mask: u32) -> bool {
    matches!(mask, 12 | 260 | 264)
}

pub(super) fn build(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    mask: u32,
) -> wgpu::RenderPipeline {
    let source = WORLD_SHADER_SOURCE
        .replace(
            "const included_worlds: u32 = 126735u;",
            &format!("const included_worlds: u32 = {mask}u;"),
        )
        .replace(
            "const implicit_bridges: bool = true;",
            &format!("const implicit_bridges: bool = {};", has_bridge(mask)),
        );
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("bounded world pair"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("prepared world pair"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(HISTORY_FORMAT.into())],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

pub(super) struct Preparation {
    pub requests: Sender<u32>,
    pub ready: Receiver<(u32, Result<wgpu::RenderPipeline, String>)>,
    pub completed: Arc<AtomicUsize>,
    cancelled: Arc<AtomicBool>,
}

impl Preparation {
    pub fn start(
        device: wgpu::Device,
        layout: wgpu::PipelineLayout,
        initial: u32,
    ) -> std::io::Result<Self> {
        let (requests, requested) = bounded(16);
        let (results, ready) = bounded(2);
        let cancelled = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&cancelled);
        let completed = Arc::new(AtomicUsize::new(0));
        let progress = Arc::clone(&completed);
        std::thread::Builder::new()
            .name("amara shader preparation".into())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut remaining = masks(initial);
                    while !remaining.is_empty() && !stop.load(Ordering::Relaxed) {
                        // Only already-planned keys can be promoted. Each is built at most once.
                        for priority in requested.try_iter() {
                            if let Some(index) = remaining.iter().position(|mask| *mask == priority)
                            {
                                remaining.swap(0, index);
                            }
                        }
                        let mask = remaining.remove(0);
                        let memory_scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
                        let internal_scope = device.push_error_scope(wgpu::ErrorFilter::Internal);
                        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
                        let pipeline = build(&device, &layout, mask);
                        let validation = pollster::block_on(scope.pop());
                        let internal = pollster::block_on(internal_scope.pop());
                        let memory = pollster::block_on(memory_scope.pop());
                        let result = match validation.or(internal).or(memory) {
                            Some(error) => Err(error.to_string()),
                            None => Ok(pipeline),
                        };
                        if stop.load(Ordering::Relaxed) || results.send((mask, result)).is_err() {
                            break;
                        }
                        progress.fetch_add(1, Ordering::Relaxed);
                    }
                }));
                if outcome.is_err() {
                    log::warn!(
                        "World preparation stopped unexpectedly; general renderer remains available"
                    );
                }
                // This is completed preparation attempts, not a success counter.
                // Failed keys log their error and retain the general fallback.
                progress.store(COUNT, Ordering::Relaxed);
            })?;
        Ok(Self {
            requests,
            ready,
            completed,
            cancelled,
        })
    }
}

impl Drop for Preparation {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_actual_pair_is_in_the_unique_finite_set() {
        let keys = masks(1 << 11);
        assert_eq!(keys.len(), COUNT);
        assert_eq!(keys[0], 1 << 11);
        assert_eq!(
            keys.iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            COUNT
        );
        for a in 0..crate::presets::WORLD_COUNT {
            for b in 0..crate::presets::WORLD_COUNT {
                assert!(keys.contains(&frame_mask(a as f32, b as f32, 0.5)));
                assert_eq!(
                    frame_mask(a as f32, b as f32, 0.0),
                    world_mask(a) | world_mask(b)
                );
                assert_eq!(frame_mask(a as f32, b as f32, 1.0), world_mask(b));
            }
        }
    }
    #[test]
    fn procedural_and_raster_worlds_partition_registry() {
        let raster = [4, 5, 6, 7, 12];
        for world in 0..crate::presets::WORLD_COUNT {
            assert_ne!(PROCEDURAL.contains(&world), raster.contains(&world));
        }
        assert_eq!(PROCEDURAL.len() + raster.len(), crate::presets::WORLD_COUNT);
    }
    #[test]
    fn implicit_bridges_are_only_the_three_original_pairs() {
        for a in 0..crate::presets::WORLD_COUNT {
            for b in 0..crate::presets::WORLD_COUNT {
                let expected =
                    matches!((a, b), (2, 3) | (3, 2) | (2, 8) | (8, 2) | (3, 8) | (8, 3));
                assert_eq!(has_bridge(frame_mask(a as f32, b as f32, 0.5)), expected);
            }
        }
    }
}
