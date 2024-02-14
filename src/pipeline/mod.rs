use std::sync::Arc;

use vulkano::{
    command_buffer::{
        allocator::StandardCommandBufferAllocator, CommandBufferBeginInfo, CommandBufferLevel,
        CommandBufferUsage, RecordingCommandBuffer, RenderingAttachmentInfo, RenderingInfo,
    },
    device::Queue,
    format::ClearValue,
    image::view::ImageView,
    pipeline::graphics::viewport::Viewport,
    render_pass::{AttachmentLoadOp, AttachmentStoreOp},
    sync::GpuFuture,
};

pub mod sample;

pub fn draw(
    before: Box<dyn GpuFuture>,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    queue: Arc<Queue>,
    dst_image: Arc<ImageView>,
    depth_image: Arc<ImageView>,
    record_fn: impl FnOnce(&mut RecordingCommandBuffer),
) -> Box<dyn GpuFuture> {
    let mut builder = RecordingCommandBuffer::new(
        command_buffer_allocator.clone(),
        queue.queue_family_index(),
        CommandBufferLevel::Primary,
        CommandBufferBeginInfo {
            usage: CommandBufferUsage::OneTimeSubmit,
            ..Default::default()
        },
    )
    .unwrap();

    let viewport: Viewport = {
        let extent = dst_image.image().extent();
        Viewport {
            extent: [extent[0] as f32, extent[1] as f32],
            ..Default::default()
        }
    };

    builder
        .begin_rendering(RenderingInfo {
            color_attachments: vec![Some(RenderingAttachmentInfo {
                load_op: AttachmentLoadOp::Clear,
                store_op: AttachmentStoreOp::Store,
                clear_value: Some([0.0, 0.0, 0.0, 1.0].into()),
                ..RenderingAttachmentInfo::image_view(dst_image)
            })],
            depth_attachment: Some(RenderingAttachmentInfo {
                load_op: AttachmentLoadOp::Clear,
                store_op: AttachmentStoreOp::DontCare,
                clear_value: Some(ClearValue::Depth(1.0)),
                ..RenderingAttachmentInfo::image_view(depth_image)
            }),
            ..Default::default()
        })
        .unwrap()
        .set_viewport(0, [viewport].into_iter().collect())
        .unwrap();

    record_fn(&mut builder);

    builder.end_rendering().unwrap();

    let command_buffer = builder.end().unwrap();

    before.then_execute(queue, command_buffer).unwrap().boxed()
}
