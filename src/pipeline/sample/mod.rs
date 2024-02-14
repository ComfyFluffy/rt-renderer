use std::{mem::size_of, sync::Arc};

use cgmath::SquareMatrix;
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::RecordingCommandBuffer,
    descriptor_set::{DescriptorSet, WriteDescriptorSet},
    device::Queue,
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    padded::Padded,
    pipeline::{
        graphics::{
            color_blend::{ColorBlendAttachmentState, ColorBlendState},
            depth_stencil::{CompareOp, DepthState, DepthStencilState},
            input_assembly::{InputAssemblyState, PrimitiveTopology},
            multisample::MultisampleState,
            rasterization::{CullMode, RasterizationState},
            subpass::PipelineRenderingCreateInfo,
            vertex_input::{Vertex, VertexDefinition},
            viewport::ViewportState,
            GraphicsPipelineCreateInfo,
        },
        layout::PipelineDescriptorSetLayoutCreateInfo,
        DynamicState, GraphicsPipeline, Pipeline, PipelineLayout, PipelineShaderStageCreateInfo,
    },
};

use crate::{App, MyVertex};

mod vs {
    vulkano_shaders::shader!(ty: "vertex", path: "src/pipeline/sample/sample.vert");
}

mod fs {
    vulkano_shaders::shader!(ty: "fragment", path: "src/pipeline/sample/sample.frag");
}

pub use fs::{Light, Material};

pub struct SamplePipeline {
    pipeline: Arc<GraphicsPipeline>,
    descriptor_sets: [Arc<DescriptorSet>; 2],
}

pub struct Camera {
    pub view: cgmath::Matrix4<f32>,
    pub proj: cgmath::Matrix4<f32>,
    pub position: cgmath::Point3<f32>,
}

fn create_uniform_buffer_from_data<T>(
    allocator: Arc<StandardMemoryAllocator>,
    data: T,
) -> Subbuffer<T>
where
    T: BufferContents,
{
    Buffer::from_data(
        allocator,
        BufferCreateInfo {
            usage: BufferUsage::UNIFORM_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ..Default::default()
        },
        data,
    )
    .unwrap()
}

