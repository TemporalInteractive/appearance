fn main() {
    let child = std::thread::Builder::new()
        .stack_size(1024 * 1024 * 512)
        .spawn(render_node::internal_main)
        .unwrap();
    let _ = child.join().unwrap();
}
