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

#[allow(dead_code)]
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
    /// Descriptor set layout for texture samplers
    descriptor_set_layout: Option<vk::DescriptorSetLayout>,
    /// Descriptor pool for allocating descriptor sets
    descriptor_pool: Option<vk::DescriptorPool>,
    /// Descriptor sets for texture binding (one per frame in flight)
    descriptor_sets: Vec<vk::DescriptorSet>,
    /// Texture images (16 texture units max)
    texture_images: [Option<vk::Image>; 16],
    /// Texture image views
    texture_image_views: [Option<vk::ImageView>; 16],
    /// Texture image allocations
    texture_allocations: [Option<Allocation>; 16],
    /// Texture samplers (16 texture units max)
    texture_samplers: [Option<vk::Sampler>; 16],
    /// Whether each texture slot is bound
    texture_bound: [bool; 16],
    /// Current vertex shader module
    vertex_shader: Option<vk::ShaderModule>,
    /// Current fragment shader module
    fragment_shader: Option<vk::ShaderModule>,
    /// Shader translator
    shader_translator: crate::shader::ShaderTranslator,
    /// Vertex input bindings
    vertex_bindings: Vec<vk::VertexInputBindingDescription>,
    /// Vertex input attributes
    vertex_attributes: Vec<vk::VertexInputAttributeDescription>,
    /// Whether we're in a render pass
    in_render_pass: bool,
    /// MSAA sample count
    msaa_samples: vk::SampleCountFlags,
    /// MSAA color resolve images (one per MRT)
    msaa_color_images: Vec<vk::Image>,
    /// MSAA color image views
    msaa_color_image_views: Vec<vk::ImageView>,
    /// MSAA color image allocations
    msaa_color_allocations: Vec<Allocation>,
    /// MSAA depth image
    msaa_depth_image: Option<vk::Image>,
    /// MSAA depth image view
    msaa_depth_image_view: Option<vk::ImageView>,
    /// MSAA depth allocation
    msaa_depth_allocation: Option<Allocation>,
    /// Multiple render targets (MRT) - up to 4 color attachments
    mrt_images: Vec<Vec<vk::Image>>,
    /// MRT image views
    mrt_image_views: Vec<Vec<vk::ImageView>>,
    /// MRT allocations
    mrt_allocations: Vec<Vec<Allocation>>,
    /// Number of active render targets
    active_mrt_count: u32,
    /// Render-to-texture framebuffers (texture offset -> framebuffer)
    rtt_framebuffers: Vec<(u32, vk::Framebuffer)>,
    /// Anisotropic filtering level
    anisotropy_level: f32,
    /// Maximum supported anisotropy
    max_anisotropy: f32,
    /// Vertex buffers (binding index -> (buffer, allocation, size))
    vertex_buffers: Vec<(u32, vk::Buffer, Option<Allocation>, u64)>,
    /// Index buffer (buffer, allocation, size, index_type)
    index_buffer: Option<(vk::Buffer, Option<Allocation>, u64, vk::IndexType)>,
    /// Pipeline cache for reusing compiled pipelines
    pipeline_cache_handle: Option<vk::PipelineCache>,
    /// Cached pipelines by hash key
    pipeline_cache: std::collections::HashMap<u64, vk::Pipeline>,
    /// MSAA sample mask for sample coverage
    sample_mask: u32,
    /// Suballocation pool for small buffers
    suballocation_pool: Option<SuballocationPool>,
    /// Staging buffer pool for uploads
    staging_pool: Option<StagingBufferPool>,
    /// Fence pool for frame pacing
    fence_pool: FencePool,
    /// Timeline semaphore for RSX semaphores (Vulkan 1.2+)
    timeline_semaphore: Option<vk::Semaphore>,
    /// Current timeline value
    timeline_value: u64,
    /// Compute pipeline layout for RSX emulation
    compute_pipeline_layout: Option<vk::PipelineLayout>,
    /// Compute pipelines
    compute_pipelines: std::collections::HashMap<String, vk::Pipeline>,
    /// Compute descriptor set layout
    compute_descriptor_set_layout: Option<vk::DescriptorSetLayout>,
    /// Dynamic states enabled for pipelines
    dynamic_states_enabled: Vec<vk::DynamicState>,
    /// Per-attachment blend states (up to 4 MRT)
    per_attachment_blend: [BlendAttachmentConfig; 4],
}

/// Small buffer suballocation pool
#[derive(Debug)]
pub struct SuballocationPool {
    /// Pool of suballocations
    blocks: Vec<SuballocationBlock>,
    /// Block size
    block_size: u64,
    /// Alignment requirement
    alignment: u64,
    /// Statistics
    stats: SuballocationStats,
}

/// A block in the suballocation pool
#[derive(Debug)]
struct SuballocationBlock {
    buffer: vk::Buffer,
    allocation: Option<Allocation>,
    size: u64,
    used: u64,
    /// Free list: (offset, size)
    free_list: Vec<(u64, u64)>,
}

/// Suballocation statistics
#[derive(Debug, Default, Clone)]
pub struct SuballocationStats {
    /// Total bytes allocated
    pub total_allocated: u64,
    /// Total bytes used
    pub total_used: u64,
    /// Number of suballocations
    pub allocation_count: u64,
    /// Number of blocks
    pub block_count: u64,
}

impl SuballocationPool {
    /// Default block size: 4MB
    const DEFAULT_BLOCK_SIZE: u64 = 4 * 1024 * 1024;
    /// Default alignment: 256 bytes
    const DEFAULT_ALIGNMENT: u64 = 256;
    
    /// Create a new suballocation pool
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            block_size: Self::DEFAULT_BLOCK_SIZE,
            alignment: Self::DEFAULT_ALIGNMENT,
            stats: SuballocationStats::default(),
        }
    }
    
    /// Create with custom block size
    pub fn with_block_size(block_size: u64) -> Self {
        Self {
            blocks: Vec::new(),
            block_size,
            alignment: Self::DEFAULT_ALIGNMENT,
            stats: SuballocationStats::default(),
        }
    }
    
    /// Allocate from pool, returns (buffer, offset)
    pub fn allocate(
        &mut self,
        device: &ash::Device,
        allocator: &Arc<Mutex<Allocator>>,
        size: u64,
        usage: vk::BufferUsageFlags,
    ) -> Result<(vk::Buffer, u64), String> {
        let aligned_size = (size + self.alignment - 1) & !(self.alignment - 1);
        
        // Try to find space in existing blocks
        for block in &mut self.blocks {
            if let Some((free_list_idx, (free_offset, free_size))) = block.free_list.iter()
                .enumerate()
                .find(|(_, (_, s))| *s >= aligned_size)
                .map(|(i, &entry)| (i, entry))
            {
                block.free_list.remove(free_list_idx);
                if free_size > aligned_size {
                    block.free_list.push((free_offset + aligned_size, free_size - aligned_size));
                }
                block.used += aligned_size;
                self.stats.total_used += aligned_size;
                self.stats.allocation_count += 1;
                return Ok((block.buffer, free_offset));
            }
        }
        
        // Need new block
        let block_size = self.block_size.max(aligned_size);
        
        let buffer_info = vk::BufferCreateInfo::default()
            .size(block_size)
            .usage(usage | vk::BufferUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let buffer = unsafe {
            device.create_buffer(&buffer_info, None)
                .map_err(|e| format!("Failed to create suballocation block: {:?}", e))?
        };
        
        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
        
        let alloc_desc = AllocationCreateDesc {
            name: "suballocation_block",
            requirements,
            location: MemoryLocation::GpuOnly,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        };
        
        let allocation = allocator.lock().unwrap()
            .allocate(&alloc_desc)
            .map_err(|e| format!("Failed to allocate suballocation block memory: {:?}", e))?;
        
        unsafe {
            device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
                .map_err(|e| format!("Failed to bind suballocation block memory: {:?}", e))?;
        }
        
        let mut block = SuballocationBlock {
            buffer,
            allocation: Some(allocation),
            size: block_size,
            used: aligned_size,
            free_list: Vec::new(),
        };
        
        if block_size > aligned_size {
            block.free_list.push((aligned_size, block_size - aligned_size));
        }
        
        let result_buffer = block.buffer;
        self.blocks.push(block);
        
        self.stats.total_allocated += block_size;
        self.stats.total_used += aligned_size;
        self.stats.allocation_count += 1;
        self.stats.block_count += 1;
        
        Ok((result_buffer, 0))
    }
    
    /// Free a suballocation
    pub fn free(&mut self, buffer: vk::Buffer, offset: u64, size: u64) {
        let aligned_size = (size + self.alignment - 1) & !(self.alignment - 1);
        
        for block in &mut self.blocks {
            if block.buffer == buffer {
                block.free_list.push((offset, aligned_size));
                block.used = block.used.saturating_sub(aligned_size);
                self.stats.total_used = self.stats.total_used.saturating_sub(aligned_size);
                self.stats.allocation_count = self.stats.allocation_count.saturating_sub(1);
                
                // Merge adjacent free regions
                block.free_list.sort_by_key(|&(off, _)| off);
                let mut i = 0;
                while i + 1 < block.free_list.len() {
                    let (off1, size1) = block.free_list[i];
                    let (off2, size2) = block.free_list[i + 1];
                    if off1 + size1 == off2 {
                        block.free_list[i] = (off1, size1 + size2);
                        block.free_list.remove(i + 1);
                    } else {
                        i += 1;
                    }
                }
                break;
            }
        }
    }
    
    /// Get statistics
    pub fn stats(&self) -> &SuballocationStats {
        &self.stats
    }
    
    /// Cleanup and destroy pool
    pub fn destroy(&mut self, device: &ash::Device, allocator: &Arc<Mutex<Allocator>>) {
        for block in self.blocks.drain(..) {
            unsafe {
                device.destroy_buffer(block.buffer, None);
            }
            if let Some(alloc) = block.allocation {
                allocator.lock().unwrap().free(alloc).ok();
            }
        }
        self.stats = SuballocationStats::default();
    }
}

impl Default for SuballocationPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Staging buffer pool for uploads
#[derive(Debug)]
pub struct StagingBufferPool {
    /// Pool of available staging buffers
    available: Vec<StagingBuffer>,
    /// Currently in-use staging buffers
    in_use: Vec<StagingBuffer>,
    /// Default buffer size
    default_size: u64,
    /// Statistics
    stats: StagingBufferStats,
}

/// A staging buffer
#[derive(Debug)]
struct StagingBuffer {
    buffer: vk::Buffer,
    allocation: Option<Allocation>,
    size: u64,
    /// Fence to track when buffer can be reused
    fence: Option<vk::Fence>,
}

/// Staging buffer statistics
#[derive(Debug, Default, Clone)]
pub struct StagingBufferStats {
    /// Total buffers created
    pub buffers_created: u64,
    /// Total buffers reused
    pub buffers_reused: u64,
    /// Total bytes transferred
    pub bytes_transferred: u64,
    /// Current pool size
    pub pool_size: u64,
}

impl StagingBufferPool {
    /// Default staging buffer size: 1MB
    const DEFAULT_BUFFER_SIZE: u64 = 1024 * 1024;
    
    /// Create a new staging buffer pool
    pub fn new() -> Self {
        Self {
            available: Vec::new(),
            in_use: Vec::new(),
            default_size: Self::DEFAULT_BUFFER_SIZE,
            stats: StagingBufferStats::default(),
        }
    }
    
    /// Acquire a staging buffer for upload
    pub fn acquire(
        &mut self,
        device: &ash::Device,
        allocator: &Arc<Mutex<Allocator>>,
        size: u64,
    ) -> Result<(vk::Buffer, Option<Allocation>), String> {
        let required_size = size.max(self.default_size);
        
        // Try to reclaim completed buffers
        self.reclaim(device);
        
        // Find suitable buffer from pool
        if let Some(idx) = self.available.iter().position(|b| b.size >= required_size) {
            let buffer = self.available.remove(idx);
            self.stats.buffers_reused += 1;
            let result = (buffer.buffer, buffer.allocation);
            return Ok(result);
        }
        
        // Create new buffer
        let buffer_info = vk::BufferCreateInfo::default()
            .size(required_size)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let buffer = unsafe {
            device.create_buffer(&buffer_info, None)
                .map_err(|e| format!("Failed to create staging buffer: {:?}", e))?
        };
        
        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
        
        let alloc_desc = AllocationCreateDesc {
            name: "staging_buffer",
            requirements,
            location: MemoryLocation::CpuToGpu,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        };
        
        let allocation = allocator.lock().unwrap()
            .allocate(&alloc_desc)
            .map_err(|e| format!("Failed to allocate staging buffer memory: {:?}", e))?;
        
        unsafe {
            device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
                .map_err(|e| format!("Failed to bind staging buffer memory: {:?}", e))?;
        }
        
        self.stats.buffers_created += 1;
        self.stats.pool_size += required_size;
        
        Ok((buffer, Some(allocation)))
    }
    
