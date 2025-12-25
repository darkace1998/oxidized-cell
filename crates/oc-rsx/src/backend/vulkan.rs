//! Vulkan backend for RSX
//!
//! This module contains the Vulkan implementation for RSX rendering.

use super::{GraphicsBackend, PrimitiveType};
use crate::vertex::{VertexAttribute, VertexAttributeType};
use ash::vk;
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator, AllocatorCreateDesc};
use gpu_allocator::MemoryLocation;
use std::ffi::CString;
use std::sync::{Arc, Mutex};

/// Vulkan graphics backend
pub struct VulkanBackend {
    /// Entry point for Vulkan API
    entry: Option<ash::Entry>,
    /// Vulkan instance
    instance: Option<ash::Instance>,
    /// Physical device
    physical_device: Option<vk::PhysicalDevice>,
    /// Logical device
    device: Option<ash::Device>,
    /// Graphics queue
    graphics_queue: Option<vk::Queue>,
    /// Graphics queue family index
    graphics_queue_family: u32,
    /// Command pool for graphics commands
    command_pool: Option<vk::CommandPool>,
    /// Command buffers for each frame in flight
    command_buffers: Vec<vk::CommandBuffer>,
    /// Current command buffer
    current_cmd_buffer: Option<vk::CommandBuffer>,
    /// Current render pass
    render_pass: Option<vk::RenderPass>,
    /// Current framebuffer
    framebuffer: Option<vk::Framebuffer>,
    /// Render target images
    render_images: Vec<vk::Image>,
    /// Render target image views
    render_image_views: Vec<vk::ImageView>,
    /// Render target image memory allocations
    render_image_allocations: Vec<Allocation>,
    /// Depth image
    depth_image: Option<vk::Image>,
    /// Depth image view
    depth_image_view: Option<vk::ImageView>,
    /// Depth image memory allocation
    depth_image_allocation: Option<Allocation>,
    /// GPU memory allocator
    allocator: Option<Arc<Mutex<Allocator>>>,
    /// Synchronization: Image available semaphores
    image_available_semaphores: Vec<vk::Semaphore>,
    /// Synchronization: Render finished semaphores
    render_finished_semaphores: Vec<vk::Semaphore>,
    /// Synchronization: In-flight fences
    in_flight_fences: Vec<vk::Fence>,
    /// Current frame index
    current_frame: usize,
    /// Maximum frames in flight
    max_frames_in_flight: usize,
    /// Whether backend is initialized
    initialized: bool,
    /// Render width
    width: u32,
    /// Render height
    height: u32,
    /// Current pipeline layout
    pipeline_layout: Option<vk::PipelineLayout>,
    /// Current graphics pipeline
    pipeline: Option<vk::Pipeline>,
    /// Descriptor set layout
    descriptor_set_layout: Option<vk::DescriptorSetLayout>,
    /// Vertex input bindings
    vertex_bindings: Vec<vk::VertexInputBindingDescription>,
    /// Vertex input attributes
    vertex_attributes: Vec<vk::VertexInputAttributeDescription>,
    /// Whether we're in a render pass
    in_render_pass: bool,
}

impl VulkanBackend {
    /// Create a new Vulkan backend
    pub fn new() -> Self {
        Self::with_frames_in_flight(2)
    }

    /// Create a new Vulkan backend with specified frames in flight
    pub fn with_frames_in_flight(max_frames: usize) -> Self {
        Self {
            entry: None,
            instance: None,
            physical_device: None,
            device: None,
            graphics_queue: None,
            graphics_queue_family: 0,
            command_pool: None,
            command_buffers: Vec::new(),
            current_cmd_buffer: None,
            render_pass: None,
            framebuffer: None,
            render_images: Vec::new(),
            render_image_views: Vec::new(),
            render_image_allocations: Vec::new(),
            depth_image: None,
            depth_image_view: None,
            depth_image_allocation: None,
            allocator: None,
            image_available_semaphores: Vec::new(),
            render_finished_semaphores: Vec::new(),
            in_flight_fences: Vec::new(),
            current_frame: 0,
            max_frames_in_flight: max_frames,
            initialized: false,
            width: 1280,
            height: 720,
            pipeline_layout: None,
            pipeline: None,
            descriptor_set_layout: None,
            vertex_bindings: Vec::new(),
            vertex_attributes: Vec::new(),
            in_render_pass: false,
        }
    }

