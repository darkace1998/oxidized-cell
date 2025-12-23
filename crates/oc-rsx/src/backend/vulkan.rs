//! Vulkan backend for RSX
//!
//! This module contains the Vulkan implementation for RSX rendering.

use super::GraphicsBackend;
use ash::vk;
use std::ffi::CString;

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
    /// Current command buffer
    current_cmd_buffer: Option<vk::CommandBuffer>,
    /// Current render pass
    render_pass: Option<vk::RenderPass>,
    /// Current framebuffer
    framebuffer: Option<vk::Framebuffer>,
    /// Whether backend is initialized
    initialized: bool,
}

impl VulkanBackend {
    /// Create a new Vulkan backend
    pub fn new() -> Self {
        Self {
            entry: None,
            instance: None,
            physical_device: None,
            device: None,
            graphics_queue: None,
            graphics_queue_family: 0,
            command_pool: None,
            current_cmd_buffer: None,
            render_pass: None,
            framebuffer: None,
            initialized: false,
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

        // Create command pool
        let command_pool = Self::create_command_pool(&device, graphics_queue_family)?;

        // Allocate command buffer
        let cmd_buffer = Self::allocate_command_buffer(&device, command_pool)?;

        // Create render pass
        let render_pass = Self::create_render_pass(&device)?;

        self.entry = Some(entry);
        self.instance = Some(instance);
        self.physical_device = Some(physical_device);
        self.device = Some(device);
        self.graphics_queue = Some(graphics_queue);
        self.graphics_queue_family = graphics_queue_family;
        self.command_pool = Some(command_pool);
        self.current_cmd_buffer = Some(cmd_buffer);
        self.render_pass = Some(render_pass);
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
        self.initialized = false;

        tracing::info!("Vulkan backend shut down");
    }

    fn begin_frame(&mut self) {
        if !self.initialized {
            return;
        }

        if let (Some(device), Some(cmd_buffer)) = (&self.device, self.current_cmd_buffer) {
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
            unsafe {
                if let Err(e) = device.end_command_buffer(cmd_buffer) {
                    tracing::error!("Failed to end command buffer: {:?}", e);
                    return;
                }

                let cmd_buffers = [cmd_buffer];
                let submit_info = vk::SubmitInfo::default().command_buffers(&cmd_buffers);

                if let Err(e) = device.queue_submit(queue, &[submit_info], vk::Fence::null()) {
                    tracing::error!("Failed to submit command buffer: {:?}", e);
                    return;
                }

                if let Err(e) = device.queue_wait_idle(queue) {
                    tracing::error!("Failed to wait for queue idle: {:?}", e);
                }
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

        // Clear operations would be recorded into the command buffer
        // In a real implementation, this would set up clear values for the render pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vulkan_backend_creation() {
        let backend = VulkanBackend::new();
        assert!(!backend.initialized);
    }

    #[test]
    fn test_vulkan_backend_init() {
        let mut backend = VulkanBackend::new();
        // Note: This may fail in CI environments without Vulkan support
        // In production, we'd want to handle this gracefully
        match backend.init() {
            Ok(_) => {
                assert!(backend.initialized);
                backend.shutdown();
                assert!(!backend.initialized);
            }
            Err(e) => {
                // Expected in environments without Vulkan
                tracing::warn!("Vulkan init failed (expected in CI): {}", e);
            }
        }
    }
}
