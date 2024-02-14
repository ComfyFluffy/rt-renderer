#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {
        let scenes = easy_gltf::load("/Users/i/Developer/rt-renderer/cube.glb").unwrap();
        for s in scenes {
            for m in s.models {
                let vertices = m.vertices();
                let _indicies = m.indices().unwrap();
                for (i, v) in vertices.iter().enumerate() {
                    println!("Vertex {}: {:?}", i, v.position);
                }
            }
            for c in s.cameras {
                println!("Camera: {:?}", c);
            }
        }
    }
}
