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
    format::Format,
    image::{view::ImageView, Image, ImageCreateInfo, ImageType, ImageUsage, SampleCount},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::graphics::{subpass::PipelineRenderingCreateInfo, vertex_input::Vertex},
    swapchain::ColorSpace,
};
use vulkano_util::{
    context::{VulkanoConfig, VulkanoContext},
    renderer::VulkanoWindowRenderer,
    window::{VulkanoWindows, WindowDescriptor},
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
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
        let mut config = VulkanoConfig {
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
                fill_mode_non_solid: true,
                ..Features::empty()
            },
            ..Default::default()
        };
        config
            .instance_create_info
            .enabled_extensions
            .ext_swapchain_colorspace = true;

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
                resizable: false,
                ..Default::default()
            },
            |create_info| {
                create_info.image_format = Format::R16G16B16A16_SFLOAT;
                create_info.image_color_space = ColorSpace::ExtendedSrgbLinear;
            },
        );

        #[cfg(target_os = "macos")]
        unsafe {
            let window_handle = self
                .windows
                .get_window(window_id)
                .unwrap()
                .window_handle()
                .unwrap()
                .as_raw();
            enable_edr(window_handle);
        }

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
                depth_attachment_format: Some(Format::D32_SFLOAT),
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
                    model.vertices().iter().map(|v| {
                        let mut v = MyVertex::from(*v);
                        v.position[1] *= -1.0;
                        v
                    }),
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
                (elapsed * 0.5).sin() * 3.0,
                1.0,
                (elapsed * 0.5).cos() * 3.0,
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

        let extent = self
            .windows
            .get_renderer_mut(window_id)
            .unwrap()
            .swapchain_image_view()
            .image()
            .extent();

        let samples = SampleCount::Sample4;

        let depth_image = ImageView::new_default(
            Image::new(
                self.memory_allocator(),
                ImageCreateInfo {
                    image_type: ImageType::Dim2d,
                    extent: [extent[0], extent[1], 1],
                    format: Format::D32_SFLOAT,
                    usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT,
                    samples,
                    ..Default::default()
                },
                AllocationCreateInfo::default(),
            )
            .unwrap(),
        )
        .unwrap();

        let msaa_color_image = ImageView::new_default(
            Image::new(
                self.memory_allocator(),
                ImageCreateInfo {
                    image_type: ImageType::Dim2d,
                    extent: [extent[0], extent[1], 1],
                    format: Format::R16G16B16A16_SFLOAT,
                    usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSIENT_ATTACHMENT,
                    samples,
                    ..Default::default()
                },
                AllocationCreateInfo::default(),
            )
            .unwrap(),
        )
        .unwrap();

        let command_buffer_allocator = self.command_buffer_allocator.clone();
        let redraw = |renderer: &mut VulkanoWindowRenderer| {
            let before = renderer.acquire().unwrap();

            let after = draw(
                before,
                command_buffer_allocator.clone(),
                queue.clone(),
                msaa_color_image.clone(),
                renderer.swapchain_image_view(),
                depth_image.clone(),
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

#[cfg(target_os = "macos")]
unsafe fn enable_edr(window_handle: RawWindowHandle) {
    use objc2::{
        class,
        ffi::{BOOL, YES},
        msg_send,
        runtime::AnyObject,
    };
    if let RawWindowHandle::AppKit(window) = window_handle {
        let ns_view = window.ns_view.cast::<AnyObject>();
        let main_layer: *mut AnyObject = msg_send![ns_view, layer];
        let class = class!(CAMetalLayer);
        let is_valid_layer: BOOL = msg_send![main_layer, isKindOfClass: class];
        assert!(is_valid_layer, "Layer is not a CAMetalLayer");
        let () = msg_send![main_layer, setWantsExtendedDynamicRangeContent: YES];
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
