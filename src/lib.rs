use std::{sync::Arc, time::Instant};

use easy_gltf::Scene;
use pipeline::{
    draw,
    sample::{Camera, SamplePipeline},
};
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::allocator::StandardCommandBufferAllocator,
    descriptor_set::allocator::StandardDescriptorSetAllocator,
    device::{DeviceExtensions, Features},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::graphics::{subpass::PipelineRenderingCreateInfo, vertex_input::Vertex},
};
use vulkano_util::{
    context::{VulkanoConfig, VulkanoContext},
    renderer::VulkanoWindowRenderer,
    window::{VulkanoWindows, WindowDescriptor},
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

mod gltf;
mod pipeline;

pub struct App {
    context: VulkanoContext,
    windows: VulkanoWindows,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
}

struct MyModel {
    vertex_buffer: Subbuffer<[MyVertex]>,
    index_buffer: Subbuffer<[u32]>,
}

#[derive(BufferContents, Vertex, Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct MyVertex {
    #[format(R32G32B32_SFLOAT)]
    pub position: [f32; 3],
    #[format(R32G32B32_SFLOAT)]
    pub normal: [f32; 3],
    #[format(R32G32_SFLOAT)]
    pub tex_coord: [f32; 2],
}

impl From<easy_gltf::model::Vertex> for MyVertex {
    fn from(vertex: easy_gltf::model::Vertex) -> Self {
        Self {
            position: vertex.position.into(),
            normal: vertex.normal.into(),
            tex_coord: vertex.tex_coords.into(),
        }
    }
}

impl App {
    pub fn new() -> Self {
        let config = VulkanoConfig {
            device_extensions: DeviceExtensions {
                khr_swapchain: true,
                khr_dynamic_rendering: true,
                // khr_acceleration_structure: true,
                // khr_ray_tracing_pipeline: true,
                // khr_deferred_host_operations: true,
                ..DeviceExtensions::empty()
            },
            device_features: Features {
                dynamic_rendering: true,
                ..Features::empty()
            },
            ..Default::default()
        };

        let context = VulkanoContext::new(config);
        let windows = VulkanoWindows::default();

        let device = context.device();

        let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(
            device.clone(),
            Default::default(),
        ));
        let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(
            device.clone(),
            Default::default(),
        ));

        Self {
            context,
            windows,
            command_buffer_allocator,
            descriptor_set_allocator,
        }
    }

    pub fn run(&mut self, scene: &Scene) {
        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(ControlFlow::Poll);

        let window_id = self.windows.create_window(
            &event_loop,
            &self.context,
            &WindowDescriptor {
                width: 1280.0,
                height: 720.0,
                title: "r/place 2023 Player".to_string(),
                ..Default::default()
            },
            |_| {},
        );

        let queue = self.context.graphics_queue().clone();

        let sample_pipeline = SamplePipeline::new(
            &self,
            queue.clone(),
            PipelineRenderingCreateInfo {
                color_attachment_formats: vec![Some(
                    self.windows
                        .get_renderer(window_id)
                        .unwrap()
                        .swapchain_format(),
                )],
                ..Default::default()
            },
        );

        let memory_allocator = self.memory_allocator();

        let models = scene
            .models
            .iter()
            .map(|model| {
                let vertex_buffer = Buffer::from_iter(
                    memory_allocator.clone(),
                    BufferCreateInfo {
                        usage: BufferUsage::VERTEX_BUFFER,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                            | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    model.vertices().iter().map(|v| MyVertex::from(*v)),
                )
                .unwrap();
                let index_buffer = Buffer::from_iter(
                    memory_allocator.clone(),
                    BufferCreateInfo {
                        usage: BufferUsage::INDEX_BUFFER,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                            | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    model.indices().unwrap().iter().cloned(),
                )
                .unwrap();

                MyModel {
                    vertex_buffer,
                    index_buffer,
                }
            })
            .collect::<Vec<_>>();

        let render_start = Instant::now();
        let camera_fn = || {
            let elapsed = render_start.elapsed().as_secs_f32();
            let position = cgmath::Point3::new(
                (elapsed * 0.5).sin() * 5.0,
                1.0,
                (elapsed * 0.5).cos() * 5.0,
            );
            Camera {
                position,
                view: cgmath::Matrix4::look_at_rh(
                    position,
                    cgmath::Point3::new(0.0, 0.0, 0.0),
                    cgmath::Vector3::unit_y(),
                ),
                proj: cgmath::perspective(cgmath::Deg(60.0), 1280.0 / 720.0, 0.1, 100.0),
            }
        };

        let command_buffer_allocator = self.command_buffer_allocator.clone();
        let redraw = |renderer: &mut VulkanoWindowRenderer| {
            let before = renderer.acquire().unwrap();

            let after = draw(
                before,
                command_buffer_allocator.clone(),
                queue.clone(),
                renderer.swapchain_image_view(),
                |builder| {
                    for model in &models {
                        let vertex_buffer = model.vertex_buffer.clone();
                        let index_buffer = model.index_buffer.clone();

                        sample_pipeline.render_object(
                            builder,
                            vertex_buffer,
                            Some(index_buffer),
                            &camera_fn(),
                        )
                    }
                },
            );
            renderer.present(after, true);
        };

        event_loop
            .run(move |event, elwt| {
                let renderer = self.windows.get_renderer_mut(window_id).unwrap();
                match event {
                    Event::WindowEvent { event, .. } => match event {
                        WindowEvent::CloseRequested => elwt.exit(),
                        WindowEvent::Resized(..) => {
                            renderer.resize();
                        }
                        WindowEvent::ScaleFactorChanged { .. } => {
                            renderer.resize();
                        }
                        WindowEvent::RedrawRequested => {
                            redraw(renderer);
                        }
                        _ => {}
                    },
                    Event::AboutToWait => {
                        self.windows.get_window(window_id).unwrap().request_redraw();
                    }
                    _ => {}
                }
            })
            .unwrap();
    }

    pub(crate) fn memory_allocator(&self) -> Arc<StandardMemoryAllocator> {
        self.context.memory_allocator().clone()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_new() {
        println!("{}", std::env::var("DYLD_FALLBACK_LIBRARY_PATH").unwrap());
        super::App::new();
    }
}