    /// Release a staging buffer back to pool with a fence
    pub fn release(&mut self, buffer: vk::Buffer, allocation: Option<Allocation>, size: u64, fence: Option<vk::Fence>) {
        self.in_use.push(StagingBuffer {
            buffer,
            allocation,
            size,
            fence,
        });
    }
    
    /// Reclaim completed buffers
    fn reclaim(&mut self, device: &ash::Device) {
        let mut still_in_use = Vec::new();
        
        for buffer in self.in_use.drain(..) {
            let ready = if let Some(fence) = buffer.fence {
                match unsafe { device.get_fence_status(fence) } {
                    Ok(signaled) => signaled,
                    Err(e) => {
                        // Log error but keep buffer in use to avoid premature reuse
                        tracing::warn!("Fence status query failed: {:?}, keeping buffer in use", e);
                        false
                    }
                }
            } else {
                true
            };
            
            if ready {
                self.available.push(StagingBuffer {
                    buffer: buffer.buffer,
                    allocation: buffer.allocation,
                    size: buffer.size,
                    fence: None,
                });
            } else {
                still_in_use.push(buffer);
            }
        }
        
        self.in_use = still_in_use;
    }
    
    /// Get statistics
    pub fn stats(&self) -> &StagingBufferStats {
        &self.stats
    }
    
    /// Record bytes transferred
    pub fn record_transfer(&mut self, bytes: u64) {
        self.stats.bytes_transferred += bytes;
    }
    
    /// Cleanup and destroy pool
    pub fn destroy(&mut self, device: &ash::Device, allocator: &Arc<Mutex<Allocator>>) {
        for buffer in self.available.drain(..).chain(self.in_use.drain(..)) {
            unsafe {
                device.destroy_buffer(buffer.buffer, None);
            }
            if let Some(alloc) = buffer.allocation {
                allocator.lock().unwrap().free(alloc).ok();
            }
        }
        self.stats = StagingBufferStats::default();
    }
}

impl Default for StagingBufferPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Fence pool for frame pacing
#[derive(Debug, Default)]
pub struct FencePool {
    /// Available fences
    available: Vec<vk::Fence>,
    /// In-use fences
    in_use: Vec<vk::Fence>,
}

impl FencePool {
    /// Create a new fence pool
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Acquire a fence
    pub fn acquire(&mut self, device: &ash::Device) -> Result<vk::Fence, String> {
        // Reclaim completed fences
        self.reclaim(device);
        
        if let Some(fence) = self.available.pop() {
            unsafe {
                device.reset_fences(&[fence])
                    .map_err(|e| format!("Failed to reset fence: {:?}", e))?;
            }
            self.in_use.push(fence);
            return Ok(fence);
        }
        
        // Create new fence
        let fence_info = vk::FenceCreateInfo::default();
        let fence = unsafe {
            device.create_fence(&fence_info, None)
                .map_err(|e| format!("Failed to create fence: {:?}", e))?
        };
        
        self.in_use.push(fence);
        Ok(fence)
    }
    
    /// Release a fence back to pool
    pub fn release(&mut self, fence: vk::Fence) {
        if let Some(idx) = self.in_use.iter().position(|&f| f == fence) {
            self.in_use.remove(idx);
            self.available.push(fence);
        }
    }
    
    /// Reclaim completed fences
    fn reclaim(&mut self, device: &ash::Device) {
        let mut still_in_use = Vec::new();
        
        for fence in self.in_use.drain(..) {
            let signaled = match unsafe { device.get_fence_status(fence) } {
                Ok(status) => status,
                Err(e) => {
                    // Log error but keep fence in use to avoid premature reuse
                    tracing::warn!("Fence status query failed: {:?}, keeping fence in use", e);
                    false
                }
            };
            if signaled {
                self.available.push(fence);
            } else {
                still_in_use.push(fence);
            }
        }
        
        self.in_use = still_in_use;
    }
    
    /// Wait for all in-use fences
    pub fn wait_all(&self, device: &ash::Device, timeout: u64) -> Result<(), String> {
        if self.in_use.is_empty() {
            return Ok(());
        }
        unsafe {
            device.wait_for_fences(&self.in_use, true, timeout)
                .map_err(|e| format!("Failed to wait for fences: {:?}", e))
        }
    }
    
    /// Destroy all fences
    pub fn destroy(&mut self, device: &ash::Device) {
        for fence in self.available.drain(..).chain(self.in_use.drain(..)) {
            unsafe {
                device.destroy_fence(fence, None);
            }
        }
    }
}

/// Per-attachment blend configuration
#[derive(Debug, Clone, Copy)]
pub struct BlendAttachmentConfig {
    /// Whether blending is enabled
    pub blend_enable: bool,
    /// Source color blend factor
    pub src_color_factor: vk::BlendFactor,
    /// Destination color blend factor
    pub dst_color_factor: vk::BlendFactor,
    /// Color blend operation
    pub color_blend_op: vk::BlendOp,
    /// Source alpha blend factor
    pub src_alpha_factor: vk::BlendFactor,
    /// Destination alpha blend factor
    pub dst_alpha_factor: vk::BlendFactor,
    /// Alpha blend operation
    pub alpha_blend_op: vk::BlendOp,
    /// Color write mask
    pub write_mask: vk::ColorComponentFlags,
}

impl Default for BlendAttachmentConfig {
    fn default() -> Self {
        Self {
            blend_enable: false,
            src_color_factor: vk::BlendFactor::ONE,
            dst_color_factor: vk::BlendFactor::ZERO,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_factor: vk::BlendFactor::ONE,
            dst_alpha_factor: vk::BlendFactor::ZERO,
            alpha_blend_op: vk::BlendOp::ADD,
            write_mask: vk::ColorComponentFlags::RGBA,
        }
    }
}

