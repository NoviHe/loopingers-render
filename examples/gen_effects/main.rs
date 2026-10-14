//! Renders the overlay effect videos in `assets/effects/`.
//!
//! Every effect is a seamless 8 second loop on a black background, meant to be blended with the
//! `screen` mode. Motion uses whole numbers of cycles per loop so the last frame leads straight
//! back into the first.
//!
//! ```text
//! cargo run --release --example gen_effects            # all effects into assets/effects
//! cargo run --release --example gen_effects rain snow  # only these
//! ```

mod canvas;
mod classic;
mod more;

use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
};

use canvas::{Canvas, Draw, FPS, FRAMES, HEIGHT, WIDTH};

fn effects() -> Vec<(&'static str, Draw)> {
    vec![
        ("rain", classic::rain(101)),
        ("snow", classic::snow(202)),
        ("fire", classic::embers(303)),
        ("bokeh", classic::bokeh(404)),
        ("light_leak", classic::light_leak(505)),
        ("sparkles", classic::sparkles(606)),
        ("fireflies", classic::fireflies(707)),
        ("dust", classic::dust(808)),
        ("stars", classic::stars(909)),
        ("confetti", classic::confetti(1010)),
        ("hearts", classic::hearts(1111)),
        ("petals", classic::petals(1212)),
        ("mist", classic::mist(1313)),
        ("aurora", classic::aurora(1414)),
        ("rays", classic::rays(1515)),
        ("lens_flare", classic::lens_flare(1616)),
        ("drizzle", classic::rain_scaled(1717, 0.45)),
        (
            "rose_petals",
            classic::petals_colored(1818, [1.0, 0.55, 0.6], [0.85, 0.08, 0.18], 64),
        ),
        ("shooting_stars", more::shooting_stars(1919)),
        ("fireworks", more::fireworks(2020)),
        ("lightning", more::lightning(2121)),
        ("bubbles", more::bubbles(2222)),
        ("leaves", more::leaves(2323)),
        ("smoke", more::smoke(2424)),
        ("music_notes", more::music_notes(2525)),
        ("ripples", more::ripples(2626)),
        ("film_dust", more::film_dust(2727)),
        ("matrix", more::matrix(2828)),
        ("snowflakes", more::snowflakes(2929)),
        ("gold_rain", more::gold_rain(3030)),
        ("stage_lights", more::stage_lights(3131)),
        ("equalizer", more::equalizer(3232)),
        ("rainbow", more::rainbow(3333)),
        ("balloons", more::balloons(3434)),
        ("nebula", more::nebula(3535)),
        ("synthwave", more::synthwave(3636)),
        ("light_streaks", more::light_streaks(3737)),
    ]
}

fn render(name: &str, draw: &Draw, directory: &Path) -> std::io::Result<()> {
    let output = directory.join(format!("{name}.mp4"));
    let mut encoder = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
        ])
        .args([
            "-s",
            &format!("{WIDTH}x{HEIGHT}"),
            "-r",
            &FPS.to_string(),
            "-i",
            "-",
        ])
        .args([
            "-c:v", "libx264", "-preset", "slow", "-crf", "17", "-pix_fmt", "yuv420p",
        ])
        .args(["-movflags", "+faststart"])
        .arg(&output)
        .stdin(Stdio::piped())
        .spawn()?;
    let mut input = encoder.stdin.take().expect("piped stdin");
    let mut canvas = Canvas::new();
    for frame in 0..FRAMES {
        canvas.clear();
        draw(&mut canvas, frame as f32 / FRAMES as f32);
        input.write_all(&canvas.to_rgb8())?;
    }
    drop(input);
    let status = encoder.wait()?;
    if !status.success() {
        return Err(std::io::Error::other(format!("ffmpeg failed for {name}")));
    }
    println!("{name}: {}", output.display());
    Ok(())
}

fn main() {
    let wanted: Vec<String> = std::env::args().skip(1).collect();
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("effects");
    std::fs::create_dir_all(&directory).expect("effects folder");

    let jobs: Vec<_> = effects()
        .into_iter()
        .filter(|(name, _)| wanted.is_empty() || wanted.iter().any(|w| w == name))
        .collect();
    let next = AtomicUsize::new(0);
    let workers = thread::available_parallelism().map_or(2, |n| n.get().min(8));
    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::SeqCst);
                let Some((name, draw)) = jobs.get(index) else {
                    break;
                };
                if let Err(error) = render(name, draw, &directory) {
                    eprintln!("{name}: {error}");
                }
            });
        }
    });
}
