#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "checked: target_size ≤ MAX_PIXELS (4 Mi pixels) and source dimensions are finite positive floats from resvg"
)]

use std::io::{self, Read, Write};

use resvg::tiny_skia::Pixmap;
use resvg::usvg::Tree;

const MAX_SVG_BYTES: u32 = 1_024_000;
const MAX_PIXELS: u32 = 2048 * 2048;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    sandbox_child()?;

    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    // First u32 is the target pixel dimension for scaling
    let mut target_bytes = [0u8; 4];
    stdin.read_exact(&mut target_bytes)?;
    let target_size = u32::from_le_bytes(target_bytes);

    // Read the rest of stdin as the SVG document bytes
    let mut svg_data = Vec::new();
    stdin.read_to_end(&mut svg_data)?;

    if svg_data.is_empty()
        || svg_data.len() > MAX_SVG_BYTES as usize
        || target_size == 0
        || target_size > MAX_SVG_BYTES
    {
        eprintln!("invalid input");
        std::process::exit(1);
    }

    let secondary_image = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let data_image = std::sync::Arc::clone(&secondary_image);
    let path_image = std::sync::Arc::clone(&secondary_image);
    let options = resvg::usvg::Options {
        image_href_resolver: resvg::usvg::ImageHrefResolver {
            resolve_data: Box::new(move |_mime, _data, _options| {
                data_image.store(true, std::sync::atomic::Ordering::Relaxed);
                None
            }),
            resolve_string: Box::new(move |_href, _options| {
                path_image.store(true, std::sync::atomic::Ordering::Relaxed);
                None
            }),
        },
        ..resvg::usvg::Options::default()
    };
    let tree = Tree::from_data(&svg_data, &options)?;

    // Reject SVGs that attempt to load secondary images
    if secondary_image.load(std::sync::atomic::Ordering::Relaxed) {
        eprintln!("SVG icons must not contain secondary images");
        std::process::exit(1);
    }

    let source_width = tree.size().width();
    let source_height = tree.size().height();

    if !source_width.is_finite()
        || !source_height.is_finite()
        || source_width <= 0.0
        || source_height <= 0.0
    {
        eprintln!("invalid SVG dimensions");
        std::process::exit(1);
    }

    let scale = (target_size as f32 / source_width).min(target_size as f32 / source_height);
    if !scale.is_finite() || scale <= 0.0 {
        eprintln!("invalid scale factor");
        std::process::exit(1);
    }

    let scaled_width = (source_width * scale).round().max(1.0) as u32;
    let scaled_height = (source_height * scale).round().max(1.0) as u32;

    if scaled_width == 0
        || scaled_height == 0
        || scaled_width > MAX_PIXELS
        || scaled_height > MAX_PIXELS
        || u64::from(scaled_width) * u64::from(scaled_height) > u64::from(MAX_PIXELS)
    {
        eprintln!("scaled dimensions exceed limits");
        std::process::exit(1);
    }

    let mut pixmap = Pixmap::new(scaled_width, scaled_height).ok_or("failed to allocate pixmap")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let rgba = pixmap.take();

    stdout.write_all(&scaled_width.to_le_bytes())?;
    stdout.write_all(&scaled_height.to_le_bytes())?;
    stdout.write_all(&rgba)?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn sandbox_child() -> Result<(), Box<dyn std::error::Error>> {
    use rustix::process::{setrlimit, Resource, Rlimit};

    // Clear all environment variables using safe API
    for (key, _) in std::env::vars() {
        std::env::remove_var(key);
    }
    std::env::set_var("PATH", "/usr/bin:/bin");

    // 1 second CPU limit prevents CPU-bound SVG bombs
    setrlimit(
        Resource::Cpu,
        Rlimit {
            current: Some(1),
            maximum: Some(1),
        },
    )?;

    // 64 MiB address space limit prevents memory exhaustion
    setrlimit(
        Resource::As,
        Rlimit {
            current: Some(64 * 1024 * 1024),
            maximum: Some(64 * 1024 * 1024),
        },
    )?;

    // 32 MiB file write limit prevents disk-write bombs
    setrlimit(
        Resource::Fsize,
        Rlimit {
            current: Some(32 * 1024 * 1024),
            maximum: Some(32 * 1024 * 1024),
        },
    )?;

    // Isolate to root directory so file-open attempts fail predictably
    std::env::set_current_dir("/")?;

    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn sandbox_child() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}