impl BlendAttachmentConfig {
    /// Convert to Vulkan attachment state
    pub fn to_vk(&self) -> vk::PipelineColorBlendAttachmentState {
        vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(self.blend_enable)
            .src_color_blend_factor(self.src_color_factor)
            .dst_color_blend_factor(self.dst_color_factor)
            .color_blend_op(self.color_blend_op)
            .src_alpha_blend_factor(self.src_alpha_factor)
            .dst_alpha_blend_factor(self.dst_alpha_factor)
            .alpha_blend_op(self.alpha_blend_op)
            .color_write_mask(self.write_mask)
    }
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
            descriptor_pool: None,
            descriptor_sets: Vec::new(),
            texture_images: Default::default(),
            texture_image_views: Default::default(),
            texture_allocations: Default::default(),
            texture_samplers: Default::default(),
            texture_bound: [false; 16],
            vertex_shader: None,
            fragment_shader: None,
            shader_translator: crate::shader::ShaderTranslator::new(),
            vertex_bindings: Vec::new(),
            vertex_attributes: Vec::new(),
            in_render_pass: false,
            msaa_samples: vk::SampleCountFlags::TYPE_1,
            msaa_color_images: Vec::new(),
            msaa_color_image_views: Vec::new(),
            msaa_color_allocations: Vec::new(),
            msaa_depth_image: None,
            msaa_depth_image_view: None,
            msaa_depth_allocation: None,
            mrt_images: Vec::new(),
            mrt_image_views: Vec::new(),
            mrt_allocations: Vec::new(),
            active_mrt_count: 1,
            rtt_framebuffers: Vec::new(),
            anisotropy_level: 1.0,
            max_anisotropy: 16.0,
            vertex_buffers: Vec::new(),
            index_buffer: None,
            pipeline_cache_handle: None,
            pipeline_cache: std::collections::HashMap::new(),
            sample_mask: 0xFFFFFFFF,
            suballocation_pool: None,
            staging_pool: None,
            fence_pool: FencePool::new(),
            timeline_semaphore: None,
            timeline_value: 0,
            compute_pipeline_layout: None,
            compute_pipelines: std::collections::HashMap::new(),
            compute_descriptor_set_layout: None,
            dynamic_states_enabled: vec![vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR],
            per_attachment_blend: [BlendAttachmentConfig::default(); 4],
        }
    }

    /// Create a new Vulkan backend with MSAA
    pub fn with_msaa(max_frames: usize, sample_count: u32) -> Self {
        let mut backend = Self::with_frames_in_flight(max_frames);
        backend.msaa_samples = Self::sample_count_to_flags(sample_count);
        backend
    }

    /// Convert sample count to Vulkan flags
    fn sample_count_to_flags(count: u32) -> vk::SampleCountFlags {
        match count {
            1 => vk::SampleCountFlags::TYPE_1,
            2 => vk::SampleCountFlags::TYPE_2,
            4 => vk::SampleCountFlags::TYPE_4,
            8 => vk::SampleCountFlags::TYPE_8,
            16 => vk::SampleCountFlags::TYPE_16,
            32 => vk::SampleCountFlags::TYPE_32,
            64 => vk::SampleCountFlags::TYPE_64,
            _ => vk::SampleCountFlags::TYPE_1,
        }
    }

    /// Set MSAA sample count
    pub fn set_msaa_samples(&mut self, count: u32) {
        self.msaa_samples = Self::sample_count_to_flags(count);
    }

    /// Get current MSAA sample count
    pub fn msaa_sample_count(&self) -> u32 {
        match self.msaa_samples {
            vk::SampleCountFlags::TYPE_1 => 1,
            vk::SampleCountFlags::TYPE_2 => 2,
            vk::SampleCountFlags::TYPE_4 => 4,
            vk::SampleCountFlags::TYPE_8 => 8,
            vk::SampleCountFlags::TYPE_16 => 16,
            vk::SampleCountFlags::TYPE_32 => 32,
            vk::SampleCountFlags::TYPE_64 => 64,
            _ => 1,
        }
    }

    /// Set number of active render targets (MRT)
    pub fn set_active_mrt_count(&mut self, count: u32) {
        self.active_mrt_count = count.clamp(1, 4);
    }

    /// Get number of active render targets
    pub fn active_mrt_count(&self) -> u32 {
        self.active_mrt_count
    }

    /// Set anisotropic filtering level
    pub fn set_anisotropy_level(&mut self, level: f32) {
        self.anisotropy_level = level.clamp(1.0, self.max_anisotropy);
    }

    /// Get anisotropic filtering level
    pub fn anisotropy_level(&self) -> f32 {
        self.anisotropy_level
    }
    
    /// Set sample mask for MSAA sample coverage
    pub fn set_sample_mask(&mut self, mask: u32) {
        self.sample_mask = mask;
    }
    
    /// Get sample mask
    pub fn sample_mask(&self) -> u32 {
        self.sample_mask
    }
    
    /// Set per-attachment blend state
    pub fn set_attachment_blend(&mut self, attachment: usize, config: BlendAttachmentConfig) {
        if attachment < 4 {
            self.per_attachment_blend[attachment] = config;
        }
    }
    
    /// Get per-attachment blend state
    pub fn attachment_blend(&self, attachment: usize) -> Option<&BlendAttachmentConfig> {
        self.per_attachment_blend.get(attachment)
    }
    
    /// Enable dynamic states for pipelines
    pub fn set_dynamic_states(&mut self, states: Vec<vk::DynamicState>) {
        self.dynamic_states_enabled = states;
    }
    
    /// Get enabled dynamic states
    pub fn dynamic_states(&self) -> &[vk::DynamicState] {
        &self.dynamic_states_enabled
    }
    
    /// Create pipeline cache
    pub fn create_pipeline_cache(&mut self) -> Result<(), String> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        
        let cache_info = vk::PipelineCacheCreateInfo::default();
        
        let cache = unsafe {
            device.create_pipeline_cache(&cache_info, None)
                .map_err(|e| format!("Failed to create pipeline cache: {:?}", e))?
        };
        
        self.pipeline_cache_handle = Some(cache);
        Ok(())
    }
    
    /// Get cached pipeline by key (hash of pipeline state)
    pub fn get_cached_pipeline(&self, key: u64) -> Option<vk::Pipeline> {
        self.pipeline_cache.get(&key).copied()
    }
    
    /// Store pipeline in cache
    pub fn cache_pipeline(&mut self, key: u64, pipeline: vk::Pipeline) {
        self.pipeline_cache.insert(key, pipeline);
    }
    
    /// Get suballocation pool statistics
    pub fn suballocation_stats(&self) -> Option<&SuballocationStats> {
        self.suballocation_pool.as_ref().map(|p| p.stats())
    }
    
    /// Get staging pool statistics
    pub fn staging_stats(&self) -> Option<&StagingBufferStats> {
        self.staging_pool.as_ref().map(|p| p.stats())
    }
    
    /// Initialize suballocation pool
    pub fn init_suballocation_pool(&mut self) {
        self.suballocation_pool = Some(SuballocationPool::new());
    }
    
    /// Initialize staging buffer pool
    pub fn init_staging_pool(&mut self) {
        self.staging_pool = Some(StagingBufferPool::new());
    }
    
    /// Allocate small buffer from suballocation pool
    pub fn allocate_small_buffer(
        &mut self,
        size: u64,
        usage: vk::BufferUsageFlags,
    ) -> Result<(vk::Buffer, u64), String> {
        // Initialize pool if needed first
        if self.suballocation_pool.is_none() {
            self.init_suballocation_pool();
        }
        
        // Now get references we need
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        let allocator = self.allocator.as_ref().ok_or("Allocator not initialized")?;
        
        self.suballocation_pool
            .as_mut()
            .unwrap()
            .allocate(device, allocator, size, usage)
    }
    
    /// Free small buffer from suballocation pool
    pub fn free_small_buffer(&mut self, buffer: vk::Buffer, offset: u64, size: u64) {
        if let Some(pool) = &mut self.suballocation_pool {
            pool.free(buffer, offset, size);
        }
    }
    
    /// Acquire staging buffer for upload
    pub fn acquire_staging_buffer(&mut self, size: u64) -> Result<(vk::Buffer, Option<Allocation>), String> {
        // Initialize pool if needed first
        if self.staging_pool.is_none() {
            self.init_staging_pool();
        }
        
        // Now get references we need
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        let allocator = self.allocator.as_ref().ok_or("Allocator not initialized")?;
        
        self.staging_pool
            .as_mut()
            .unwrap()
            .acquire(device, allocator, size)
    }
    
    /// Release staging buffer back to pool
    pub fn release_staging_buffer(&mut self, buffer: vk::Buffer, allocation: Option<Allocation>, size: u64, fence: Option<vk::Fence>) {
        if let Some(pool) = &mut self.staging_pool {
            pool.release(buffer, allocation, size, fence);
        }
    }
    
    /// Acquire fence from pool
    pub fn acquire_fence(&mut self) -> Result<vk::Fence, String> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        self.fence_pool.acquire(device)
    }
    
    /// Release fence back to pool
    pub fn release_fence(&mut self, fence: vk::Fence) {
        self.fence_pool.release(fence);
    }
    
    /// Wait for all fences in pool
    pub fn wait_all_fences(&self, timeout: u64) -> Result<(), String> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        self.fence_pool.wait_all(device, timeout)
    }
    
    /// Create timeline semaphore for RSX semaphore emulation
    pub fn create_timeline_semaphore(&mut self) -> Result<(), String> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        
        let mut type_info = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(0);
        
        let semaphore_info = vk::SemaphoreCreateInfo::default()
            .push_next(&mut type_info);
        
        let semaphore = unsafe {
            device.create_semaphore(&semaphore_info, None)
                .map_err(|e| format!("Failed to create timeline semaphore: {:?}", e))?
        };
        
        self.timeline_semaphore = Some(semaphore);
        self.timeline_value = 0;
        Ok(())
    }
    
    /// Signal timeline semaphore
    pub fn signal_timeline(&mut self, value: u64) -> Result<(), String> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        let semaphore = self.timeline_semaphore.ok_or("Timeline semaphore not created")?;
        
        let signal_info = vk::SemaphoreSignalInfo::default()
            .semaphore(semaphore)
            .value(value);
        
        unsafe {
            device.signal_semaphore(&signal_info)
                .map_err(|e| format!("Failed to signal timeline semaphore: {:?}", e))?;
        }
        
        self.timeline_value = value;
        Ok(())
    }
    
    /// Wait for timeline semaphore
    pub fn wait_timeline(&self, value: u64, timeout: u64) -> Result<(), String> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        let semaphore = self.timeline_semaphore.ok_or("Timeline semaphore not created")?;
        
        let wait_info = vk::SemaphoreWaitInfo::default()
            .semaphores(std::slice::from_ref(&semaphore))
            .values(std::slice::from_ref(&value));
        
        unsafe {
            device.wait_semaphores(&wait_info, timeout)
                .map_err(|e| format!("Failed to wait for timeline semaphore: {:?}", e))
        }
    }
    
    /// Get current timeline value
    pub fn timeline_value(&self) -> u64 {
        self.timeline_value
    }
    
    /// Resolve MSAA image to non-MSAA target
    pub fn resolve_msaa(
        &self,
        src_image: vk::Image,
        dst_image: vk::Image,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        let queue = self.graphics_queue.ok_or("Graphics queue not available")?;
        let command_pool = self.command_pool.ok_or("Command pool not available")?;
        
        // Allocate one-time command buffer
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        
        let cmd_buffer = unsafe {
            device.allocate_command_buffers(&alloc_info)
                .map_err(|e| format!("Failed to allocate command buffer: {:?}", e))?
        }[0];
        
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        
        unsafe {
            device.begin_command_buffer(cmd_buffer, &begin_info)
                .map_err(|e| format!("Failed to begin command buffer: {:?}", e))?;
            
            // Image resolve
            let resolve_region = vk::ImageResolve::default()
                .src_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .dst_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .extent(vk::Extent3D { width, height, depth: 1 });
            
            device.cmd_resolve_image(
                cmd_buffer,
                src_image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                dst_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[resolve_region],
            );
            
            device.end_command_buffer(cmd_buffer)
                .map_err(|e| format!("Failed to end command buffer: {:?}", e))?;
            
            let submit_info = vk::SubmitInfo::default()
                .command_buffers(std::slice::from_ref(&cmd_buffer));
            
            device.queue_submit(queue, &[submit_info], vk::Fence::null())
                .map_err(|e| format!("Failed to submit resolve command: {:?}", e))?;
            
            device.queue_wait_idle(queue)
                .map_err(|e| format!("Failed to wait for queue: {:?}", e))?;
            
            device.free_command_buffers(command_pool, &[cmd_buffer]);
        }
        
        Ok(())
    }
    
    /// Create compute pipeline for RSX emulation
    pub fn create_compute_pipeline(
        &mut self,
        shader_spirv: &[u32],
        name: &str,
    ) -> Result<vk::Pipeline, String> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        
        // Create shader module
        let shader_info = vk::ShaderModuleCreateInfo::default()
            .code(shader_spirv);
        
        let shader_module = unsafe {
            device.create_shader_module(&shader_info, None)
                .map_err(|e| format!("Failed to create compute shader module: {:?}", e))?
        };
        
        // Create compute pipeline layout if needed
        if self.compute_pipeline_layout.is_none() {
            // Create descriptor set layout for compute
            let bindings = [
                vk::DescriptorSetLayoutBinding::default()
                    .binding(0)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::COMPUTE),
                vk::DescriptorSetLayoutBinding::default()
                    .binding(1)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::COMPUTE),
            ];
            
            let layout_info = vk::DescriptorSetLayoutCreateInfo::default()
                .bindings(&bindings);
            
            let desc_layout = unsafe {
                device.create_descriptor_set_layout(&layout_info, None)
                    .map_err(|e| format!("Failed to create compute descriptor layout: {:?}", e))?
            };
            self.compute_descriptor_set_layout = Some(desc_layout);
            
            // Create pipeline layout
            let set_layouts = [desc_layout];
            let push_constant_range = vk::PushConstantRange::default()
                .stage_flags(vk::ShaderStageFlags::COMPUTE)
                .offset(0)
                .size(64); // 64 bytes (16 floats at 4 bytes each) for compute params
            
            let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
                .set_layouts(&set_layouts)
                .push_constant_ranges(std::slice::from_ref(&push_constant_range));
            
            let layout = unsafe {
                device.create_pipeline_layout(&pipeline_layout_info, None)
                    .map_err(|e| format!("Failed to create compute pipeline layout: {:?}", e))?
            };
            self.compute_pipeline_layout = Some(layout);
        }
        
        // Create compute pipeline
        let main_name = std::ffi::CStr::from_bytes_with_nul(b"main\0").unwrap();
        
        let stage_info = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(shader_module)
            .name(main_name);
        
        let pipeline_info = vk::ComputePipelineCreateInfo::default()
            .stage(stage_info)
            .layout(self.compute_pipeline_layout.unwrap());
        
        let cache = self.pipeline_cache_handle.unwrap_or(vk::PipelineCache::null());
        
        let pipeline = unsafe {
            let result = device.create_compute_pipelines(cache, &[pipeline_info], None)
                .map_err(|e| format!("Failed to create compute pipeline: {:?}", e.1))?;
            
            // Clean up shader module
            device.destroy_shader_module(shader_module, None);
            
            result[0]
        };
        
        // Store in cache
        self.compute_pipelines.insert(name.to_string(), pipeline);
        
        Ok(pipeline)
    }
    
    /// Get compute pipeline by name
    pub fn get_compute_pipeline(&self, name: &str) -> Option<vk::Pipeline> {
        self.compute_pipelines.get(name).copied()
    }
    
    /// Dispatch compute shader
    pub fn dispatch_compute(
        &self,
        pipeline_name: &str,
        groups_x: u32,
        groups_y: u32,
        groups_z: u32,
    ) -> Result<(), String> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        let cmd_buffer = self.current_cmd_buffer.ok_or("No active command buffer")?;
        let pipeline = self.compute_pipelines.get(pipeline_name)
            .ok_or("Compute pipeline not found")?;
        
        unsafe {
            device.cmd_bind_pipeline(cmd_buffer, vk::PipelineBindPoint::COMPUTE, *pipeline);
            device.cmd_dispatch(cmd_buffer, groups_x, groups_y, groups_z);
        }
        
        Ok(())
    }
    
    /// Create render pass with MSAA support
    pub fn create_msaa_render_pass(&mut self) -> Result<vk::RenderPass, String> {
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        
        // MSAA color attachment
        let msaa_color_attachment = vk::AttachmentDescription::default()
            .format(vk::Format::B8G8R8A8_UNORM)
            .samples(self.msaa_samples)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
        
        // Resolve attachment (non-MSAA)
        let resolve_attachment = vk::AttachmentDescription::default()
            .format(vk::Format::B8G8R8A8_UNORM)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::DONT_CARE)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);
        
        // MSAA depth attachment
        let msaa_depth_attachment = vk::AttachmentDescription::default()
            .format(vk::Format::D24_UNORM_S8_UINT)
            .samples(self.msaa_samples)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::CLEAR)
            .stencil_store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        
        let attachments = [msaa_color_attachment, resolve_attachment, msaa_depth_attachment];
        
        let color_attachment_ref = vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
        
        let resolve_attachment_ref = vk::AttachmentReference::default()
            .attachment(1)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
        
        let depth_attachment_ref = vk::AttachmentReference::default()
            .attachment(2)
            .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL);
        
        let color_refs = [color_attachment_ref];
        let resolve_refs = [resolve_attachment_ref];
        
        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_refs)
            .resolve_attachments(&resolve_refs)
            .depth_stencil_attachment(&depth_attachment_ref);
        
        let dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE);
        
        let render_pass_info = vk::RenderPassCreateInfo::default()
            .attachments(&attachments)
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(std::slice::from_ref(&dependency));
        
        unsafe {
            device.create_render_pass(&render_pass_info, None)
                .map_err(|e| format!("Failed to create MSAA render pass: {:?}", e))
        }
    }
    
    /// Create MSAA images for rendering
    pub fn create_msaa_images(&mut self) -> Result<(), String> {
        if self.msaa_samples == vk::SampleCountFlags::TYPE_1 {
            return Ok(()); // No MSAA needed
        }
        
        let device = self.device.as_ref().ok_or("Device not initialized")?;
        let allocator = self.allocator.as_ref().ok_or("Allocator not initialized")?;
        
        // Create MSAA color image
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::B8G8R8A8_UNORM)
            .extent(vk::Extent3D {
                width: self.width,
                height: self.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(self.msaa_samples)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let msaa_color_image = unsafe {
            device.create_image(&image_info, None)
                .map_err(|e| format!("Failed to create MSAA color image: {:?}", e))?
        };
        
        let requirements = unsafe { device.get_image_memory_requirements(msaa_color_image) };
        
        let alloc_desc = AllocationCreateDesc {
            name: "msaa_color_image",
            requirements,
            location: MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        };
        
        let allocation = allocator.lock().unwrap()
            .allocate(&alloc_desc)
            .map_err(|e| format!("Failed to allocate MSAA color image memory: {:?}", e))?;
        
        unsafe {
            device.bind_image_memory(msaa_color_image, allocation.memory(), allocation.offset())
                .map_err(|e| format!("Failed to bind MSAA color image memory: {:?}", e))?;
        }
        
        // Create image view
        let view_info = vk::ImageViewCreateInfo::default()
            .image(msaa_color_image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(vk::Format::B8G8R8A8_UNORM)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        
        let msaa_color_view = unsafe {
            device.create_image_view(&view_info, None)
                .map_err(|e| format!("Failed to create MSAA color image view: {:?}", e))?
        };
        
        self.msaa_color_images.push(msaa_color_image);
        self.msaa_color_image_views.push(msaa_color_view);
        self.msaa_color_allocations.push(allocation);
        
        // Create MSAA depth image
        let depth_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::D24_UNORM_S8_UINT)
            .extent(vk::Extent3D {
                width: self.width,
                height: self.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(self.msaa_samples)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::TRANSIENT_ATTACHMENT)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let msaa_depth_image = unsafe {
            device.create_image(&depth_info, None)
                .map_err(|e| format!("Failed to create MSAA depth image: {:?}", e))?
        };
        
        let depth_requirements = unsafe { device.get_image_memory_requirements(msaa_depth_image) };
        
        let depth_alloc_desc = AllocationCreateDesc {
            name: "msaa_depth_image",
            requirements: depth_requirements,
            location: MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        };
        
        let depth_allocation = allocator.lock().unwrap()
            .allocate(&depth_alloc_desc)
            .map_err(|e| format!("Failed to allocate MSAA depth image memory: {:?}", e))?;
        
        unsafe {
            device.bind_image_memory(msaa_depth_image, depth_allocation.memory(), depth_allocation.offset())
                .map_err(|e| format!("Failed to bind MSAA depth image memory: {:?}", e))?;
        }
        
        let depth_view_info = vk::ImageViewCreateInfo::default()
            .image(msaa_depth_image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(vk::Format::D24_UNORM_S8_UINT)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        
        let msaa_depth_view = unsafe {
            device.create_image_view(&depth_view_info, None)
                .map_err(|e| format!("Failed to create MSAA depth image view: {:?}", e))?
        };
        
        self.msaa_depth_image = Some(msaa_depth_image);
        self.msaa_depth_image_view = Some(msaa_depth_view);
        self.msaa_depth_allocation = Some(depth_allocation);
        
        Ok(())
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
    #[allow(dead_code)]
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

    /// Create descriptor set layout for texture samplers
    /// Matches the SPIR-V shader bindings: 16 combined image samplers at set 0
    fn create_descriptor_set_layout(device: &ash::Device) -> Result<vk::DescriptorSetLayout, String> {
        // Create bindings for 16 texture units
        let bindings: Vec<vk::DescriptorSetLayoutBinding> = (0..16)
            .map(|i| {
                vk::DescriptorSetLayoutBinding::default()
                    .binding(i)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .descriptor_count(1)
                    .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            })
            .collect();

        let layout_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(&bindings);

        unsafe {
            device
                .create_descriptor_set_layout(&layout_info, None)
                .map_err(|e| format!("Failed to create descriptor set layout: {:?}", e))
        }
    }

    /// Create descriptor pool for allocating descriptor sets
    fn create_descriptor_pool(device: &ash::Device, max_sets: u32) -> Result<vk::DescriptorPool, String> {
        // Pool size for combined image samplers (16 per set * max_sets)
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: 16 * max_sets,
            },
        ];

        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(max_sets);

        unsafe {
            device
                .create_descriptor_pool(&pool_info, None)
                .map_err(|e| format!("Failed to create descriptor pool: {:?}", e))
        }
    }

    /// Allocate descriptor sets for each frame in flight
    fn allocate_descriptor_sets(
        device: &ash::Device,
        pool: vk::DescriptorPool,
        layout: vk::DescriptorSetLayout,
        count: usize,
    ) -> Result<Vec<vk::DescriptorSet>, String> {
        let layouts: Vec<vk::DescriptorSetLayout> = vec![layout; count];

        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(pool)
            .set_layouts(&layouts);

        unsafe {
            device
                .allocate_descriptor_sets(&alloc_info)
                .map_err(|e| format!("Failed to allocate descriptor sets: {:?}", e))
        }
    }

    /// Create a texture sampler with the given configuration
    fn create_sampler(
        device: &ash::Device,
        min_filter: vk::Filter,
        mag_filter: vk::Filter,
        mipmap_mode: vk::SamplerMipmapMode,
        address_mode_u: vk::SamplerAddressMode,
        address_mode_v: vk::SamplerAddressMode,
        address_mode_w: vk::SamplerAddressMode,
        anisotropy: f32,
        max_anisotropy: f32,
    ) -> Result<vk::Sampler, String> {
        let enable_anisotropy = anisotropy > 1.0;
        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(mag_filter)
            .min_filter(min_filter)
            .mipmap_mode(mipmap_mode)
            .address_mode_u(address_mode_u)
            .address_mode_v(address_mode_v)
            .address_mode_w(address_mode_w)
            .anisotropy_enable(enable_anisotropy)
            .max_anisotropy(anisotropy.min(max_anisotropy))
            .compare_enable(false)
            .compare_op(vk::CompareOp::ALWAYS)
            .min_lod(0.0)
            .max_lod(vk::LOD_CLAMP_NONE)
            .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false);

        unsafe {
            device
                .create_sampler(&sampler_info, None)
                .map_err(|e| format!("Failed to create sampler: {:?}", e))
        }
    }

    /// Create default samplers for all 16 texture units
    fn create_default_samplers(device: &ash::Device, max_anisotropy: f32) -> Result<[Option<vk::Sampler>; 16], String> {
        let mut samplers: [Option<vk::Sampler>; 16] = Default::default();
        
        for (i, sampler) in samplers.iter_mut().enumerate() {
            *sampler = Some(Self::create_sampler(
                device,
                vk::Filter::LINEAR,
                vk::Filter::LINEAR,
                vk::SamplerMipmapMode::LINEAR,
                vk::SamplerAddressMode::REPEAT,
                vk::SamplerAddressMode::REPEAT,
                vk::SamplerAddressMode::REPEAT,
                1.0,
                max_anisotropy,
            ).map_err(|e| format!("Failed to create default sampler {}: {}", i, e))?);
        }

        Ok(samplers)
    }

    /// Create a placeholder texture image (1x1 white) for unbound texture slots
    fn create_placeholder_texture(
        device: &ash::Device,
        allocator: &Arc<Mutex<Allocator>>,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
    ) -> Result<(vk::Image, vk::ImageView, Allocation), String> {
        // Create 1x1 RGBA8 image
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_UNORM)
            .extent(vk::Extent3D { width: 1, height: 1, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let image = unsafe {
            device
                .create_image(&image_info, None)
                .map_err(|e| format!("Failed to create placeholder image: {:?}", e))?
        };

        let requirements = unsafe { device.get_image_memory_requirements(image) };

        let allocation = allocator
            .lock()
            .unwrap()
            .allocate(&AllocationCreateDesc {
                name: "placeholder_texture",
                requirements,
                location: MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })
            .map_err(|e| format!("Failed to allocate placeholder texture memory: {:?}", e))?;

        unsafe {
            device
                .bind_image_memory(image, allocation.memory(), allocation.offset())
                .map_err(|e| format!("Failed to bind placeholder image memory: {:?}", e))?;
        }

        // Transition to shader read layout
        Self::transition_image_layout(
            device,
            queue,
            command_pool,
            image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )?;

        // Create image view
        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_UNORM)
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
                .map_err(|e| format!("Failed to create placeholder image view: {:?}", e))?
        };

        Ok((image, view, allocation))
    }

    /// Transition image layout using a one-time command buffer
    fn transition_image_layout(
        device: &ash::Device,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        image: vk::Image,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
    ) -> Result<(), String> {
        // Allocate one-time command buffer
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let cmd_buffer = unsafe {
            device
                .allocate_command_buffers(&alloc_info)
                .map_err(|e| format!("Failed to allocate command buffer: {:?}", e))?
        }[0];

        // Begin recording
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            device
                .begin_command_buffer(cmd_buffer, &begin_info)
                .map_err(|e| format!("Failed to begin command buffer: {:?}", e))?;
        }

        // Determine access masks and pipeline stages based on layouts
        let (src_access, dst_access, src_stage, dst_stage) = match (old_layout, new_layout) {
            (vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL) => (
                vk::AccessFlags::empty(),
                vk::AccessFlags::TRANSFER_WRITE,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
            ),
            (vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
                vk::AccessFlags::TRANSFER_WRITE,
                vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
            ),
            (vk::ImageLayout::UNDEFINED, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
                vk::AccessFlags::empty(),
                vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
            ),
            // Transition from color attachment to transfer source for framebuffer readback
            (vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL) => (
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::AccessFlags::TRANSFER_READ,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::PipelineStageFlags::TRANSFER,
            ),
            // Transition back from transfer source to color attachment after readback
            (vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL) => (
                vk::AccessFlags::TRANSFER_READ,
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            ),
            // Transition from general layout to transfer source (for images that may be in general layout)
            (vk::ImageLayout::GENERAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL) => (
                vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
                vk::AccessFlags::TRANSFER_READ,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::PipelineStageFlags::TRANSFER,
            ),
            // Transition back to general layout after readback
            (vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::GENERAL) => (
                vk::AccessFlags::TRANSFER_READ,
                vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::ALL_COMMANDS,
            ),
            // Transition from shader read to transfer source
            (vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::ImageLayout::TRANSFER_SRC_OPTIMAL) => (
                vk::AccessFlags::SHADER_READ,
                vk::AccessFlags::TRANSFER_READ,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::PipelineStageFlags::TRANSFER,
            ),
            // Transition back from transfer source to shader read
            (vk::ImageLayout::TRANSFER_SRC_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
                vk::AccessFlags::TRANSFER_READ,
                vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
            ),
            _ => {
                return Err(format!(
                    "Unsupported layout transition: {:?} -> {:?}",
                    old_layout, new_layout
                ));
            }
        };

        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            })
            .src_access_mask(src_access)
            .dst_access_mask(dst_access);

        unsafe {
            device.cmd_pipeline_barrier(
                cmd_buffer,
                src_stage,
                dst_stage,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );

            device
                .end_command_buffer(cmd_buffer)
                .map_err(|e| format!("Failed to end command buffer: {:?}", e))?;

            // Submit and wait
            let submit_info = vk::SubmitInfo::default()
                .command_buffers(std::slice::from_ref(&cmd_buffer));

            device
                .queue_submit(queue, &[submit_info], vk::Fence::null())
                .map_err(|e| format!("Failed to submit command buffer: {:?}", e))?;

            device
                .queue_wait_idle(queue)
                .map_err(|e| format!("Failed to wait for queue: {:?}", e))?;

            // Free the command buffer
            device.free_command_buffers(command_pool, &[cmd_buffer]);
        }

        Ok(())
    }

    /// Upload texture data to a Vulkan image
    pub fn upload_texture(
        &mut self,
        slot: u32,
        texture: &crate::texture::Texture,
        data: &[u8],
    ) -> Result<(), String> {
        if slot >= 16 {
            return Err(format!("Invalid texture slot: {}", slot));
        }

        let slot = slot as usize;

        // Destroy existing texture at this slot if any (do this first before borrowing device)
        self.destroy_texture_at_slot(slot);

        let device = self.device.as_ref().ok_or("Device not initialized")?;
        let allocator = self.allocator.as_ref().ok_or("Allocator not initialized")?;
        let queue = self.graphics_queue.ok_or("Graphics queue not available")?;
        let command_pool = self.command_pool.ok_or("Command pool not available")?;

        // Determine Vulkan format from RSX format
        let vk_format = Self::rsx_format_to_vk(texture.format);

        // Create texture image
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk_format)
            .extent(vk::Extent3D {
                width: texture.width as u32,
                height: texture.height as u32,
                depth: 1,
            })
            .mip_levels(texture.mipmap_levels as u32)
            .array_layers(if texture.is_cubemap { 6 } else { 1 })
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let image = unsafe {
            device
                .create_image(&image_info, None)
                .map_err(|e| format!("Failed to create texture image: {:?}", e))?
        };

        let requirements = unsafe { device.get_image_memory_requirements(image) };

        let allocation = allocator
            .lock()
            .unwrap()
            .allocate(&AllocationCreateDesc {
                name: &format!("texture_{}", slot),
                requirements,
                location: MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })
            .map_err(|e| format!("Failed to allocate texture memory: {:?}", e))?;

        unsafe {
            device
                .bind_image_memory(image, allocation.memory(), allocation.offset())
                .map_err(|e| format!("Failed to bind texture image memory: {:?}", e))?;
        }

        // Create staging buffer for upload
        let staging_buffer_info = vk::BufferCreateInfo::default()
            .size(data.len() as u64)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let staging_buffer = unsafe {
            device
                .create_buffer(&staging_buffer_info, None)
                .map_err(|e| format!("Failed to create staging buffer: {:?}", e))?
        };

        let staging_requirements = unsafe { device.get_buffer_memory_requirements(staging_buffer) };

        let staging_allocation = allocator
            .lock()
            .unwrap()
            .allocate(&AllocationCreateDesc {
                name: "texture_staging",
                requirements: staging_requirements,
                location: MemoryLocation::CpuToGpu,
                linear: true,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })
            .map_err(|e| format!("Failed to allocate staging buffer memory: {:?}", e))?;

        unsafe {
            device
                .bind_buffer_memory(staging_buffer, staging_allocation.memory(), staging_allocation.offset())
                .map_err(|e| format!("Failed to bind staging buffer memory: {:?}", e))?;
        }

        // Copy data to staging buffer
        if let Some(mapped_ptr) = staging_allocation.mapped_ptr() {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    mapped_ptr.as_ptr() as *mut u8,
                    data.len(),
                );
            }
        } else {
            // Clean up and return error
            unsafe {
                device.destroy_buffer(staging_buffer, None);
                device.destroy_image(image, None);
            }
            allocator.lock().unwrap().free(staging_allocation).ok();
            allocator.lock().unwrap().free(allocation).ok();
            return Err("Staging buffer not mappable".to_string());
        }

        // Transition to transfer dst, copy, then transition to shader read
        Self::transition_image_layout(
            device,
            queue,
            command_pool,
            image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        )?;

        // Copy buffer to image
        Self::copy_buffer_to_image(
            device,
            queue,
            command_pool,
            staging_buffer,
            image,
            texture.width as u32,
            texture.height as u32,
        )?;

        Self::transition_image_layout(
            device,
            queue,
            command_pool,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )?;

        // Clean up staging buffer
        unsafe {
            device.destroy_buffer(staging_buffer, None);
        }
        allocator.lock().unwrap().free(staging_allocation).ok();

        // Create image view
        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(if texture.is_cubemap {
                vk::ImageViewType::CUBE
            } else {
                vk::ImageViewType::TYPE_2D
            })
            .format(vk_format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: texture.mipmap_levels as u32,
                base_array_layer: 0,
                layer_count: if texture.is_cubemap { 6 } else { 1 },
            });

        let view = unsafe {
            device
                .create_image_view(&view_info, None)
                .map_err(|e| format!("Failed to create texture image view: {:?}", e))?
        };

        // Store in slot
        self.texture_images[slot] = Some(image);
        self.texture_image_views[slot] = Some(view);
        self.texture_allocations[slot] = Some(allocation);
        self.texture_bound[slot] = false;

        Ok(())
    }

    /// Copy buffer data to image
    fn copy_buffer_to_image(
        device: &ash::Device,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        buffer: vk::Buffer,
        image: vk::Image,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        // Allocate one-time command buffer
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let cmd_buffer = unsafe {
            device
                .allocate_command_buffers(&alloc_info)
                .map_err(|e| format!("Failed to allocate command buffer: {:?}", e))?
        }[0];

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            device
                .begin_command_buffer(cmd_buffer, &begin_info)
                .map_err(|e| format!("Failed to begin command buffer: {:?}", e))?;

            let region = vk::BufferImageCopy::default()
                .buffer_offset(0)
                .buffer_row_length(0)
                .buffer_image_height(0)
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                .image_extent(vk::Extent3D { width, height, depth: 1 });

            device.cmd_copy_buffer_to_image(
                cmd_buffer,
                buffer,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            );

            device
                .end_command_buffer(cmd_buffer)
                .map_err(|e| format!("Failed to end command buffer: {:?}", e))?;

            let submit_info = vk::SubmitInfo::default()
                .command_buffers(std::slice::from_ref(&cmd_buffer));

            device
                .queue_submit(queue, &[submit_info], vk::Fence::null())
                .map_err(|e| format!("Failed to submit command buffer: {:?}", e))?;

            device
                .queue_wait_idle(queue)
                .map_err(|e| format!("Failed to wait for queue: {:?}", e))?;

            device.free_command_buffers(command_pool, &[cmd_buffer]);
        }

        Ok(())
    }

    /// Copy image to buffer (for framebuffer readback)
    fn copy_image_to_buffer(
        device: &ash::Device,
        queue: vk::Queue,
        command_pool: vk::CommandPool,
        image: vk::Image,
        buffer: vk::Buffer,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        // Allocate one-time command buffer
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let cmd_buffer = unsafe {
            device
                .allocate_command_buffers(&alloc_info)
                .map_err(|e| format!("Failed to allocate command buffer: {:?}", e))?
        }[0];

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            device
                .begin_command_buffer(cmd_buffer, &begin_info)
                .map_err(|e| format!("Failed to begin command buffer: {:?}", e))?;

            let region = vk::BufferImageCopy::default()
                .buffer_offset(0)
                .buffer_row_length(0)
                .buffer_image_height(0)
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                .image_extent(vk::Extent3D { width, height, depth: 1 });

            device.cmd_copy_image_to_buffer(
                cmd_buffer,
                image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                buffer,
                &[region],
            );

            device
                .end_command_buffer(cmd_buffer)
                .map_err(|e| format!("Failed to end command buffer: {:?}", e))?;

            let submit_info = vk::SubmitInfo::default()
                .command_buffers(std::slice::from_ref(&cmd_buffer));

            device
                .queue_submit(queue, &[submit_info], vk::Fence::null())
                .map_err(|e| format!("Failed to submit command buffer: {:?}", e))?;

            device
                .queue_wait_idle(queue)
                .map_err(|e| format!("Failed to wait for queue: {:?}", e))?;

            device.free_command_buffers(command_pool, &[cmd_buffer]);
        }

        Ok(())
    }

    /// Destroy texture resources at a slot
    fn destroy_texture_at_slot(&mut self, slot: usize) {
        if slot >= 16 {
            return;
        }

        if let Some(device) = &self.device {
            // Destroy image view
            if let Some(view) = self.texture_image_views[slot].take() {
                unsafe {
                    device.destroy_image_view(view, None);
                }
            }

            // Destroy image
            if let Some(image) = self.texture_images[slot].take() {
                unsafe {
                    device.destroy_image(image, None);
                }
            }

            // Free allocation
            if let Some(allocator) = &self.allocator {
                if let Some(allocation) = self.texture_allocations[slot].take() {
                    allocator.lock().unwrap().free(allocation).ok();
                }
            }

            self.texture_bound[slot] = false;
        }
    }

    /// Convert RSX texture format to Vulkan format
    fn rsx_format_to_vk(format: u8) -> vk::Format {
        use crate::texture::format::*;
        match format {
            B8 => vk::Format::R8_UNORM,
            A1R5G5B5 | R5G5B5A1 | D1R5G5B5 => vk::Format::A1R5G5B5_UNORM_PACK16,
            A4R4G4B4 => vk::Format::R4G4B4A4_UNORM_PACK16,
            R5G6B5 => vk::Format::R5G6B5_UNORM_PACK16,
            ARGB8 | A8R8G8B8 | D8R8G8B8 => vk::Format::B8G8R8A8_UNORM,
            XRGB8 => vk::Format::B8G8R8A8_UNORM,
            DXT1 => vk::Format::BC1_RGBA_UNORM_BLOCK,
            DXT3 => vk::Format::BC2_UNORM_BLOCK,
            DXT5 => vk::Format::BC3_UNORM_BLOCK,
            G8B8 => vk::Format::R8G8_UNORM,
            DEPTH24_D8 => vk::Format::D24_UNORM_S8_UINT,
            DEPTH16 => vk::Format::D16_UNORM,
            X16 => vk::Format::R16_UNORM,
            Y16_X16 => vk::Format::R16G16_UNORM,
            W16_Z16_Y16_X16_FLOAT => vk::Format::R16G16B16A16_SFLOAT,
            W32_Z32_Y32_X32_FLOAT => vk::Format::R32G32B32A32_SFLOAT,
            X32_FLOAT => vk::Format::R32_SFLOAT,
            Y16_X16_FLOAT => vk::Format::R16G16_SFLOAT,
            _ => vk::Format::R8G8B8A8_UNORM, // Default fallback
        }
    }

    /// Update descriptor set with bound textures
    fn update_descriptor_set(&self, set_index: usize) {
        if set_index >= self.descriptor_sets.len() {
            return;
        }

        let device = match &self.device {
            Some(d) => d,
            None => return,
        };

        let descriptor_set = self.descriptor_sets[set_index];
        
        // First, collect all valid texture bindings
        let mut bindings: Vec<(u32, vk::DescriptorImageInfo)> = Vec::with_capacity(16);
        
        for i in 0..16 {
            if let (Some(view), Some(sampler)) = (self.texture_image_views[i], self.texture_samplers[i]) {
                let image_info = vk::DescriptorImageInfo::default()
                    .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .image_view(view)
                    .sampler(sampler);
                bindings.push((i as u32, image_info));
            }
        }

        if bindings.is_empty() {
            return;
        }

        // Store image infos in a boxed array to ensure stable addresses
        let image_infos: Vec<[vk::DescriptorImageInfo; 1]> = bindings
            .iter()
            .map(|(_, info)| [*info])
            .collect();
        
        // Now build write descriptors referencing the stable storage
        let write_descriptors: Vec<vk::WriteDescriptorSet> = bindings
            .iter()
            .zip(image_infos.iter())
            .map(|((binding, _), info)| {
                vk::WriteDescriptorSet::default()
                    .dst_set(descriptor_set)
                    .dst_binding(*binding)
                    .dst_array_element(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .descriptor_count(1)
                    .image_info(info)
            })
            .collect();

        unsafe {
            device.update_descriptor_sets(&write_descriptors, &[]);
        }
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

    /// Create a Vulkan shader module from SPIR-V bytecode
    pub fn create_shader_module(&self, spirv: &[u32]) -> Result<vk::ShaderModule, String> {
        let device = self.device.as_ref()
            .ok_or("Device not initialized")?;

        let create_info = vk::ShaderModuleCreateInfo::default()
            .code(spirv);

        unsafe {
            device.create_shader_module(&create_info, None)
                .map_err(|e| format!("Failed to create shader module: {:?}", e))
        }
    }

    /// Compile RSX vertex program to Vulkan shader module
    pub fn compile_vertex_program(&mut self, program: &mut crate::shader::VertexProgram) -> Result<vk::ShaderModule, String> {
        // Translate to SPIR-V
        let spirv_module = self.shader_translator.translate_vertex(program)?;
        
        // Create Vulkan shader module
        let module = self.create_shader_module(&spirv_module.bytecode)?;
        
        // Store and return
        if let Some(old) = self.vertex_shader.take() {
            if let Some(device) = &self.device {
                unsafe { device.destroy_shader_module(old, None); }
            }
        }
        self.vertex_shader = Some(module);
        
        Ok(module)
    }

    /// Compile RSX fragment program to Vulkan shader module
    pub fn compile_fragment_program(&mut self, program: &mut crate::shader::FragmentProgram) -> Result<vk::ShaderModule, String> {
        // Translate to SPIR-V
        let spirv_module = self.shader_translator.translate_fragment(program)?;
        
        // Create Vulkan shader module
        let module = self.create_shader_module(&spirv_module.bytecode)?;
        
        // Store and return
        if let Some(old) = self.fragment_shader.take() {
            if let Some(device) = &self.device {
                unsafe { device.destroy_shader_module(old, None); }
            }
        }
        self.fragment_shader = Some(module);
        
        Ok(module)
    }

    /// Create graphics pipeline with current shaders
    pub fn create_graphics_pipeline(
        &mut self,
        topology: vk::PrimitiveTopology,
    ) -> Result<vk::Pipeline, String> {
        let device = self.device.as_ref()
            .ok_or("Device not initialized")?;
        let render_pass = self.render_pass
            .ok_or("Render pass not created")?;

        // Get shader modules
        let vs = self.vertex_shader
            .ok_or("Vertex shader not compiled")?;
        let fs = self.fragment_shader
            .ok_or("Fragment shader not compiled")?;

        // Shader stage info
        let main_name = std::ffi::CStr::from_bytes_with_nul(b"main\0").unwrap();
        
        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vs)
                .name(main_name),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(fs)
                .name(main_name),
        ];

        // Vertex input state
        let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(&self.vertex_bindings)
            .vertex_attribute_descriptions(&self.vertex_attributes);

        // Input assembly
        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(topology)
            .primitive_restart_enable(false);

        // Viewport and scissor (dynamic)
        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: self.width as f32,
            height: self.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk::Extent2D { width: self.width, height: self.height },
        };
        let viewports = [viewport];
        let scissors = [scissor];
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(&viewports)
            .scissors(&scissors);

        // Rasterization
        let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::CLOCKWISE)
            .depth_bias_enable(false);

        // Multisampling
        let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
            .sample_shading_enable(false)
            .rasterization_samples(self.msaa_samples);

        // Depth stencil
        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);

        // Color blending
        let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(true)
            .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
            .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD);
        let color_blend_attachments = [color_blend_attachment];
        let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op_enable(false)
            .attachments(&color_blend_attachments);

        // Create pipeline layout if needed (includes descriptor set layout for textures)
        if self.pipeline_layout.is_none() {
            let set_layouts = if let Some(desc_layout) = self.descriptor_set_layout {
                vec![desc_layout]
            } else {
                vec![]
            };
            
            let layout_info = vk::PipelineLayoutCreateInfo::default()
                .set_layouts(&set_layouts);
            
            self.pipeline_layout = Some(unsafe {
                device.create_pipeline_layout(&layout_info, None)
                    .map_err(|e| format!("Failed to create pipeline layout: {:?}", e))?
            });
        }
        let pipeline_layout = self.pipeline_layout.unwrap();

        // Create pipeline
        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input_info)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
            .depth_stencil_state(&depth_stencil)
            .color_blend_state(&color_blending)
            .layout(pipeline_layout)
            .render_pass(render_pass)
            .subpass(0);

        let pipeline = unsafe {
            device.create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                .map_err(|e| format!("Failed to create graphics pipeline: {:?}", e.1))?
        };

        // Destroy old pipeline
        if let Some(old) = self.pipeline.take() {
            unsafe { device.destroy_pipeline(old, None); }
        }
        self.pipeline = Some(pipeline[0]);

        Ok(pipeline[0])
    }

    /// Bind current pipeline for rendering
    pub fn bind_pipeline(&self) {
        if let (Some(cmd_buffer), Some(pipeline)) = (self.current_cmd_buffer, self.pipeline) {
            if let Some(device) = &self.device {
                unsafe {
                    device.cmd_bind_pipeline(cmd_buffer, vk::PipelineBindPoint::GRAPHICS, pipeline);
                }
                
                // Also bind descriptor sets for textures
                self.bind_descriptor_set_impl();
            }
        }
    }

    /// Bind the current descriptor set during rendering (inherent impl)
    fn bind_descriptor_set_impl(&self) {
        if !self.initialized {
            return;
        }

        if let (Some(device), Some(cmd_buffer), Some(layout)) = 
            (&self.device, self.current_cmd_buffer, self.pipeline_layout) 
        {
            if self.descriptor_sets.len() > self.current_frame {
                let descriptor_set = self.descriptor_sets[self.current_frame];
                unsafe {
                    device.cmd_bind_descriptor_sets(
                        cmd_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        layout,
                        0,
                        &[descriptor_set],
                        &[],
                    );
                }
            }
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

        // Create descriptor set layout for texture samplers
        let descriptor_set_layout = Self::create_descriptor_set_layout(&device)?;

        // Create descriptor pool
        let descriptor_pool = Self::create_descriptor_pool(&device, self.max_frames_in_flight as u32)?;

        // Allocate descriptor sets
        let descriptor_sets = Self::allocate_descriptor_sets(
            &device,
            descriptor_pool,
            descriptor_set_layout,
            self.max_frames_in_flight,
        )?;

        // Create default samplers for all texture units
        let texture_samplers = Self::create_default_samplers(&device, self.max_anisotropy)?;

        // Create placeholder textures for all 16 slots
        let mut texture_images: [Option<vk::Image>; 16] = Default::default();
        let mut texture_image_views: [Option<vk::ImageView>; 16] = Default::default();
        let mut texture_allocations: [Option<Allocation>; 16] = Default::default();

        // Create one placeholder and use it for all slots
        let (placeholder_image, placeholder_view, placeholder_allocation) =
            Self::create_placeholder_texture(&device, &allocator, graphics_queue, command_pool)?;

        // For slot 0, use the actual placeholder; others will be None until textures are uploaded
        texture_images[0] = Some(placeholder_image);
        texture_image_views[0] = Some(placeholder_view);
        texture_allocations[0] = Some(placeholder_allocation);

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
        self.descriptor_set_layout = Some(descriptor_set_layout);
        self.descriptor_pool = Some(descriptor_pool);
        self.descriptor_sets = descriptor_sets;
        self.texture_images = texture_images;
        self.texture_image_views = texture_image_views;
        self.texture_allocations = texture_allocations;
        self.texture_samplers = texture_samplers;
        self.allocator = Some(allocator);
        self.initialized = true;

        // Initialize descriptor sets with placeholder textures
        for i in 0..self.max_frames_in_flight {
            self.update_descriptor_set(i);
        }

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

                // Destroy texture resources
                for i in 0..16 {
                    if let Some(view) = self.texture_image_views[i].take() {
                        device.destroy_image_view(view, None);
                    }
                    if let Some(image) = self.texture_images[i].take() {
                        device.destroy_image(image, None);
                    }
                    if let Some(sampler) = self.texture_samplers[i].take() {
                        device.destroy_sampler(sampler, None);
                    }
                    if let Some(allocator) = &self.allocator {
                        if let Some(allocation) = self.texture_allocations[i].take() {
                            allocator.lock().unwrap().free(allocation).ok();
                        }
                    }
                }

                // Destroy descriptor pool (this also frees descriptor sets)
                if let Some(pool) = self.descriptor_pool.take() {
                    device.destroy_descriptor_pool(pool, None);
                }
                self.descriptor_sets.clear();

                if let Some(layout) = self.descriptor_set_layout.take() {
                    device.destroy_descriptor_set_layout(layout, None);
                }

                // Destroy shader modules
                if let Some(vs) = self.vertex_shader.take() {
                    device.destroy_shader_module(vs, None);
                }
                if let Some(fs) = self.fragment_shader.take() {
                    device.destroy_shader_module(fs, None);
                }

                // Destroy vertex buffers
                if let Some(allocator) = &self.allocator {
                    for (_, buffer, alloc, _) in self.vertex_buffers.drain(..) {
                        if let Some(allocation) = alloc {
                            allocator.lock().unwrap().free(allocation).ok();
                        }
                        device.destroy_buffer(buffer, None);
                    }
                }

                // Destroy index buffer
                if let Some((buffer, alloc, _, _)) = self.index_buffer.take() {
                    if let Some(allocator) = &self.allocator {
                        if let Some(allocation) = alloc {
                            allocator.lock().unwrap().free(allocation).ok();
                        }
                    }
                    device.destroy_buffer(buffer, None);
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

        if slot >= 16 {
            tracing::warn!("Invalid texture slot: {}", slot);
            return;
        }

        tracing::trace!("Bind texture: slot={}, offset=0x{:08x}", slot, offset);

        let slot = slot as usize;

        // Mark this slot as bound (texture should have been uploaded via upload_texture)
        if self.texture_image_views[slot].is_some() {
            self.texture_bound[slot] = true;

            // Update descriptor sets for all frames in flight
            for i in 0..self.max_frames_in_flight {
                self.update_descriptor_set(i);
            }
        } else {
            tracing::warn!("Texture at slot {} not uploaded, binding skipped", slot);
        }
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
    
    fn submit_vertex_buffer(&mut self, binding: u32, data: &[u8], stride: u32) {
        if !self.initialized || data.is_empty() {
            return;
        }

        tracing::trace!(
            "Submit vertex buffer: binding={}, size={}, stride={}",
            binding, data.len(), stride
        );

        let device = match &self.device {
            Some(d) => d,
            None => return,
        };

        let allocator = match &self.allocator {
            Some(a) => a,
            None => return,
        };

        // Create or reuse a vertex buffer for this binding
        let buffer_size = data.len() as u64;

        // Remove old buffer for this binding if it exists
        // Find and remove old buffers for this binding
        let mut i = 0;
        while i < self.vertex_buffers.len() {
            if self.vertex_buffers[i].0 == binding {
                let (_, old_buf, old_alloc, _) = self.vertex_buffers.remove(i);
                if let Some(allocation) = old_alloc {
                    let _ = allocator.lock().unwrap().free(allocation);
                }
                unsafe { device.destroy_buffer(old_buf, None); }
            } else {
                i += 1;
            }
        }

        // Create new vertex buffer with CPU-visible memory for direct upload
        let buffer_info = vk::BufferCreateInfo::default()
            .size(buffer_size)
            .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = match unsafe { device.create_buffer(&buffer_info, None) } {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("Failed to create vertex buffer: {:?}", e);
                return;
            }
        };

        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

        let allocation_desc = AllocationCreateDesc {
            name: "vertex_buffer",
            requirements,
            location: MemoryLocation::CpuToGpu,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        };

        let allocation = match allocator.lock().unwrap().allocate(&allocation_desc) {
            Ok(a) => a,
            Err(e) => {
                tracing::error!("Failed to allocate vertex buffer memory: {:?}", e);
                unsafe { device.destroy_buffer(buffer, None); }
                return;
            }
        };

        // Bind buffer memory
        if let Err(e) = unsafe {
            device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
        } {
            tracing::error!("Failed to bind vertex buffer memory: {:?}", e);
            let _ = allocator.lock().unwrap().free(allocation);
            unsafe { device.destroy_buffer(buffer, None); }
            return;
        }

        // Copy data to buffer
        if let Some(mapped_ptr) = allocation.mapped_ptr() {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    mapped_ptr.as_ptr() as *mut u8,
                    data.len(),
                );
            }
        } else {
            tracing::error!("Vertex buffer memory not mapped");
            let _ = allocator.lock().unwrap().free(allocation);
            unsafe { device.destroy_buffer(buffer, None); }
            return;
        }

        // Store the buffer
        self.vertex_buffers.push((binding, buffer, Some(allocation), buffer_size));

        // Bind the vertex buffer to the command buffer
        if let Some(cmd_buffer) = self.current_cmd_buffer {
            unsafe {
                device.cmd_bind_vertex_buffers(cmd_buffer, binding, &[buffer], &[0]);
            }
        }

        tracing::trace!("Vertex buffer submitted and bound for binding {}", binding);
    }
    
    fn submit_index_buffer(&mut self, data: &[u8], index_type: u32) {
        if !self.initialized || data.is_empty() {
            return;
        }

        tracing::trace!(
            "Submit index buffer: size={}, index_type={}",
            data.len(), index_type
        );

        let device = match &self.device {
            Some(d) => d,
            None => return,
        };

        let allocator = match &self.allocator {
            Some(a) => a,
            None => return,
        };

        let vk_index_type = match index_type {
            2 => vk::IndexType::UINT16,
            4 => vk::IndexType::UINT32,
            _ => {
                tracing::warn!("Unknown index type {}, defaulting to UINT16", index_type);
                vk::IndexType::UINT16
            }
        };

        // Remove old index buffer if it exists
        if let Some((buf, alloc, _, _)) = self.index_buffer.take() {
            if let Some(allocation) = alloc {
                let _ = allocator.lock().unwrap().free(allocation);
            }
            unsafe { device.destroy_buffer(buf, None); }
        }

        let buffer_size = data.len() as u64;

        // Create new index buffer with CPU-visible memory for direct upload
        let buffer_info = vk::BufferCreateInfo::default()
            .size(buffer_size)
            .usage(vk::BufferUsageFlags::INDEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let buffer = match unsafe { device.create_buffer(&buffer_info, None) } {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("Failed to create index buffer: {:?}", e);
                return;
            }
        };

        let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };

        let allocation_desc = AllocationCreateDesc {
            name: "index_buffer",
            requirements,
            location: MemoryLocation::CpuToGpu,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        };

        let allocation = match allocator.lock().unwrap().allocate(&allocation_desc) {
            Ok(a) => a,
            Err(e) => {
                tracing::error!("Failed to allocate index buffer memory: {:?}", e);
                unsafe { device.destroy_buffer(buffer, None); }
                return;
            }
        };

        // Bind buffer memory
        if let Err(e) = unsafe {
            device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())
        } {
            tracing::error!("Failed to bind index buffer memory: {:?}", e);
            let _ = allocator.lock().unwrap().free(allocation);
            unsafe { device.destroy_buffer(buffer, None); }
            return;
        }

        // Copy data to buffer
        if let Some(mapped_ptr) = allocation.mapped_ptr() {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    mapped_ptr.as_ptr() as *mut u8,
                    data.len(),
                );
            }
        } else {
            tracing::error!("Index buffer memory not mapped");
            let _ = allocator.lock().unwrap().free(allocation);
            unsafe { device.destroy_buffer(buffer, None); }
            return;
        }

        // Store the buffer
        self.index_buffer = Some((buffer, Some(allocation), buffer_size, vk_index_type));

        // Bind the index buffer to the command buffer
        if let Some(cmd_buffer) = self.current_cmd_buffer {
            unsafe {
                device.cmd_bind_index_buffer(cmd_buffer, buffer, 0, vk_index_type);
            }
        }

        tracing::trace!("Index buffer submitted and bound");
    }
    
    fn get_framebuffer(&self) -> Option<super::FramebufferData> {
        /// RGBA format uses 4 bytes per pixel
        const BYTES_PER_PIXEL: u32 = 4;
        
        if !self.initialized {
            return None;
        }
        
        // Get required resources
        let device = self.device.as_ref()?;
        let queue = self.graphics_queue?;
        let command_pool = self.command_pool?;
        let allocator = self.allocator.as_ref()?;
        
        // Get the render image to read from
        // Use the current frame's render image
        let render_image = if !self.render_images.is_empty() {
            self.render_images[self.current_frame % self.render_images.len()]
        } else {
            tracing::warn!("No render images available for framebuffer readback");
            return Some(super::FramebufferData::test_pattern(self.width, self.height));
        };
        
        // Calculate buffer size (RGBA, 4 bytes per pixel)
        let buffer_size = (self.width * self.height * BYTES_PER_PIXEL) as u64;
        let buffer_size_bytes = buffer_size as usize;
        
        // Create staging buffer for readback (CPU-readable)
        let staging_buffer_info = vk::BufferCreateInfo::default()
            .size(buffer_size)
            .usage(vk::BufferUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let staging_buffer = match unsafe { device.create_buffer(&staging_buffer_info, None) } {
            Ok(buffer) => buffer,
            Err(e) => {
                tracing::error!("Failed to create staging buffer for readback: {:?}", e);
                return Some(super::FramebufferData::test_pattern(self.width, self.height));
            }
        };
        
        let staging_requirements = unsafe { device.get_buffer_memory_requirements(staging_buffer) };
        
        // Allocate CPU-readable memory for the staging buffer
        let alloc_desc = AllocationCreateDesc {
            name: "framebuffer_readback_staging",
            requirements: staging_requirements,
            location: MemoryLocation::GpuToCpu,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        };
        
        let staging_allocation = match allocator.lock().unwrap().allocate(&alloc_desc) {
            Ok(alloc) => alloc,
            Err(e) => {
                tracing::error!("Failed to allocate staging buffer memory: {:?}", e);
                unsafe { device.destroy_buffer(staging_buffer, None); }
                return Some(super::FramebufferData::test_pattern(self.width, self.height));
            }
        };
        
        if let Err(e) = unsafe {
            device.bind_buffer_memory(staging_buffer, staging_allocation.memory(), staging_allocation.offset())
        } {
            tracing::error!("Failed to bind staging buffer memory: {:?}", e);
            unsafe { device.destroy_buffer(staging_buffer, None); }
            allocator.lock().unwrap().free(staging_allocation).ok();
            return Some(super::FramebufferData::test_pattern(self.width, self.height));
        }
        
        // Transition render image to transfer source layout
        if let Err(e) = Self::transition_image_layout(
            device,
            queue,
            command_pool,
            render_image,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        ) {
            tracing::error!("Failed to transition image for readback: {}", e);
            unsafe { device.destroy_buffer(staging_buffer, None); }
            allocator.lock().unwrap().free(staging_allocation).ok();
            return Some(super::FramebufferData::test_pattern(self.width, self.height));
        }
        
        // Copy image to buffer
        if let Err(e) = Self::copy_image_to_buffer(
            device,
            queue,
            command_pool,
            render_image,
            staging_buffer,
            self.width,
            self.height,
        ) {
            tracing::error!("Failed to copy image to buffer: {}", e);
            // Try to transition back even on error
            let _ = Self::transition_image_layout(
                device,
                queue,
                command_pool,
                render_image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );
            unsafe { device.destroy_buffer(staging_buffer, None); }
            allocator.lock().unwrap().free(staging_allocation).ok();
            return Some(super::FramebufferData::test_pattern(self.width, self.height));
        }
        
        // Transition render image back to color attachment layout
        if let Err(e) = Self::transition_image_layout(
            device,
            queue,
            command_pool,
            render_image,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        ) {
            tracing::error!("Failed to transition image back after readback: {}", e);
            // Continue anyway, we have the data
        }
        
        // Read the pixel data from the staging buffer
        let pixels = if let Some(mapped_ptr) = staging_allocation.mapped_ptr() {
            let mut pixels = vec![0u8; buffer_size_bytes];
            unsafe {
                std::ptr::copy_nonoverlapping(
                    mapped_ptr.as_ptr() as *const u8,
                    pixels.as_mut_ptr(),
                    buffer_size_bytes,
                );
            }
            pixels
        } else {
            tracing::error!("Staging buffer not mappable for readback");
            unsafe { device.destroy_buffer(staging_buffer, None); }
            allocator.lock().unwrap().free(staging_allocation).ok();
            return Some(super::FramebufferData::test_pattern(self.width, self.height));
        };
        
        // Clean up staging buffer
        unsafe { device.destroy_buffer(staging_buffer, None); }
        allocator.lock().unwrap().free(staging_allocation).ok();
        
        Some(super::FramebufferData {
            width: self.width,
            height: self.height,
            pixels,
        })
    }
    
    fn get_dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// Render-to-texture (RTT) management.
/// Tracks framebuffers bound to texture offsets so the GPU can render
/// directly into a texture that is later sampled by a shader.
#[derive(Debug)]
pub struct RenderToTexture {
    /// Map from texture offset → framebuffer handle index
    bindings: std::collections::HashMap<u32, u32>,
    /// Whether RTT is currently active
    active: bool,
    /// Currently bound RTT target offset
    current_target: u32,
    /// Resolution of the current RTT surface
    current_width: u32,
    current_height: u32,
}

impl RenderToTexture {
    pub fn new() -> Self {
        Self {
            bindings: std::collections::HashMap::new(),
            active: false,
            current_target: 0,
            current_width: 0,
            current_height: 0,
        }
    }

    /// Begin rendering to a texture at the given offset.
    /// Returns `true` if the target was successfully bound, `false` if already active.
    pub fn begin_rtt(&mut self, texture_offset: u32, width: u32, height: u32) -> bool {
        if self.active {
            return false;
        }
        self.active = true;
        self.current_target = texture_offset;
        self.current_width = width;
        self.current_height = height;
        // Assign a framebuffer index
        let fb_idx = self.bindings.len() as u32;
        self.bindings.insert(texture_offset, fb_idx);
        true
    }

    /// End rendering to the current RTT target.
    /// Returns the texture offset that was being rendered to, or 0 if not active.
    pub fn end_rtt(&mut self) -> u32 {
        if !self.active {
            return 0;
        }
        self.active = false;
        let target = self.current_target;
        self.current_target = 0;
        self.current_width = 0;
        self.current_height = 0;
        target
    }

    /// Check if a texture offset has an associated RTT framebuffer.
    pub fn has_rtt_binding(&self, texture_offset: u32) -> bool {
        self.bindings.contains_key(&texture_offset)
    }

    /// Get the framebuffer index for a texture offset.
    pub fn get_rtt_framebuffer(&self, texture_offset: u32) -> Option<u32> {
        self.bindings.get(&texture_offset).copied()
    }

    /// Whether RTT is currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get current target dimensions (returns (0,0) if not active).
    pub fn current_dimensions(&self) -> (u32, u32) {
        if self.active {
            (self.current_width, self.current_height)
        } else {
            (0, 0)
        }
    }

    /// Remove an RTT binding.
    pub fn remove_binding(&mut self, texture_offset: u32) -> bool {
        self.bindings.remove(&texture_offset).is_some()
    }

    /// Get total number of RTT bindings.
    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }
}

