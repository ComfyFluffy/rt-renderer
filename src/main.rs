fn main() {
    let mut app = rt_renderer::App::new();
    let scenes = easy_gltf::load("./cube.glb").unwrap();
    app.run(&scenes[0]);
}