    /// Create Vulkan instance
    fn create_instance(entry: &ash::Entry) -> Result<ash::Instance, String> {
        let app_name = CString::new("oxidized-cell RSX").unwrap();
        let engine_name = CString::new("oxidized-cell").unwrap();

        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0, 1, 0, 0))
            .engine_name(&engine_name)
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(vk::API_VERSION_1_2);

        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info);

        unsafe {
            entry
                .create_instance(&create_info, None)
                .map_err(|e| format!("Failed to create Vulkan instance: {:?}", e))
        }
    }

    /// Select physical device
    fn select_physical_device(
        instance: &ash::Instance,
    ) -> Result<(vk::PhysicalDevice, u32), String> {
        unsafe {
            let devices = instance
                .enumerate_physical_devices()
                .map_err(|e| format!("Failed to enumerate physical devices: {:?}", e))?;

            if devices.is_empty() {
                return Err("No Vulkan-capable devices found".to_string());
            }

            // Use first device and find graphics queue family
            let physical_device = devices[0];
            let queue_families = instance.get_physical_device_queue_family_properties(physical_device);

            let graphics_queue_family = queue_families
                .iter()
                .enumerate()
                .find(|(_, props)| props.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                .map(|(i, _)| i as u32)
                .ok_or("No graphics queue family found")?;

            Ok((physical_device, graphics_queue_family))
        }
    }

    /// Create logical device
    fn create_device(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        queue_family: u32,
    ) -> Result<(ash::Device, vk::Queue), String> {
        let queue_priorities = [1.0f32];
        let queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family)
            .queue_priorities(&queue_priorities);

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_create_info));

        unsafe {
            let device = instance
                .create_device(physical_device, &device_create_info, None)
                .map_err(|e| format!("Failed to create logical device: {:?}", e))?;

            let queue = device.get_device_queue(queue_family, 0);

            Ok((device, queue))
        }
    }

    /// Create command pool
    fn create_command_pool(device: &ash::Device, queue_family: u32) -> Result<vk::CommandPool, String> {
        let pool_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(queue_family);

        unsafe {
            device
                .create_command_pool(&pool_info, None)
                .map_err(|e| format!("Failed to create command pool: {:?}", e))
        }
    }

    /// Allocate command buffer
    fn allocate_command_buffer(
        device: &ash::Device,
        command_pool: vk::CommandPool,
    ) -> Result<vk::CommandBuffer, String> {
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        unsafe {
            device
                .allocate_command_buffers(&alloc_info)
                .map(|buffers| buffers[0])
                .map_err(|e| format!("Failed to allocate command buffer: {:?}", e))
        }
    }

    /// Create a basic render pass for color and depth
    fn create_render_pass(device: &ash::Device) -> Result<vk::RenderPass, String> {
        let color_attachment = vk::AttachmentDescription::default()
            .format(vk::Format::B8G8R8A8_UNORM)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

        let depth_attachment = vk::AttachmentDescription::default()
            .format(vk::Format::D24_UNORM_S8_UINT)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::CLEAR)
            .stencil_store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        let color_attachment_ref = vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let depth_attachment_ref = vk::AttachmentReference::default()
            .attachment(1)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);

        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&color_attachment_ref))
            .depth_stencil_attachment(&depth_attachment_ref);

        let attachments = [color_attachment, depth_attachment];
        let render_pass_info = vk::RenderPassCreateInfo::default()
            .attachments(&attachments)
            .subpasses(std::slice::from_ref(&subpass));

        unsafe {
            device
                .create_render_pass(&render_pass_info, None)
                .map_err(|e| format!("Failed to create render pass: {:?}", e))
        }
    }

    /// Create synchronization primitives for frame synchronization
    fn create_sync_objects(
        device: &ash::Device,
        count: usize,
    ) -> Result<(Vec<vk::Semaphore>, Vec<vk::Semaphore>, Vec<vk::Fence>), String> {
        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

        unsafe {
            let mut image_available = Vec::with_capacity(count);
            let mut render_finished = Vec::with_capacity(count);
            let mut fences = Vec::with_capacity(count);

            for _ in 0..count {
                image_available.push(
                    device
                        .create_semaphore(&semaphore_info, None)
                        .map_err(|e| format!("Failed to create semaphore: {:?}", e))?,
                );
                render_finished.push(
                    device
                        .create_semaphore(&semaphore_info, None)
                        .map_err(|e| format!("Failed to create semaphore: {:?}", e))?,
                );
                fences.push(
                    device
                        .create_fence(&fence_info, None)
                        .map_err(|e| format!("Failed to create fence: {:?}", e))?,
                );
            }

            Ok((image_available, render_finished, fences))
        }
    }

    /// Create render target images and views
    fn create_render_targets(
        device: &ash::Device,
        allocator: &Arc<Mutex<Allocator>>,
        width: u32,
        height: u32,
        count: usize,
    ) -> Result<(Vec<vk::Image>, Vec<vk::ImageView>, Vec<Allocation>), String> {
        let mut images = Vec::with_capacity(count);
        let mut views = Vec::with_capacity(count);
        let mut allocations = Vec::with_capacity(count);

        for i in 0..count {
            // Create image for render target
            let image_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::B8G8R8A8_UNORM)
                .extent(vk::Extent3D {
                    width,
                    height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC)
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED);

            let image = unsafe {
                device
                    .create_image(&image_info, None)
                    .map_err(|e| format!("Failed to create render image {}: {:?}", i, e))?
            };

            // Get memory requirements and allocate
            let requirements = unsafe { device.get_image_memory_requirements(image) };

            let allocation = allocator
                .lock()
                .unwrap()
                .allocate(&AllocationCreateDesc {
                    name: &format!("render_target_{}", i),
                    requirements,
                    location: MemoryLocation::GpuOnly,
                    linear: false,
                    allocation_scheme: AllocationScheme::GpuAllocatorManaged,
                })
                .map_err(|e| format!("Failed to allocate memory for render image {}: {:?}", i, e))?;

            unsafe {
                device
                    .bind_image_memory(image, allocation.memory(), allocation.offset())
                    .map_err(|e| format!("Failed to bind render image memory {}: {:?}", i, e))?;
            }

            // Create image view
            let view_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(vk::Format::B8G8R8A8_UNORM)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });

            let view = unsafe {
                device
                    .create_image_view(&view_info, None)
                    .map_err(|e| format!("Failed to create render image view {}: {:?}", i, e))?
            };

            images.push(image);
            views.push(view);
            allocations.push(allocation);
        }

        Ok((images, views, allocations))
    }

    /// Create depth buffer
    fn create_depth_buffer(
        device: &ash::Device,
        allocator: &Arc<Mutex<Allocator>>,
        width: u32,
        height: u32,
    ) -> Result<(vk::Image, vk::ImageView, Allocation), String> {
        // Create depth image
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::D24_UNORM_S8_UINT)
            .extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let image = unsafe {
            device
                .create_image(&image_info, None)
                .map_err(|e| format!("Failed to create depth image: {:?}", e))?
        };

        // Get memory requirements and allocate
        let requirements = unsafe { device.get_image_memory_requirements(image) };

        let allocation = allocator
            .lock()
            .unwrap()
            .allocate(&AllocationCreateDesc {
                name: "depth_buffer",
                requirements,
                location: MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })
            .map_err(|e| format!("Failed to allocate memory for depth buffer: {:?}", e))?;

        unsafe {
            device
                .bind_image_memory(image, allocation.memory(), allocation.offset())
                .map_err(|e| format!("Failed to bind depth image memory: {:?}", e))?;
        }

        // Create image view
        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(vk::Format::D24_UNORM_S8_UINT)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });

        let view = unsafe {
            device
                .create_image_view(&view_info, None)
                .map_err(|e| format!("Failed to create depth image view: {:?}", e))?
        };

        Ok((image, view, allocation))
    }

    /// Convert RSX vertex attribute type to Vulkan format
    fn vertex_type_to_vk_format(type_: VertexAttributeType, size: u8, normalized: bool) -> vk::Format {
        match (type_, size, normalized) {
            (VertexAttributeType::FLOAT, 1, _) => vk::Format::R32_SFLOAT,
            (VertexAttributeType::FLOAT, 2, _) => vk::Format::R32G32_SFLOAT,
            (VertexAttributeType::FLOAT, 3, _) => vk::Format::R32G32B32_SFLOAT,
            (VertexAttributeType::FLOAT, 4, _) => vk::Format::R32G32B32A32_SFLOAT,
            (VertexAttributeType::SHORT, 1, true) => vk::Format::R16_SNORM,
            (VertexAttributeType::SHORT, 1, false) => vk::Format::R16_SINT,
            (VertexAttributeType::SHORT, 2, true) => vk::Format::R16G16_SNORM,
            (VertexAttributeType::SHORT, 2, false) => vk::Format::R16G16_SINT,
            (VertexAttributeType::SHORT, 3, true) => vk::Format::R16G16B16_SNORM,
            (VertexAttributeType::SHORT, 3, false) => vk::Format::R16G16B16_SINT,
            (VertexAttributeType::SHORT, 4, true) => vk::Format::R16G16B16A16_SNORM,
            (VertexAttributeType::SHORT, 4, false) => vk::Format::R16G16B16A16_SINT,
            (VertexAttributeType::BYTE, 1, true) => vk::Format::R8_SNORM,
            (VertexAttributeType::BYTE, 1, false) => vk::Format::R8_SINT,
            (VertexAttributeType::BYTE, 2, true) => vk::Format::R8G8_SNORM,
            (VertexAttributeType::BYTE, 2, false) => vk::Format::R8G8_SINT,
            (VertexAttributeType::BYTE, 3, true) => vk::Format::R8G8B8_SNORM,
            (VertexAttributeType::BYTE, 3, false) => vk::Format::R8G8B8_SINT,
            (VertexAttributeType::BYTE, 4, true) => vk::Format::R8G8B8A8_SNORM,
            (VertexAttributeType::BYTE, 4, false) => vk::Format::R8G8B8A8_SINT,
            (VertexAttributeType::HALF_FLOAT, 1, _) => vk::Format::R16_SFLOAT,
            (VertexAttributeType::HALF_FLOAT, 2, _) => vk::Format::R16G16_SFLOAT,
            (VertexAttributeType::HALF_FLOAT, 3, _) => vk::Format::R16G16B16_SFLOAT,
            (VertexAttributeType::HALF_FLOAT, 4, _) => vk::Format::R16G16B16A16_SFLOAT,
            // COMPRESSED: Compressed vertex data (CMP) - typically 10/10/10/2 format
            (VertexAttributeType::COMPRESSED, _, true) => vk::Format::A2B10G10R10_SNORM_PACK32,
            (VertexAttributeType::COMPRESSED, _, false) => vk::Format::A2B10G10R10_UINT_PACK32,
            _ => vk::Format::R32G32B32A32_SFLOAT, // Default fallback
        }
    }

    /// Convert RSX primitive type to Vulkan topology
    fn primitive_to_vk_topology(primitive: PrimitiveType) -> vk::PrimitiveTopology {
        match primitive {
            PrimitiveType::Points => vk::PrimitiveTopology::POINT_LIST,
            PrimitiveType::Lines => vk::PrimitiveTopology::LINE_LIST,
            PrimitiveType::LineLoop => vk::PrimitiveTopology::LINE_STRIP, // Approximate
            PrimitiveType::LineStrip => vk::PrimitiveTopology::LINE_STRIP,
            PrimitiveType::Triangles => vk::PrimitiveTopology::TRIANGLE_LIST,
            PrimitiveType::TriangleStrip => vk::PrimitiveTopology::TRIANGLE_STRIP,
            PrimitiveType::TriangleFan => vk::PrimitiveTopology::TRIANGLE_FAN,
            PrimitiveType::Quads => vk::PrimitiveTopology::TRIANGLE_LIST, // Will need index conversion
            PrimitiveType::QuadStrip => vk::PrimitiveTopology::TRIANGLE_STRIP,
            PrimitiveType::Polygon => vk::PrimitiveTopology::TRIANGLE_FAN, // Approximate
        }
    }
}