impl Default for RenderToTexture {
    fn default() -> Self {
        Self::new()
    }
}

/// Framebuffer copy/blit operations.
/// Handles copying pixel data between framebuffers, including format conversion
/// and scaling via blit.
#[derive(Debug)]
pub struct FramebufferCopier {
    /// Pending copy operations
    pending_copies: Vec<FramebufferCopy>,
    /// Statistics
    total_copies: u64,
    total_bytes: u64,
}

/// A single framebuffer copy operation descriptor.
#[derive(Debug, Clone)]
pub struct FramebufferCopy {
    /// Source framebuffer offset
    pub src_offset: u32,
    /// Destination framebuffer offset
    pub dst_offset: u32,
    /// Source rectangle (x, y, width, height)
    pub src_rect: (u32, u32, u32, u32),
    /// Destination rectangle (x, y, width, height)
    pub dst_rect: (u32, u32, u32, u32),
    /// Source format (bytes per pixel)
    pub src_bpp: u32,
    /// Destination format (bytes per pixel)
    pub dst_bpp: u32,
}

impl FramebufferCopier {
    pub fn new() -> Self {
        Self {
            pending_copies: Vec::new(),
            total_copies: 0,
            total_bytes: 0,
        }
    }

    /// Queue a framebuffer copy operation.
    pub fn queue_copy(&mut self, copy: FramebufferCopy) {
        self.pending_copies.push(copy);
    }

