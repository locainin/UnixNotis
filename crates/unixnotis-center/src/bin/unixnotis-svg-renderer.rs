use std::io::{self, Read, Write};

use resvg::tiny_skia::Pixmap;
use resvg::usvg::Tree;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let lines: Vec<&str> = input.lines().collect();
    if lines.len() < 3 {
        eprintln!("expected 3 lines: svg, width, scale");
        std::process::exit(1);
    }

    let svg_data = lines[0];
    let _width: u32 = lines[1].parse()?;
    let scale: f32 = lines[2].parse()?;

    let options = resvg::usvg::Options::default();
    let tree = Tree::from_str(svg_data, &options)?;

    let source_width = tree.size().width();
    let source_height = tree.size().height();
    let scaled_width = (source_width * scale).round() as u32;
    let scaled_height = (source_height * scale).round() as u32;

    let mut pixmap = Pixmap::new(scaled_width, scaled_height).ok_or("failed to allocate pixmap")?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());

    let rgba = pixmap.take();
    io::stdout().write_all(&rgba)?;

    Ok(())
}