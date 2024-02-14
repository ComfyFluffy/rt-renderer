fn main() {
    let mut app = rt_renderer::App::new();
    let scenes = easy_gltf::load("./su.glb").unwrap();
    app.run(&scenes[0]);
}