    /// Execute all pending copy operations on a linear framebuffer.
    /// Returns the number of copies performed.
    pub fn execute_copies(&mut self, fb_data: &mut [u8], fb_pitch: u32) -> usize {
        let count = self.pending_copies.len();
        
        for copy in self.pending_copies.drain(..) {
            let (sx, sy, sw, sh) = copy.src_rect;
            let (dx, dy, dw, dh) = copy.dst_rect;
            
            // Simple 1:1 copy (no scaling) when dimensions match
            if sw == dw && sh == dh && copy.src_bpp == copy.dst_bpp {
                let bpp = copy.src_bpp;
                for row in 0..sh {
                    let src_row_offset = ((sy + row) * fb_pitch + sx * bpp) as usize;
                    let dst_row_offset = ((dy + row) * fb_pitch + dx * bpp) as usize;
                    let row_bytes = (sw * bpp) as usize;
                    
                    // Check bounds and ensure source and destination don't overlap
                    if src_row_offset + row_bytes <= fb_data.len() 
                        && dst_row_offset + row_bytes <= fb_data.len()
                        && (src_row_offset + row_bytes <= dst_row_offset
                            || dst_row_offset + row_bytes <= src_row_offset)
                    {
                        let (left, right) = fb_data.split_at_mut(dst_row_offset);
                        let src_slice = &left[src_row_offset..src_row_offset + row_bytes];
                        right[..row_bytes].copy_from_slice(src_slice);
                    }
                }
                self.total_bytes += (sw * sh * bpp) as u64;
            }
            // Scaled copy using nearest-neighbor sampling
            else if dw > 0 && dh > 0 {
                let src_bpp = copy.src_bpp;
                // When formats differ, copy the minimum of src/dst bpp bytes per pixel
                // (truncating extra channels rather than converting formats)
                let dst_bpp = copy.dst_bpp.min(copy.src_bpp);
                
                // Read source pixels into a temporary row buffer to avoid
                // aliasing issues when source and destination overlap
                let row_buf_size = (dw * dst_bpp) as usize;
                let mut row_buf = vec![0u8; row_buf_size];
                
                for dy_row in 0..dh {
                    let src_row = sy + (dy_row * sh / dh);
                    
                    // Read source pixels into temporary buffer
                    for dx_col in 0..dw {
                        let src_col = sx + (dx_col * sw / dw);
                        let src_off = ((src_row * fb_pitch) + src_col * src_bpp) as usize;
                        let buf_off = (dx_col * dst_bpp) as usize;
                        let copy_bytes = dst_bpp as usize;
                        
                        if src_off + copy_bytes <= fb_data.len() && buf_off + copy_bytes <= row_buf.len() {
                            row_buf[buf_off..buf_off + copy_bytes]
                                .copy_from_slice(&fb_data[src_off..src_off + copy_bytes]);
                        }
                    }
                    
                    // Write from buffer to destination
                    for dx_col in 0..dw {
                        let dst_off = (((dy + dy_row) * fb_pitch) + (dx + dx_col) * dst_bpp) as usize;
                        let buf_off = (dx_col * dst_bpp) as usize;
                        let copy_bytes = dst_bpp as usize;
                        
                        if dst_off + copy_bytes <= fb_data.len() && buf_off + copy_bytes <= row_buf.len() {
                            fb_data[dst_off..dst_off + copy_bytes]
                                .copy_from_slice(&row_buf[buf_off..buf_off + copy_bytes]);
                        }
                    }
                }
                self.total_bytes += (dw * dh * dst_bpp) as u64;
            }
            
            self.total_copies += 1;
        }
        
        count
    }

