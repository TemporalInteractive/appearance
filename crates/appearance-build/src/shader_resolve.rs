use std::path::Path;

use appearance_asset_database::asset_paths::resolve_asset_path;

fn parse_shader_includes_recursive(name: &str, includes: &mut Vec<String>) -> String {
    let file_path = Path::new(&resolve_asset_path(name, "shaders/"))
        .strip_prefix("assets/")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();

    if includes.contains(&file_path) {
        return String::new();
    }
    includes.push(file_path.clone());

    let mut contents = std::fs::read_to_string(format!("{}.wgsl", file_path))
        .unwrap_or_else(|_| panic!("Invalid shader name: {}.", file_path));

    let mut include_indices: Vec<usize> = contents.match_indices("@include").map(|i| i.0).collect();
    include_indices.reverse();
    for include_index in include_indices {
        let end_of_line = contents[include_index..].find('\n').unwrap() + include_index - 1;
        let include_name = contents[(include_index + 9)..end_of_line].to_owned();

        for i in (include_index..end_of_line).rev() {
            contents.remove(i);
        }
        contents.insert_str(
            include_index,
            &parse_shader_includes_recursive(&include_name, includes),
        );
    }

    contents
}

pub fn parse_shader_includes(mut contents: String) -> String {
    let mut includes = vec![];

    let mut include_indices: Vec<usize> = contents.match_indices("@include").map(|i| i.0).collect();
    include_indices.reverse();
    for include_index in include_indices {
        let end_of_line = contents[include_index..].find('\n').unwrap() + include_index - 1;
        let include_name = contents[(include_index + 9)..end_of_line].to_owned();

        for i in (include_index..end_of_line).rev() {
            contents.remove(i);
        }
        contents.insert_str(
            include_index,
            &parse_shader_includes_recursive(&include_name, &mut includes),
        );
    }

    contents.replace("::", "_")
}