impl SamplePipeline {
    pub fn new(
        app: &App,
        queue: Arc<Queue>,
        rendering_info: PipelineRenderingCreateInfo,
    ) -> SamplePipeline {
        assert!(size_of::<vs::PushConstants>() == size_of::<fs::PushConstants>());

        let pipeline = {
            let device = queue.device();
            let vs = vs::load(device.clone())
                .expect("failed to create shader module")
                .entry_point("main")
                .expect("shader entry point not found");
            let fs = fs::load(device.clone())
                .expect("failed to create shader module")
                .entry_point("main")
                .expect("shader entry point not found");
            let vertex_input_state = MyVertex::per_vertex()
                .definition(&vs.info().input_interface)
                .unwrap();
            let stages = [
                PipelineShaderStageCreateInfo::new(vs),
                PipelineShaderStageCreateInfo::new(fs),
            ];
            let layout = PipelineLayout::new(
                device.clone(),
                PipelineDescriptorSetLayoutCreateInfo::from_stages(&stages)
                    .into_pipeline_layout_create_info(device.clone())
                    .unwrap(),
            )
            .unwrap();

            GraphicsPipeline::new(
                device.clone(),
                None,
                GraphicsPipelineCreateInfo {
                    stages: stages.into_iter().collect(),
                    vertex_input_state: Some(vertex_input_state),
                    input_assembly_state: Some(InputAssemblyState {
                        topology: PrimitiveTopology::TriangleList,
                        ..Default::default()
                    }),
                    viewport_state: Some(ViewportState::default()),
                    rasterization_state: Some(RasterizationState {
                        cull_mode: CullMode::Back,
                        ..Default::default()
                    }),
                    multisample_state: Some(MultisampleState::default()),
                    color_blend_state: Some(ColorBlendState::with_attachment_states(
                        rendering_info.color_attachment_formats.len() as u32,
                        ColorBlendAttachmentState::default(),
                    )),
                    depth_stencil_state: Some(DepthStencilState {
                        depth: Some(DepthState {
                            compare_op: CompareOp::Less,
                            write_enable: true,
                        }),
                        ..Default::default()
                    }),
                    dynamic_state: [DynamicState::Viewport].into_iter().collect(),
                    subpass: Some(rendering_info.into()),
                    ..GraphicsPipelineCreateInfo::layout(layout)
                },
            )
            .unwrap()
        };

        let descriptor_sets = {
            // set = 0, binding = 0
            let model_uniform = create_uniform_buffer_from_data(
                app.memory_allocator(),
                vs::ModelBuffer {
                    model: cgmath::Matrix4::identity().into(),
                },
            );

            // set = 1, binding = 0
            let material_uniform = create_uniform_buffer_from_data(
                app.memory_allocator(),
                fs::Material {
                    ambient: Padded([0.1, 0.1, 0.1]),
                    diffuse: Padded([0.7, 0.7, 0.7]),
                    specular: [0.5, 0.5, 0.5],
                    shininess: 32.0,
                },
            );

            // set = 1, binding = 1
            let light_uniform = create_uniform_buffer_from_data(
                app.memory_allocator(),
                fs::Light {
                    position: Padded([3.0, 3.0, 3.0]),
                    ambient: Padded([1.0, 1.0, 1.0]),
                    diffuse: Padded([1.0, 1.0, 1.0]),
                    specular: [2.0, 2.0, 2.0],
                },
            );

            let set_layouts = pipeline.layout().set_layouts();
            let vertex_desc_layout = set_layouts[0].clone();
            let fragment_desc_layout = set_layouts[1].clone();

            let vertex_descriptor_set = DescriptorSet::new(
                app.descriptor_set_allocator.clone(),
                vertex_desc_layout,
                [WriteDescriptorSet::buffer(0, model_uniform)],
                [],
            )
            .unwrap();

            let fragment_descriptor_set = DescriptorSet::new(
                app.descriptor_set_allocator.clone(),
                fragment_desc_layout,
                [
                    WriteDescriptorSet::buffer(0, material_uniform),
                    WriteDescriptorSet::buffer(1, light_uniform),
                ],
                [],
            )
            .unwrap();

            [vertex_descriptor_set, fragment_descriptor_set]
        };

        Self {
            pipeline,
            descriptor_sets,
        }
    }

    pub fn render_object(
        &self,
        builder: &mut RecordingCommandBuffer,
        vertex_buffer: Subbuffer<[MyVertex]>,
        index_buffer: Option<Subbuffer<[u32]>>,
        camera: &Camera,
    ) {
        let vertex_count = vertex_buffer.len() as u32;

        builder
            .bind_pipeline_graphics(self.pipeline.clone())
            .unwrap()
            .bind_vertex_buffers(0, vertex_buffer)
            .unwrap()
            .bind_descriptor_sets(
                self.pipeline.bind_point(),
                self.pipeline.layout().clone(),
                0,
                self.descriptor_sets.iter().cloned().collect::<Vec<_>>(),
                // TODO: PR to improve DescriptorSetsCollection
            )
            .unwrap()
            .push_constants(
                self.pipeline.layout().clone(),
                0,
                vs::PushConstants {
                    view: camera.view.into(),
                    proj: camera.proj.into(),
                    camera_pos: camera.position.into(),
                },
            )
            .unwrap();
        unsafe {
            if let Some(index_buffer) = index_buffer {
                let index_count = index_buffer.len() as u32;
                builder
                    .bind_index_buffer(index_buffer)
                    .unwrap()
                    .draw_indexed(index_count, 1, 0, 0, 0)
                    .unwrap()
            } else {
                builder.draw(vertex_count, 1, 0, 0).unwrap()
            }
        };
    }
}