    /// Get the number of pending copy operations.
    pub fn pending_count(&self) -> usize {
        self.pending_copies.len()
    }

    /// Get total copies executed.
    pub fn total_copies(&self) -> u64 {
        self.total_copies
    }

    /// Get total bytes copied.
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }
}

impl Default for FramebufferCopier {
    fn default() -> Self {
        Self::new()
    }
}

/// MSAA (Multi-Sample Anti-Aliasing) resolver.
/// Handles resolving multi-sampled render targets to single-sampled textures.
#[derive(Debug)]
pub struct MsaaResolver {
    /// Current sample count (1, 2, 4, or 8)
    sample_count: u8,
    /// Sample positions for custom patterns (normalized 0.0-1.0)
    sample_positions: Vec<(f32, f32)>,
    /// Whether MSAA is currently enabled
    enabled: bool,
    /// Total resolves performed
    resolve_count: u64,
}

impl MsaaResolver {
    pub fn new() -> Self {
        Self {
            sample_count: 1,
            sample_positions: Vec::new(),
            enabled: false,
            resolve_count: 0,
        }
    }

    /// Configure MSAA with the given sample count.
    /// Valid counts are 1, 2, 4, and 8.
    pub fn configure(&mut self, sample_count: u8, enabled: bool) {
        self.sample_count = match sample_count {
            1 | 2 | 4 | 8 => sample_count,
            _ => 1,
        };
        self.enabled = enabled;
        self.sample_positions = Self::default_positions(self.sample_count);
    }

