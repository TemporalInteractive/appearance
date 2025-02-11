use std::path::Path;

fn main() {
    let color_space_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("acs");
    std::fs::create_dir_all(&color_space_dir).unwrap();

    appearance_color_spaces::write_aces_tables(color_space_dir.join("aces"))
        .expect("Failed to write aces color space.");
    appearance_color_spaces::write_dci_p3_tables(color_space_dir.join("dci_p3"))
        .expect("Failed to write aces color space.");
    appearance_color_spaces::write_rec2020_tables(color_space_dir.join("rec2020"))
        .expect("Failed to write aces color space.");
    appearance_color_spaces::write_srgb_tables(color_space_dir.join("srgb"))
        .expect("Failed to write srgb color space.");
}
