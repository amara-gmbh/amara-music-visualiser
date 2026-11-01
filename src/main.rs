#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod attractor;
mod audio;
mod auto_journey;
mod background;
mod choreography;
mod director;
mod dsp;
mod frame_clock;
mod look_journey;
mod material_colour;
mod math_journey;
mod meters;
mod motion;
mod music_response;
#[cfg(test)]
mod musical_events; // Retired overlay controller retained only for regression history.
mod presets;
mod ripples;
mod scene;
mod test_track;
mod visual;
mod vocal_response;
mod voice;

use app::MusicVisualiserApp;
use eframe::egui;

const USAGE: &str = "amaraMusicVisualiser\nRun without arguments to open the app.\nDiagnostics: --verify-render, --verify-specialization, --preview-magic, --preview-media, --preview-water, --preview-aurora, --preview-mandelbrot, --preview-mandelbrot-auto, --preview-new-worlds, --preview-disco-silk, --preview-background, --preview-chromatic, --preview-attractor, --preview-music-response, --preview-neon-coast (each accepts an output folder; Silk also accepts a Glass reference PNG); --verify-glass <output folder> <reference PNG>; --verify-audio [1–30 seconds]; --benchmark-render, --benchmark-attractor, --benchmark-new-worlds, --benchmark-disco-silk, --benchmark-aurora, --benchmark-neon-coast, --benchmark-specialization.";

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .init();

    let mut arguments = std::env::args_os().skip(1);
    if let Some(command) = arguments.next() {
        if command == "--help" || command == "-h" {
            println!("{USAGE}");
            println!("--preview-aurora-response <output folder> reruns only Aurora music checks.");
            return Ok(());
        }
        if command == "--verify-render" {
            let destination = arguments
                .next()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/validation"));
            return visual::verify_render(&destination);
        }
        if command == "--verify-audio" {
            let seconds = arguments
                .next()
                .map(|value| value.to_string_lossy().parse::<u64>())
                .transpose()
                .map_err(|_| anyhow::anyhow!("Audio check duration must be 1–30 seconds"))?
                .unwrap_or(10);
            anyhow::ensure!(
                (1..=30).contains(&seconds),
                "Audio check duration must be 1–30 seconds"
            );
            anyhow::ensure!(
                arguments.next().is_none(),
                "Unexpected audio check argument"
            );
            return audio::verify_system_audio(std::time::Duration::from_secs(seconds));
        }
        if command == "--benchmark-render" {
            return visual::benchmark_render();
        }
        if command == "--benchmark-attractor" {
            return visual::benchmark_attractor();
        }
        if command == "--benchmark-new-worlds" {
            return visual::benchmark_new_worlds();
        }
        if command == "--benchmark-disco-silk" {
            return visual::benchmark_disco_silk();
        }
        if command == "--benchmark-aurora" {
            return visual::benchmark_aurora();
        }
        if command == "--benchmark-neon-coast" {
            return visual::benchmark_neon_coast();
        }
        if command == "--benchmark-specialization" {
            return visual::benchmark_specialization();
        }
        if command == "--preview-magic" {
            let destination = arguments
                .next()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/magic-preview"));
            return visual::preview_magic(&destination);
        }
        if command == "--preview-water" {
            let destination = arguments
                .next()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/water-preview"));
            return visual::preview_water(&destination);
        }
        if command == "--preview-aurora" {
            let destination = arguments
                .next()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/aurora-preview"));
            return visual::preview_aurora(&destination);
        }
        if command == "--preview-media" {
            let destination = arguments
                .next()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/media-preview"));
            return visual::preview_media(&destination);
        }
        if command == "--preview-mandelbrot" || command == "--preview-mandelbrot-auto" {
            let destination = arguments
                .next()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/mandelbrot-preview"));
            return if command == "--preview-mandelbrot-auto" {
                visual::preview_mandelbrot_auto(&destination)
            } else {
                visual::preview_mandelbrot(&destination)
            };
        }
        if command == "--preview-new-worlds" {
            let destination = arguments
                .next()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/new-worlds-preview"));
            return visual::preview_new_worlds(&destination);
        }
        if command == "--preview-disco-silk" {
            let destination = arguments
                .next()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/disco-silk-preview"));
            let glass_reference = arguments.next().map(std::path::PathBuf::from);
            return visual::preview_disco_silk(&destination, glass_reference.as_deref());
        }
        if command == "--verify-glass" {
            let destination = arguments
                .next()
                .map(std::path::PathBuf::from)
                .ok_or_else(|| {
                    anyhow::anyhow!("--verify-glass needs an output folder and reference PNG")
                })?;
            let reference = arguments
                .next()
                .map(std::path::PathBuf::from)
                .ok_or_else(|| anyhow::anyhow!("--verify-glass needs a reference PNG"))?;
            return visual::verify_glass(&destination, &reference);
        }
        if command == "--preview-music-response" || command == "--preview-aurora-response" {
            let destination = arguments
                .next()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/music-response-preview"));
            return if command == "--preview-aurora-response" {
                visual::preview_aurora_response(&destination)
            } else {
                visual::preview_music_response(&destination)
            };
        }
        if command == "--preview-neon-coast" {
            let destination = arguments
                .next()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/neon-coast-preview"));
            return visual::preview_neon_coast(&destination);
        }
        if command == "--verify-specialization" {
            let destination = arguments
                .next()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/specialization-preview"));
            return visual::verify_specialization(&destination);
        }
        if command == "--preview-chromatic" {
            let destination = arguments
                .next()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/chromatic-preview"));
            return visual::preview_chromatic(&destination);
        }
        if command == "--preview-background" {
            let destination = arguments
                .next()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/background-preview"));
            return visual::preview_background(&destination);
        }
        if command == "--preview-attractor" {
            let destination = arguments
                .next()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/attractor-preview"));
            return visual::preview_attractor(&destination);
        }
        anyhow::bail!("Unknown option. {USAGE}");
    }

    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        // This is an animated GPU workload, not an idle low-latency form.
        // Keep vsync, but permit two queued frames to absorb small work spikes.
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration::default()
            .with_surface_config(eframe::egui_wgpu::SurfaceConfig::HIGH_THROUGHPUT),
        viewport: egui::ViewportBuilder::default()
            .with_title("amaraMusicVisualiser")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 640.0])
            .with_transparent(false),
        depth_buffer: 0,
        stencil_buffer: 0,
        multisampling: 1,
        ..Default::default()
    };

    eframe::run_native(
        "amaraMusicVisualiser",
        native_options,
        Box::new(|cc| Ok(Box::new(MusicVisualiserApp::new(cc)))),
    )
    .map_err(|error| anyhow::anyhow!("Could not start amaraMusicVisualiser: {error}"))
}