    /// Get the current sample count.
    pub fn sample_count(&self) -> u8 {
        self.sample_count
    }

    /// Whether MSAA is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled && self.sample_count > 1
    }

    /// Resolve multi-sampled color data to single-sampled by box-filter averaging.
    /// `src` contains `sample_count` samples per pixel in interleaved order.
    /// `width` × `height` is the resolved output size.
    /// Returns the resolved pixel data (4 bytes per pixel: RGBA).
    pub fn resolve_color(&mut self, src: &[u8], width: u32, height: u32) -> Vec<u8> {
        let pixel_count = (width * height) as usize;
        let samples = self.sample_count as usize;
        let mut dst = vec![0u8; pixel_count * 4];
        
        if samples <= 1 || src.len() < pixel_count * samples * 4 {
            // No MSAA or insufficient data — copy directly
            let copy_len = dst.len().min(src.len());
            dst[..copy_len].copy_from_slice(&src[..copy_len]);
            self.resolve_count += 1;
            return dst;
        }
        
        // Box filter: average all samples for each pixel
        for pixel in 0..pixel_count {
            let mut r: u32 = 0;
            let mut g: u32 = 0;
            let mut b: u32 = 0;
            let mut a: u32 = 0;
            
            for s in 0..samples {
                let src_idx = (pixel * samples + s) * 4;
                if src_idx + 3 < src.len() {
                    r += src[src_idx] as u32;
                    g += src[src_idx + 1] as u32;
                    b += src[src_idx + 2] as u32;
                    a += src[src_idx + 3] as u32;
                }
            }
            
            let dst_idx = pixel * 4;
            dst[dst_idx] = (r / samples as u32) as u8;
            dst[dst_idx + 1] = (g / samples as u32) as u8;
            dst[dst_idx + 2] = (b / samples as u32) as u8;
            dst[dst_idx + 3] = (a / samples as u32) as u8;
        }
        
        self.resolve_count += 1;
        dst
    }

    /// Get total number of resolve operations performed.
    pub fn resolve_count(&self) -> u64 {
        self.resolve_count
    }

    /// Generate default sample positions for the given sample count.
    fn default_positions(count: u8) -> Vec<(f32, f32)> {
        match count {
            2 => vec![(0.25, 0.25), (0.75, 0.75)],
            4 => vec![
                (0.375, 0.125), (0.875, 0.375),
                (0.125, 0.625), (0.625, 0.875),
            ],
            8 => vec![
                (0.5625, 0.3125), (0.4375, 0.6875),
                (0.8125, 0.5625), (0.3125, 0.1875),
                (0.1875, 0.8125), (0.0625, 0.4375),
                (0.6875, 0.9375), (0.9375, 0.0625),
            ],
            _ => vec![(0.5, 0.5)], // 1 sample = center
        }
    }

    /// Get current sample positions.
    pub fn sample_positions(&self) -> &[(f32, f32)] {
        &self.sample_positions
    }

    /// Set custom sample positions.
    pub fn set_sample_positions(&mut self, positions: Vec<(f32, f32)>) {
        self.sample_positions = positions;
    }
}