impl Default for VulkanBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphicsBackend for VulkanBackend {
    fn init(&mut self) -> Result<(), String> {
        if self.initialized {
            return Ok(());
        }

        tracing::info!("Initializing Vulkan backend");

        // Load Vulkan
        let entry = unsafe {
            ash::Entry::load()
                .map_err(|e| format!("Failed to load Vulkan library: {:?}", e))?
        };

        // Create instance
        let instance = Self::create_instance(&entry)?;

        // Select physical device
        let (physical_device, graphics_queue_family) = Self::select_physical_device(&instance)?;

        // Create logical device
        let (device, graphics_queue) =
            Self::create_device(&instance, physical_device, graphics_queue_family)?;

        // Create GPU memory allocator
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.clone(),
            physical_device,
            debug_settings: Default::default(),
            buffer_device_address: false,
            allocation_sizes: Default::default(),
        })
        .map_err(|e| format!("Failed to create GPU allocator: {:?}", e))?;
        let allocator = Arc::new(Mutex::new(allocator));

        // Create command pool
        let command_pool = Self::create_command_pool(&device, graphics_queue_family)?;

        // Allocate command buffers for each frame in flight
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(self.max_frames_in_flight as u32);

        let command_buffers = unsafe {
            device
                .allocate_command_buffers(&alloc_info)
                .map_err(|e| format!("Failed to allocate command buffers: {:?}", e))?
        };

        // Create render pass
        let render_pass = Self::create_render_pass(&device)?;

        // Create synchronization objects
        let (image_available, render_finished, fences) =
            Self::create_sync_objects(&device, self.max_frames_in_flight)?;

        // Create render targets with actual images and views
        let (render_images, render_image_views, render_image_allocations) =
            Self::create_render_targets(&device, &allocator, self.width, self.height, self.max_frames_in_flight)?;

        // Create depth buffer with actual image and view
        let (depth_image, depth_image_view, depth_allocation) =
            Self::create_depth_buffer(&device, &allocator, self.width, self.height)?;

        // Create framebuffer using the first render target
        let framebuffer = if !render_image_views.is_empty() {
            let attachments = [render_image_views[0], depth_image_view];
            let framebuffer_info = vk::FramebufferCreateInfo::default()
                .render_pass(render_pass)
                .attachments(&attachments)
                .width(self.width)
                .height(self.height)
                .layers(1);

            Some(unsafe {
                device
                    .create_framebuffer(&framebuffer_info, None)
                    .map_err(|e| format!("Failed to create framebuffer: {:?}", e))?
            })
        } else {
            None
        };

        self.entry = Some(entry);
        self.instance = Some(instance);
        self.physical_device = Some(physical_device);
        self.device = Some(device);
        self.graphics_queue = Some(graphics_queue);
        self.graphics_queue_family = graphics_queue_family;
        self.command_pool = Some(command_pool);
        self.command_buffers = command_buffers.clone();
        self.current_cmd_buffer = Some(command_buffers[0]);
        self.render_pass = Some(render_pass);
        self.framebuffer = framebuffer;
        self.image_available_semaphores = image_available;
        self.render_finished_semaphores = render_finished;
        self.in_flight_fences = fences;
        self.render_images = render_images;
        self.render_image_views = render_image_views;
        self.render_image_allocations = render_image_allocations;
        self.depth_image = Some(depth_image);
        self.depth_image_view = Some(depth_image_view);
        self.depth_image_allocation = Some(depth_allocation);
        self.allocator = Some(allocator);
        self.initialized = true;

        tracing::info!("Vulkan backend initialized successfully");
        Ok(())
    }

    fn shutdown(&mut self) {
        if !self.initialized {
            return;
        }

        tracing::info!("Shutting down Vulkan backend");

        unsafe {
            if let Some(device) = &self.device {
                device.device_wait_idle().ok();

                // Destroy synchronization objects
                for semaphore in self.image_available_semaphores.drain(..) {
                    device.destroy_semaphore(semaphore, None);
                }
                for semaphore in self.render_finished_semaphores.drain(..) {
                    device.destroy_semaphore(semaphore, None);
                }
                for fence in self.in_flight_fences.drain(..) {
                    device.destroy_fence(fence, None);
                }

                // Destroy render target views and images
                for view in self.render_image_views.drain(..) {
                    device.destroy_image_view(view, None);
                }
                for image in self.render_images.drain(..) {
                    device.destroy_image(image, None);
                }

                // Free render target memory allocations
                if let Some(allocator) = &self.allocator {
                    let mut alloc = allocator.lock().unwrap();
                    for allocation in self.render_image_allocations.drain(..) {
                        alloc.free(allocation).ok();
                    }
                }

                // Destroy depth resources
                if let Some(view) = self.depth_image_view.take() {
                    if view != vk::ImageView::null() {
                        device.destroy_image_view(view, None);
                    }
                }

                if let Some(image) = self.depth_image.take() {
                    if image != vk::Image::null() {
                        device.destroy_image(image, None);
                    }
                }

                // Free depth image memory allocation
                if let Some(allocator) = &self.allocator {
                    if let Some(allocation) = self.depth_image_allocation.take() {
                        allocator.lock().unwrap().free(allocation).ok();
                    }
                }

                // Destroy pipeline resources
                if let Some(pipeline) = self.pipeline.take() {
                    device.destroy_pipeline(pipeline, None);
                }
                if let Some(layout) = self.pipeline_layout.take() {
                    device.destroy_pipeline_layout(layout, None);
                }
                if let Some(layout) = self.descriptor_set_layout.take() {
                    device.destroy_descriptor_set_layout(layout, None);
                }

                if let Some(render_pass) = self.render_pass.take() {
                    device.destroy_render_pass(render_pass, None);
                }

                if let Some(framebuffer) = self.framebuffer.take() {
                    device.destroy_framebuffer(framebuffer, None);
                }

                if let Some(command_pool) = self.command_pool.take() {
                    device.destroy_command_pool(command_pool, None);
                }

                device.destroy_device(None);
            }

            if let Some(instance) = self.instance.take() {
                instance.destroy_instance(None);
            }
        }

        self.entry = None;
        self.device = None;
        self.physical_device = None;
        self.graphics_queue = None;
        self.current_cmd_buffer = None;
        self.command_buffers.clear();
        self.render_images.clear();
        self.depth_image = None;
        self.allocator = None;
        self.initialized = false;
        self.in_render_pass = false;

        tracing::info!("Vulkan backend shut down");
    }

    fn begin_frame(&mut self) {
        if !self.initialized {
            return;
        }

        if let Some(device) = &self.device {
            // Wait for the current frame's fence
            let fence = self.in_flight_fences[self.current_frame];
            unsafe {
                if let Err(e) = device.wait_for_fences(&[fence], true, u64::MAX) {
                    tracing::error!("Failed to wait for fence: {:?}", e);
                    return;
                }
                if let Err(e) = device.reset_fences(&[fence]) {
                    tracing::error!("Failed to reset fence: {:?}", e);
                    return;
                }
            }

            // Get current command buffer
            let cmd_buffer = self.command_buffers[self.current_frame];
            self.current_cmd_buffer = Some(cmd_buffer);

            // Begin recording
            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

            unsafe {
                if let Err(e) = device.begin_command_buffer(cmd_buffer, &begin_info) {
                    tracing::error!("Failed to begin command buffer: {:?}", e);
                }
            }
        }
    }

    fn end_frame(&mut self) {
        if !self.initialized {
            return;
        }

        if let (Some(device), Some(cmd_buffer), Some(queue)) =
            (&self.device, self.current_cmd_buffer, self.graphics_queue)
        {
            // End render pass if we're still in one
            if self.in_render_pass {
                unsafe {
                    device.cmd_end_render_pass(cmd_buffer);
                }
                self.in_render_pass = false;
            }

            unsafe {
                if let Err(e) = device.end_command_buffer(cmd_buffer) {
                    tracing::error!("Failed to end command buffer: {:?}", e);
                    return;
                }

                // Set up synchronization
                let wait_semaphores = [self.image_available_semaphores[self.current_frame]];
                let signal_semaphores = [self.render_finished_semaphores[self.current_frame]];
                let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];

                let cmd_buffers = [cmd_buffer];
                let submit_info = vk::SubmitInfo::default()
                    .wait_semaphores(&wait_semaphores)
                    .wait_dst_stage_mask(&wait_stages)
                    .command_buffers(&cmd_buffers)
                    .signal_semaphores(&signal_semaphores);

                let fence = self.in_flight_fences[self.current_frame];
                if let Err(e) = device.queue_submit(queue, &[submit_info], fence) {
                    tracing::error!("Failed to submit command buffer: {:?}", e);
                    return;
                }

                // Advance to next frame
                self.current_frame = (self.current_frame + 1) % self.max_frames_in_flight;
            }
        }
    }

    fn clear(&mut self, color: [f32; 4], depth: f32, stencil: u8) {
        if !self.initialized {
            return;
        }

        tracing::trace!(
            "Clear: color={:?}, depth={}, stencil={}",
            color,
            depth,
            stencil
        );

        // Begin render pass with clear values if we have a framebuffer
        if let (Some(device), Some(cmd_buffer), Some(render_pass), Some(framebuffer)) =
            (&self.device, self.current_cmd_buffer, self.render_pass, self.framebuffer)
        {
            let clear_values = [
                vk::ClearValue {
                    color: vk::ClearColorValue { float32: color },
                },
                vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth,
                        stencil: stencil as u32,
                    },
                },
            ];

            let render_pass_info = vk::RenderPassBeginInfo::default()
                .render_pass(render_pass)
                .framebuffer(framebuffer)
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D {
                        width: self.width,
                        height: self.height,
                    },
                })
                .clear_values(&clear_values);

            unsafe {
                device.cmd_begin_render_pass(cmd_buffer, &render_pass_info, vk::SubpassContents::INLINE);
            }
            self.in_render_pass = true;
        }
    }

    fn draw_arrays(&mut self, primitive: PrimitiveType, first: u32, count: u32) {
        if !self.initialized {
            return;
        }

        tracing::trace!(
            "Draw arrays: primitive={:?}, first={}, count={}",
            primitive,
            first,
            count
        );

        // Record draw command into command buffer
        if let (Some(device), Some(cmd_buffer)) = (&self.device, self.current_cmd_buffer) {
            // Check if we're in a valid state to draw
            if !self.in_render_pass {
                tracing::warn!("draw_arrays called outside of render pass");
                return;
            }

            let _topology = Self::primitive_to_vk_topology(primitive);

            unsafe {
                // Record the draw command
                // first_vertex = first, vertex_count = count, first_instance = 0, instance_count = 1
                device.cmd_draw(cmd_buffer, count, 1, first, 0);
            }

            tracing::trace!(
                "Recorded draw command: {} vertices starting at {}",
                count,
                first
            );
        }
    }

    fn draw_indexed(&mut self, primitive: PrimitiveType, first: u32, count: u32) {
        if !self.initialized {
            return;
        }

        tracing::trace!(
            "Draw indexed: primitive={:?}, first={}, count={}",
            primitive,
            first,
            count
        );

        // Record indexed draw command into command buffer
        if let (Some(device), Some(cmd_buffer)) = (&self.device, self.current_cmd_buffer) {
            // Check if we're in a valid state to draw
            if !self.in_render_pass {
                tracing::warn!("draw_indexed called outside of render pass");
                return;
            }

            let _topology = Self::primitive_to_vk_topology(primitive);

            unsafe {
                // Record the indexed draw command
                // index_count = count, instance_count = 1, first_index = first,
                // vertex_offset = 0, first_instance = 0
                device.cmd_draw_indexed(cmd_buffer, count, 1, first, 0, 0);
            }

            tracing::trace!(
                "Recorded indexed draw command: {} indices starting at {}",
                count,
                first
            );
        }
    }

    fn set_vertex_attributes(&mut self, attributes: &[VertexAttribute]) {
        if !self.initialized {
            return;
        }

        tracing::trace!("Set vertex attributes: count={}", attributes.len());

        // Configure vertex input state by converting RSX attributes to Vulkan format
        self.vertex_bindings.clear();
        self.vertex_attributes.clear();

        for attr in attributes {
            // Create binding description for each unique binding
            let binding_exists = self.vertex_bindings.iter().any(|b| b.binding == attr.index as u32);
            if !binding_exists {
                self.vertex_bindings.push(vk::VertexInputBindingDescription {
                    binding: attr.index as u32,
                    stride: attr.stride as u32,
                    input_rate: vk::VertexInputRate::VERTEX,
                });
            }

            // Create attribute description
            let format = Self::vertex_type_to_vk_format(attr.type_, attr.size, attr.normalized);
            self.vertex_attributes.push(vk::VertexInputAttributeDescription {
                location: attr.index as u32,
                binding: attr.index as u32,
                format,
                offset: attr.offset,
            });
        }

        tracing::trace!(
            "Configured {} bindings and {} attributes",
            self.vertex_bindings.len(),
            self.vertex_attributes.len()
        );
    }

    fn bind_texture(&mut self, slot: u32, offset: u32) {
        if !self.initialized {
            return;
        }

        tracing::trace!("Bind texture: slot={}, offset=0x{:08x}", slot, offset);

        // TODO: Bind texture descriptor set
        // This would update descriptor sets with the texture at the given offset
    }

    fn set_viewport(&mut self, x: f32, y: f32, width: f32, height: f32, min_depth: f32, max_depth: f32) {
        if !self.initialized {
            return;
        }

        tracing::trace!(
            "Set viewport: x={}, y={}, width={}, height={}, depth=[{}, {}]",
            x, y, width, height, min_depth, max_depth
        );

        if let (Some(device), Some(cmd_buffer)) = (&self.device, self.current_cmd_buffer) {
            let viewport = vk::Viewport {
                x,
                y,
                width,
                height,
                min_depth,
                max_depth,
            };

            unsafe {
                device.cmd_set_viewport(cmd_buffer, 0, &[viewport]);
            }
        }
    }

    fn set_scissor(&mut self, x: u32, y: u32, width: u32, height: u32) {
        if !self.initialized {
            return;
        }

        tracing::trace!(
            "Set scissor: x={}, y={}, width={}, height={}",
            x, y, width, height
        );

        if let (Some(device), Some(cmd_buffer)) = (&self.device, self.current_cmd_buffer) {
            let scissor = vk::Rect2D {
                offset: vk::Offset2D {
                    x: x as i32,
                    y: y as i32,
                },
                extent: vk::Extent2D { width, height },
            };

            unsafe {
                device.cmd_set_scissor(cmd_buffer, 0, &[scissor]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vulkan_backend_creation() {
        let backend = VulkanBackend::new();
        assert!(!backend.initialized);
        assert_eq!(backend.max_frames_in_flight, 2);
    }

    #[test]
    fn test_vulkan_backend_with_frames() {
        let backend = VulkanBackend::with_frames_in_flight(3);
        assert!(!backend.initialized);
        assert_eq!(backend.max_frames_in_flight, 3);
    }

    #[test]
    fn test_vulkan_backend_init() {
        let mut backend = VulkanBackend::new();
        // Note: This may fail in CI environments without Vulkan support
        // In production, we'd want to handle this gracefully
        match backend.init() {
            Ok(_) => {
                assert!(backend.initialized);
                assert_eq!(backend.command_buffers.len(), 2);
                assert_eq!(backend.image_available_semaphores.len(), 2);
                assert_eq!(backend.render_finished_semaphores.len(), 2);
                assert_eq!(backend.in_flight_fences.len(), 2);
                backend.shutdown();
                assert!(!backend.initialized);
            }
            Err(e) => {
                // Expected in environments without Vulkan
                tracing::warn!("Vulkan init failed (expected in CI): {}", e);
            }
        }
    }

    #[test]
    fn test_draw_commands_without_init() {
        use crate::backend::PrimitiveType;
        let mut backend = VulkanBackend::new();
        
        // These should not crash even if backend is not initialized
        backend.draw_arrays(PrimitiveType::Triangles, 0, 3);
        backend.draw_indexed(PrimitiveType::Triangles, 0, 3);
        backend.set_viewport(0.0, 0.0, 800.0, 600.0, 0.0, 1.0);
        backend.set_scissor(0, 0, 800, 600);
    }
}
