use std::path::Path;

fn main() {
    let color_space_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("acs");
    std::fs::create_dir_all(&color_space_dir).unwrap();

    appearance_color_spaces::write_srgb_tables(color_space_dir.join("srgb"))
        .expect("Failed to write srgb color space.");
}