impl Default for MsaaResolver {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn test_vulkan_backend_msaa() {
        let backend = VulkanBackend::with_msaa(2, 4);
        assert_eq!(backend.msaa_sample_count(), 4);
        assert_eq!(backend.max_frames_in_flight, 2);
    }

    #[test]
    fn test_vulkan_backend_msaa_set() {
        let mut backend = VulkanBackend::new();
        assert_eq!(backend.msaa_sample_count(), 1);
        
        backend.set_msaa_samples(8);
        assert_eq!(backend.msaa_sample_count(), 8);
        
        backend.set_msaa_samples(16);
        assert_eq!(backend.msaa_sample_count(), 16);
    }

    #[test]
    fn test_vulkan_backend_mrt() {
        let mut backend = VulkanBackend::new();
        assert_eq!(backend.active_mrt_count(), 1);
        
        backend.set_active_mrt_count(4);
        assert_eq!(backend.active_mrt_count(), 4);
        
        // Should clamp to max 4
        backend.set_active_mrt_count(8);
        assert_eq!(backend.active_mrt_count(), 4);
    }

    #[test]
    fn test_vulkan_backend_anisotropy() {
        let mut backend = VulkanBackend::new();
        assert_eq!(backend.anisotropy_level(), 1.0);
        
        backend.set_anisotropy_level(8.0);
        assert_eq!(backend.anisotropy_level(), 8.0);
        
        backend.set_anisotropy_level(16.0);
        assert_eq!(backend.anisotropy_level(), 16.0);
    }

    #[test]
    fn test_sample_count_to_flags() {
        assert_eq!(VulkanBackend::sample_count_to_flags(1), vk::SampleCountFlags::TYPE_1);
        assert_eq!(VulkanBackend::sample_count_to_flags(2), vk::SampleCountFlags::TYPE_2);
        assert_eq!(VulkanBackend::sample_count_to_flags(4), vk::SampleCountFlags::TYPE_4);
        assert_eq!(VulkanBackend::sample_count_to_flags(8), vk::SampleCountFlags::TYPE_8);
        assert_eq!(VulkanBackend::sample_count_to_flags(99), vk::SampleCountFlags::TYPE_1); // Invalid defaults to 1
    }
}

#[cfg(test)]
mod rtt_msaa_tests {
    use super::*;

    #[test]
    fn test_rtt_begin_end() {
        let mut rtt = RenderToTexture::new();
        assert!(!rtt.is_active());
        assert!(rtt.begin_rtt(0x1000, 1920, 1080));
        assert!(rtt.is_active());
        assert_eq!(rtt.current_dimensions(), (1920, 1080));
        assert!(rtt.has_rtt_binding(0x1000));
        
        let target = rtt.end_rtt();
        assert_eq!(target, 0x1000);
        assert!(!rtt.is_active());
    }

    #[test]
    fn test_rtt_double_begin() {
        let mut rtt = RenderToTexture::new();
        assert!(rtt.begin_rtt(0x1000, 640, 480));
        assert!(!rtt.begin_rtt(0x2000, 800, 600)); // Should fail
        assert!(rtt.is_active());
    }

    #[test]
    fn test_rtt_end_without_begin() {
        let mut rtt = RenderToTexture::new();
        assert_eq!(rtt.end_rtt(), 0);
    }

    #[test]
    fn test_rtt_binding_management() {
        let mut rtt = RenderToTexture::new();
        rtt.begin_rtt(0x1000, 100, 100);
        rtt.end_rtt();
        rtt.begin_rtt(0x2000, 200, 200);
        rtt.end_rtt();
        
        assert_eq!(rtt.binding_count(), 2);
        assert!(rtt.has_rtt_binding(0x1000));
        assert!(rtt.has_rtt_binding(0x2000));
        assert!(!rtt.has_rtt_binding(0x3000));
        
        assert!(rtt.remove_binding(0x1000));
        assert_eq!(rtt.binding_count(), 1);
    }

    #[test]
    fn test_fb_copy_simple() {
        let mut copier = FramebufferCopier::new();
        assert_eq!(copier.pending_count(), 0);
        
        // Create a simple 8x2 framebuffer, 4bpp
        let mut fb = vec![0u8; 8 * 2 * 4]; // 8 wide, 2 tall, 4 bpp
        // Fill first row with 0xAA
        for b in 0..32 { fb[b] = 0xAA; }
        
        copier.queue_copy(FramebufferCopy {
            src_offset: 0,
            dst_offset: 0,
            src_rect: (0, 0, 4, 1),
            dst_rect: (4, 1, 4, 1),
            src_bpp: 4,
            dst_bpp: 4,
        });
        
        assert_eq!(copier.pending_count(), 1);
        let count = copier.execute_copies(&mut fb, 8 * 4);
        assert_eq!(count, 1);
        assert_eq!(copier.total_copies(), 1);
        
        // Check destination was written
        assert_eq!(fb[32 + 16], 0xAA); // Row 1, col 4
    }

    #[test]
    fn test_msaa_configure() {
        let mut msaa = MsaaResolver::new();
        assert!(!msaa.is_enabled());
        assert_eq!(msaa.sample_count(), 1);
        
        msaa.configure(4, true);
        assert!(msaa.is_enabled());
        assert_eq!(msaa.sample_count(), 4);
        assert_eq!(msaa.sample_positions().len(), 4);
    }

    #[test]
    fn test_msaa_invalid_count() {
        let mut msaa = MsaaResolver::new();
        msaa.configure(3, true); // Invalid, should clamp to 1
        assert_eq!(msaa.sample_count(), 1);
        assert!(!msaa.is_enabled()); // 1 sample = not really MSAA
    }

    #[test]
    fn test_msaa_resolve_4x() {
        let mut msaa = MsaaResolver::new();
        msaa.configure(4, true);
        
        // 1x1 pixel with 4 samples: R varies, G/B/A constant
        let src = vec![
            100, 50, 25, 255, // Sample 0
            200, 50, 25, 255, // Sample 1
            100, 50, 25, 255, // Sample 2
            200, 50, 25, 255, // Sample 3
        ];
        
        let result = msaa.resolve_color(&src, 1, 1);
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], 150); // Average of 100,200,100,200
        assert_eq!(result[1], 50);
        assert_eq!(result[2], 25);
        assert_eq!(result[3], 255);
    }

    #[test]
    fn test_msaa_resolve_no_msaa() {
        let mut msaa = MsaaResolver::new();
        msaa.configure(1, false);
        
        let src = vec![128, 64, 32, 255];
        let result = msaa.resolve_color(&src, 1, 1);
        assert_eq!(result, vec![128, 64, 32, 255]);
    }

    #[test]
    fn test_msaa_sample_positions_8x() {
        let mut msaa = MsaaResolver::new();
        msaa.configure(8, true);
        assert_eq!(msaa.sample_positions().len(), 8);
        
        // Positions should be within [0, 1] range
        for (x, y) in msaa.sample_positions() {
            assert!(*x >= 0.0 && *x <= 1.0);
            assert!(*y >= 0.0 && *y <= 1.0);
        }
    }
}
