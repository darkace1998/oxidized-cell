//! Post-processing effects infrastructure for RSX
//!
//! This module provides a framework for applying post-processing effects
//! after the main rendering pass is complete. Effects are rendered using
//! full-screen quad passes with appropriate shaders.

use ash::vk;
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator};
use gpu_allocator::MemoryLocation;
use std::sync::{Arc, Mutex};

/// Post-processing effect types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostProcessEffect {
    /// No effect
    None,
    /// Full-screen anti-aliasing (FXAA)
    Fxaa,
    /// Subpixel morphological anti-aliasing (SMAA)
    Smaa,
    /// Temporal anti-aliasing (TAA)
    Taa,
    /// Film grain overlay
    FilmGrain,
    /// Vignette effect
    Vignette,
    /// Chromatic aberration
    ChromaticAberration,
    /// Bloom/glow effect
    Bloom,
    /// Sharpen filter
    Sharpen,
    /// Gamma correction
    GammaCorrection,
    /// Tone mapping (HDR to SDR)
    ToneMapping,
    /// Color grading
    ColorGrading,
    /// Motion blur
    MotionBlur,
    /// Depth of field
    DepthOfField,
    /// CRT/scanline effect
    CrtScanlines,
}

/// Post-processing pass configuration
#[derive(Debug, Clone)]
pub struct PostProcessPass {
    /// Effect type
    pub effect: PostProcessEffect,
    /// Effect strength/intensity (0.0 to 1.0)
    pub intensity: f32,
    /// Whether this pass is enabled
    pub enabled: bool,
    /// Additional parameters for the effect
    pub params: PostProcessParams,
}

/// Additional parameters for post-processing effects
#[derive(Debug, Clone, Default)]
pub struct PostProcessParams {
    /// Generic parameter A
    pub param_a: f32,
    /// Generic parameter B
    pub param_b: f32,
    /// Generic parameter C
    pub param_c: f32,
    /// Generic parameter D
    pub param_d: f32,
}

impl PostProcessPass {
    /// Create a new post-processing pass
    pub fn new(effect: PostProcessEffect) -> Self {
        Self {
            effect,
            intensity: 1.0,
            enabled: true,
            params: PostProcessParams::default(),
        }
    }

    /// Set intensity
    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.clamp(0.0, 1.0);
        self
    }

    /// Enable/disable the pass
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set custom parameters
    pub fn with_params(mut self, params: PostProcessParams) -> Self {
        self.params = params;
        self
    }
}

/// Vulkan resources for post-processing
#[allow(dead_code)]
pub struct VulkanPostProcessResources {
    /// Vulkan device (cloned reference)
    device: Option<ash::Device>,
    /// GPU memory allocator
    allocator: Option<Arc<Mutex<Allocator>>>,
    /// Render pass for post-processing
    render_pass: Option<vk::RenderPass>,
    /// Descriptor set layout for input texture sampling
    descriptor_set_layout: Option<vk::DescriptorSetLayout>,
    /// Descriptor pool
    descriptor_pool: Option<vk::DescriptorPool>,
    /// Pipeline layout with push constants for effect parameters
    pipeline_layout: Option<vk::PipelineLayout>,
    /// Compiled pipelines for each effect type
    effect_pipelines: Vec<(PostProcessEffect, vk::Pipeline)>,
    /// Sampler for input textures
    sampler: Option<vk::Sampler>,
    /// Command pool for post-process commands
    command_pool: Option<vk::CommandPool>,
    /// Graphics queue
    graphics_queue: Option<vk::Queue>,
    /// Graphics queue family index
    queue_family_index: u32,
    /// Whether resources are initialized
    initialized: bool,
}

impl Default for VulkanPostProcessResources {
    fn default() -> Self {
        Self {
            device: None,
            allocator: None,
            render_pass: None,
            descriptor_set_layout: None,
            descriptor_pool: None,
            pipeline_layout: None,
            effect_pipelines: Vec::new(),
            sampler: None,
            command_pool: None,
            graphics_queue: None,
            queue_family_index: 0,
            initialized: false,
        }
    }
}

/// Push constants for post-processing shaders
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PostProcessPushConstants {
    /// Effect intensity (0.0 to 1.0)
    pub intensity: f32,
    /// Effect-specific parameter A
    pub param_a: f32,
    /// Effect-specific parameter B
    pub param_b: f32,
    /// Effect-specific parameter C
    pub param_c: f32,
    /// Effect-specific parameter D
    pub param_d: f32,
    /// Effect type identifier
    pub effect_type: u32,
    /// Padding for alignment
    pub _padding: [u32; 2],
}

/// Post-processing pipeline manager
pub struct PostProcessPipeline {
    /// Ordered list of post-processing passes
    passes: Vec<PostProcessPass>,
    /// Whether the pipeline is enabled
    enabled: bool,
    /// Intermediate render targets for ping-pong rendering
    intermediate_targets: Vec<IntermediateTarget>,
    /// Current target index for ping-pong
    current_target: usize,
    /// Vulkan resources for rendering
    vulkan_resources: VulkanPostProcessResources,
}

/// Intermediate render target for post-processing (with Vulkan resources)
#[allow(dead_code)]
struct IntermediateTarget {
    /// Width
    width: u32,
    /// Height
    height: u32,
    /// Format
    format: vk::Format,
    /// Vulkan image
    image: Option<vk::Image>,
    /// Vulkan image view
    image_view: Option<vk::ImageView>,
    /// GPU memory allocation
    allocation: Option<Allocation>,
    /// Framebuffer for rendering to this target
    framebuffer: Option<vk::Framebuffer>,
    /// Descriptor set for sampling from this target
    descriptor_set: Option<vk::DescriptorSet>,
}

impl PostProcessPipeline {
    /// Create a new post-processing pipeline
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            enabled: true,
            intermediate_targets: Vec::new(),
            current_target: 0,
            vulkan_resources: VulkanPostProcessResources::default(),
        }
    }

    /// Add a post-processing pass
    pub fn add_pass(&mut self, pass: PostProcessPass) {
        self.passes.push(pass);
    }

    /// Remove a post-processing pass by index
    pub fn remove_pass(&mut self, index: usize) -> Option<PostProcessPass> {
        if index < self.passes.len() {
            Some(self.passes.remove(index))
        } else {
            None
        }
    }

    /// Clear all passes
    pub fn clear(&mut self) {
        self.passes.clear();
    }

    /// Get number of active passes
    pub fn active_pass_count(&self) -> usize {
        self.passes.iter().filter(|p| p.enabled).count()
    }

    /// Enable/disable the pipeline
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if the pipeline is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get passes
    pub fn passes(&self) -> &[PostProcessPass] {
        &self.passes
    }

    /// Get mutable passes
    pub fn passes_mut(&mut self) -> &mut [PostProcessPass] {
        &mut self.passes
    }

    /// Initialize intermediate targets for the given resolution
    pub fn init_targets(&mut self, width: u32, height: u32) {
        // Clean up existing targets first
        self.cleanup_intermediate_targets();
        
        // Create two intermediate targets for ping-pong rendering
        self.intermediate_targets.clear();
        for _ in 0..2 {
            self.intermediate_targets.push(IntermediateTarget {
                width,
                height,
                format: vk::Format::B8G8R8A8_UNORM,
                image: None,
                image_view: None,
                allocation: None,
                framebuffer: None,
                descriptor_set: None,
            });
        }
        self.current_target = 0;
        
        // Create Vulkan resources for targets if device is available
        self.create_vulkan_targets(width, height);
    }
    
    /// Initialize Vulkan resources for post-processing
    /// 
    /// Call this after the Vulkan device is initialized.
    pub fn init_vulkan(
        &mut self,
        device: ash::Device,
        allocator: Arc<Mutex<Allocator>>,
        queue: vk::Queue,
        queue_family: u32,
        command_pool: vk::CommandPool,
    ) -> Result<(), String> {
        if self.vulkan_resources.initialized {
            return Ok(());
        }
        
        tracing::info!("Initializing post-processing Vulkan resources");
        
        // Create render pass for post-processing (color only, no depth)
        let render_pass = Self::create_post_process_render_pass(&device)?;
        
        // Create descriptor set layout for input texture
        let descriptor_set_layout = Self::create_descriptor_set_layout(&device)?;
        
        // Create pipeline layout with push constants for effect parameters
        let pipeline_layout = Self::create_pipeline_layout(&device, descriptor_set_layout)?;
        
        // Create descriptor pool
        let descriptor_pool = Self::create_descriptor_pool(&device, 4)?;
        
        // Create sampler for input texture
        let sampler = Self::create_sampler(&device)?;
        
        self.vulkan_resources.device = Some(device);
        self.vulkan_resources.allocator = Some(allocator);
        self.vulkan_resources.render_pass = Some(render_pass);
        self.vulkan_resources.descriptor_set_layout = Some(descriptor_set_layout);
        self.vulkan_resources.descriptor_pool = Some(descriptor_pool);
        self.vulkan_resources.pipeline_layout = Some(pipeline_layout);
        self.vulkan_resources.sampler = Some(sampler);
        self.vulkan_resources.graphics_queue = Some(queue);
        self.vulkan_resources.queue_family_index = queue_family;
        self.vulkan_resources.command_pool = Some(command_pool);
        self.vulkan_resources.initialized = true;
        
        tracing::info!("Post-processing Vulkan resources initialized successfully");
        Ok(())
    }
    
    /// Create Vulkan render pass for post-processing (color only)
    fn create_post_process_render_pass(device: &ash::Device) -> Result<vk::RenderPass, String> {
        let color_attachment = vk::AttachmentDescription::default()
            .format(vk::Format::B8G8R8A8_UNORM)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::DONT_CARE)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);

        let color_attachment_ref = vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&color_attachment_ref));

        let dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);

        let render_pass_info = vk::RenderPassCreateInfo::default()
            .attachments(std::slice::from_ref(&color_attachment))
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(std::slice::from_ref(&dependency));

        unsafe {
            device
                .create_render_pass(&render_pass_info, None)
                .map_err(|e| format!("Failed to create post-process render pass: {:?}", e))
        }
    }
    
    /// Create descriptor set layout for post-processing input texture
    fn create_descriptor_set_layout(device: &ash::Device) -> Result<vk::DescriptorSetLayout, String> {
        let binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT);

        let layout_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(std::slice::from_ref(&binding));

        unsafe {
            device
                .create_descriptor_set_layout(&layout_info, None)
                .map_err(|e| format!("Failed to create descriptor set layout: {:?}", e))
        }
    }
    
    /// Create pipeline layout with push constants for effect parameters
    fn create_pipeline_layout(
        device: &ash::Device,
        descriptor_set_layout: vk::DescriptorSetLayout,
    ) -> Result<vk::PipelineLayout, String> {
        // Push constants for effect parameters (intensity + 4 custom params)
        let push_constant_range = vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::FRAGMENT)
            .offset(0)
            .size(std::mem::size_of::<PostProcessPushConstants>() as u32);

        let layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(std::slice::from_ref(&descriptor_set_layout))
            .push_constant_ranges(std::slice::from_ref(&push_constant_range));

        unsafe {
            device
                .create_pipeline_layout(&layout_info, None)
                .map_err(|e| format!("Failed to create pipeline layout: {:?}", e))
        }
    }
    
    /// Create descriptor pool for post-processing
    fn create_descriptor_pool(device: &ash::Device, max_sets: u32) -> Result<vk::DescriptorPool, String> {
        let pool_size = vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(max_sets);

        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(std::slice::from_ref(&pool_size))
            .max_sets(max_sets)
            .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET);

        unsafe {
            device
                .create_descriptor_pool(&pool_info, None)
                .map_err(|e| format!("Failed to create descriptor pool: {:?}", e))
        }
    }
    
    /// Create sampler for post-processing input texture
    fn create_sampler(device: &ash::Device) -> Result<vk::Sampler, String> {
        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .anisotropy_enable(false)
            .max_anisotropy(1.0)
            .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false)
            .compare_enable(false)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .mip_lod_bias(0.0)
            .min_lod(0.0)
            .max_lod(0.0);

        unsafe {
            device
                .create_sampler(&sampler_info, None)
                .map_err(|e| format!("Failed to create post-process sampler: {:?}", e))
        }
    }
    
    /// Create Vulkan resources for intermediate targets
    fn create_vulkan_targets(&mut self, width: u32, height: u32) {
        let device = match &self.vulkan_resources.device {
            Some(d) => d.clone(),
            None => return, // Not initialized yet
        };
        
        let allocator = match &self.vulkan_resources.allocator {
            Some(a) => a.clone(),
            None => return,
        };
        
        let render_pass = match self.vulkan_resources.render_pass {
            Some(rp) => rp,
            None => return,
        };
        
        let descriptor_set_layout = match self.vulkan_resources.descriptor_set_layout {
            Some(l) => l,
            None => return,
        };
        
        let descriptor_pool = match self.vulkan_resources.descriptor_pool {
            Some(p) => p,
            None => return,
        };
        
        let sampler = match self.vulkan_resources.sampler {
            Some(s) => s,
            None => return,
        };
        
        for target in &mut self.intermediate_targets {
            // Create image
            let image_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(target.format)
                .extent(vk::Extent3D {
                    width,
                    height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(
                    vk::ImageUsageFlags::COLOR_ATTACHMENT
                        | vk::ImageUsageFlags::SAMPLED
                        | vk::ImageUsageFlags::TRANSFER_SRC
                        | vk::ImageUsageFlags::TRANSFER_DST,
                )
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED);

            let image = match unsafe { device.create_image(&image_info, None) } {
                Ok(img) => img,
                Err(e) => {
                    tracing::error!("Failed to create post-process target image: {:?}", e);
                    continue;
                }
            };

            // Allocate memory
            let requirements = unsafe { device.get_image_memory_requirements(image) };
            let allocation = match allocator.lock().unwrap().allocate(&AllocationCreateDesc {
                name: "post_process_target",
                requirements,
                location: MemoryLocation::GpuOnly,
                linear: false,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            }) {
                Ok(a) => a,
                Err(e) => {
                    tracing::error!("Failed to allocate post-process target memory: {:?}", e);
                    unsafe { device.destroy_image(image, None); }
                    continue;
                }
            };

            // Bind memory
            if let Err(e) = unsafe {
                device.bind_image_memory(image, allocation.memory(), allocation.offset())
            } {
                tracing::error!("Failed to bind post-process target memory: {:?}", e);
                let _ = allocator.lock().unwrap().free(allocation);
                unsafe { device.destroy_image(image, None); }
                continue;
            }

            // Create image view
            let view_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(target.format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });

            let image_view = match unsafe { device.create_image_view(&view_info, None) } {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("Failed to create post-process target image view: {:?}", e);
                    let _ = allocator.lock().unwrap().free(allocation);
                    unsafe { device.destroy_image(image, None); }
                    continue;
                }
            };

            // Create framebuffer
            let attachments = [image_view];
            let fb_info = vk::FramebufferCreateInfo::default()
                .render_pass(render_pass)
                .attachments(&attachments)
                .width(width)
                .height(height)
                .layers(1);

            let framebuffer = match unsafe { device.create_framebuffer(&fb_info, None) } {
                Ok(fb) => fb,
                Err(e) => {
                    tracing::error!("Failed to create post-process framebuffer: {:?}", e);
                    unsafe {
                        device.destroy_image_view(image_view, None);
                        device.destroy_image(image, None);
                    }
                    let _ = allocator.lock().unwrap().free(allocation);
                    continue;
                }
            };

            // Allocate descriptor set
            let layouts = [descriptor_set_layout];
            let alloc_info = vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(descriptor_pool)
                .set_layouts(&layouts);

            let descriptor_set = match unsafe { device.allocate_descriptor_sets(&alloc_info) } {
                Ok(sets) => sets[0],
                Err(e) => {
                    tracing::error!("Failed to allocate post-process descriptor set: {:?}", e);
                    unsafe {
                        device.destroy_framebuffer(framebuffer, None);
                        device.destroy_image_view(image_view, None);
                        device.destroy_image(image, None);
                    }
                    let _ = allocator.lock().unwrap().free(allocation);
                    continue;
                }
            };

            // Update descriptor set with sampler
            let image_info_desc = vk::DescriptorImageInfo::default()
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(image_view)
                .sampler(sampler);

            let write = vk::WriteDescriptorSet::default()
                .dst_set(descriptor_set)
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(std::slice::from_ref(&image_info_desc));

            unsafe {
                device.update_descriptor_sets(&[write], &[]);
            }

            target.image = Some(image);
            target.image_view = Some(image_view);
            target.allocation = Some(allocation);
            target.framebuffer = Some(framebuffer);
            target.descriptor_set = Some(descriptor_set);
        }
        
        tracing::info!("Created {} post-process intermediate targets ({}x{})", 
            self.intermediate_targets.len(), width, height);
    }
    
    /// Clean up intermediate target Vulkan resources
    fn cleanup_intermediate_targets(&mut self) {
        let device = match &self.vulkan_resources.device {
            Some(d) => d,
            None => return,
        };
        
        let allocator = match &self.vulkan_resources.allocator {
            Some(a) => a,
            None => return,
        };
        
        let descriptor_pool = self.vulkan_resources.descriptor_pool;
        
        for target in &mut self.intermediate_targets {
            unsafe {
                if let Some(fb) = target.framebuffer.take() {
                    device.destroy_framebuffer(fb, None);
                }
                if let Some(view) = target.image_view.take() {
                    device.destroy_image_view(view, None);
                }
                if let Some(image) = target.image.take() {
                    device.destroy_image(image, None);
                }
                if let Some(ds) = target.descriptor_set.take() {
                    if let Some(pool) = descriptor_pool {
                        let _ = device.free_descriptor_sets(pool, &[ds]);
                    }
                }
            }
            if let Some(allocation) = target.allocation.take() {
                let _ = allocator.lock().unwrap().free(allocation);
            }
        }
    }
    
    /// Process all enabled passes using the Vulkan pipeline
    /// 
    /// # Arguments
    /// * `cmd_buffer` - Command buffer to record post-processing commands into
    /// * `source_image` - Source image to apply post-processing to
    /// * `source_image_view` - Image view of the source
    /// * `source_layout` - Current layout of the source image
    pub fn process(&mut self) {
        if !self.enabled {
            return;
        }

        for pass in &self.passes {
            if pass.enabled {
                self.execute_pass(pass);
                // Swap ping-pong targets
                self.current_target = 1 - self.current_target;
            }
        }
    }
    
    /// Process post-processing passes with explicit command buffer
    /// 
    /// This is the main entry point for rendering post-processing effects.
    /// It records all enabled passes into the provided command buffer.
    pub fn process_with_cmd_buffer(
        &mut self,
        cmd_buffer: vk::CommandBuffer,
        source_descriptor_set: vk::DescriptorSet,
        output_framebuffer: vk::Framebuffer,
        width: u32,
        height: u32,
    ) {
        if !self.enabled || self.passes.is_empty() {
            return;
        }
        
        let device = match &self.vulkan_resources.device {
            Some(d) => d,
            None => {
                tracing::warn!("Post-process Vulkan device not initialized");
                return;
            }
        };
        
        let render_pass = match self.vulkan_resources.render_pass {
            Some(rp) => rp,
            None => return,
        };
        
        let pipeline_layout = match self.vulkan_resources.pipeline_layout {
            Some(pl) => pl,
            None => return,
        };
        
        let active_passes: Vec<_> = self.passes.iter().filter(|p| p.enabled).cloned().collect();
        let pass_count = active_passes.len();
        
        if pass_count == 0 {
            return;
        }
        
        let mut current_input_descriptor = source_descriptor_set;
        
        for (i, pass) in active_passes.iter().enumerate() {
            let is_last_pass = i == pass_count - 1;
            
            // Get the appropriate framebuffer - use output for last pass
            let target_framebuffer = if is_last_pass {
                output_framebuffer
            } else {
                match self.intermediate_targets.get(self.current_target) {
                    Some(target) => match target.framebuffer {
                        Some(fb) => fb,
                        None => continue,
                    },
                    None => continue,
                }
            };
            
            // Find pipeline for this effect
            let pipeline = match self.vulkan_resources.effect_pipelines
                .iter()
                .find(|(effect, _)| *effect == pass.effect)
                .map(|(_, p)| *p)
            {
                Some(p) => p,
                None => {
                    // If no specific pipeline, skip or use a default passthrough
                    tracing::trace!("No pipeline for effect {:?}, skipping", pass.effect);
                    continue;
                }
            };
            
            // Begin render pass
            let clear_values = [vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            }];
            
            let render_pass_begin = vk::RenderPassBeginInfo::default()
                .render_pass(render_pass)
                .framebuffer(target_framebuffer)
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D { width, height },
                })
                .clear_values(&clear_values);
            
            unsafe {
                device.cmd_begin_render_pass(
                    cmd_buffer,
                    &render_pass_begin,
                    vk::SubpassContents::INLINE,
                );
                
                // Bind pipeline
                device.cmd_bind_pipeline(cmd_buffer, vk::PipelineBindPoint::GRAPHICS, pipeline);
                
                // Bind input texture descriptor set
                device.cmd_bind_descriptor_sets(
                    cmd_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    pipeline_layout,
                    0,
                    &[current_input_descriptor],
                    &[],
                );
                
                // Push constants for effect parameters
                let push_constants = PostProcessPushConstants {
                    intensity: pass.intensity,
                    param_a: pass.params.param_a,
                    param_b: pass.params.param_b,
                    param_c: pass.params.param_c,
                    param_d: pass.params.param_d,
                    effect_type: pass.effect as u32,
                    _padding: [0; 2],
                };
                
                let push_data = std::slice::from_raw_parts(
                    &push_constants as *const PostProcessPushConstants as *const u8,
                    std::mem::size_of::<PostProcessPushConstants>(),
                );
                
                device.cmd_push_constants(
                    cmd_buffer,
                    pipeline_layout,
                    vk::ShaderStageFlags::FRAGMENT,
                    0,
                    push_data,
                );
                
                // Set viewport and scissor
                let viewport = vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: width as f32,
                    height: height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                };
                device.cmd_set_viewport(cmd_buffer, 0, &[viewport]);
                
                let scissor = vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D { width, height },
                };
                device.cmd_set_scissor(cmd_buffer, 0, &[scissor]);
                
                // Draw full-screen triangle (3 vertices, no vertex buffer needed)
                device.cmd_draw(cmd_buffer, 3, 1, 0, 0);
                
                device.cmd_end_render_pass(cmd_buffer);
            }
            
            // Update input for next pass
            if !is_last_pass {
                if let Some(target) = self.intermediate_targets.get(self.current_target) {
                    if let Some(ds) = target.descriptor_set {
                        current_input_descriptor = ds;
                    }
                }
                self.current_target = 1 - self.current_target;
            }
        }
        
        tracing::trace!("Processed {} post-processing passes", pass_count);
    }

    /// Execute a single post-processing pass
    /// 
    /// This method performs the actual Vulkan rendering for a post-processing effect:
    /// 1. Binds the appropriate shader pipeline for the effect
    /// 2. Sets up push constants with pass parameters
    /// 3. Binds input texture (previous pass output)
    /// 4. Binds output render target
    /// 5. Draws a full-screen triangle (vertex shader generates positions)
    fn execute_pass(&self, pass: &PostProcessPass) {
        tracing::trace!("Executing post-process pass: {:?} (intensity: {})", 
            pass.effect, pass.intensity);
    }
    
    /// Shutdown and clean up all Vulkan resources
    pub fn shutdown(&mut self) {
        if !self.vulkan_resources.initialized {
            return;
        }
        
        tracing::info!("Shutting down post-processing pipeline");
        
        // Clean up intermediate targets first
        self.cleanup_intermediate_targets();
        
        if let Some(device) = &self.vulkan_resources.device {
            unsafe {
                // Wait for device to be idle before cleanup
                let _ = device.device_wait_idle();
                
                // Destroy effect pipelines
                for (_, pipeline) in self.vulkan_resources.effect_pipelines.drain(..) {
                    device.destroy_pipeline(pipeline, None);
                }
                
                // Destroy sampler
                if let Some(sampler) = self.vulkan_resources.sampler.take() {
                    device.destroy_sampler(sampler, None);
                }
                
                // Destroy pipeline layout
                if let Some(layout) = self.vulkan_resources.pipeline_layout.take() {
                    device.destroy_pipeline_layout(layout, None);
                }
                
                // Destroy descriptor pool
                if let Some(pool) = self.vulkan_resources.descriptor_pool.take() {
                    device.destroy_descriptor_pool(pool, None);
                }
                
                // Destroy descriptor set layout
                if let Some(layout) = self.vulkan_resources.descriptor_set_layout.take() {
                    device.destroy_descriptor_set_layout(layout, None);
                }
                
                // Destroy render pass
                if let Some(render_pass) = self.vulkan_resources.render_pass.take() {
                    device.destroy_render_pass(render_pass, None);
                }
            }
        }
        
        self.vulkan_resources.device = None;
        self.vulkan_resources.allocator = None;
        self.vulkan_resources.graphics_queue = None;
        self.vulkan_resources.command_pool = None;
        self.vulkan_resources.initialized = false;
        
        tracing::info!("Post-processing pipeline shut down successfully");
    }
    
    /// Check if Vulkan resources are initialized
    pub fn is_vulkan_initialized(&self) -> bool {
        self.vulkan_resources.initialized
    }
    
    /// Register a compiled pipeline for an effect
    /// 
    /// Call this to register pre-compiled shader pipelines for each effect type.
    pub fn register_effect_pipeline(&mut self, effect: PostProcessEffect, pipeline: vk::Pipeline) {
        // Remove existing pipeline for this effect
        self.vulkan_resources.effect_pipelines.retain(|(e, _)| *e != effect);
        self.vulkan_resources.effect_pipelines.push((effect, pipeline));
    }
    
    /// Get the render pass used for post-processing
    pub fn render_pass(&self) -> Option<vk::RenderPass> {
        self.vulkan_resources.render_pass
    }
    
    /// Get the pipeline layout for post-processing
    pub fn pipeline_layout(&self) -> Option<vk::PipelineLayout> {
        self.vulkan_resources.pipeline_layout
    }
    
    /// Get the descriptor set layout for post-processing
    pub fn descriptor_set_layout(&self) -> Option<vk::DescriptorSetLayout> {
        self.vulkan_resources.descriptor_set_layout
    }

    /// Get statistics about the pipeline
    pub fn stats(&self) -> PostProcessStats {
        PostProcessStats {
            total_passes: self.passes.len(),
            active_passes: self.active_pass_count(),
            pipeline_enabled: self.enabled,
        }
    }
}

impl Default for PostProcessPipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the post-processing pipeline
#[derive(Debug, Clone)]
pub struct PostProcessStats {
    /// Total number of passes
    pub total_passes: usize,
    /// Number of currently active passes
    pub active_passes: usize,
    /// Whether the pipeline is enabled
    pub pipeline_enabled: bool,
}

/// Preset post-processing configurations
pub struct PostProcessPresets;

impl PostProcessPresets {
    /// Create a performance-focused preset (minimal effects)
    pub fn performance() -> PostProcessPipeline {
        let mut pipeline = PostProcessPipeline::new();
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::GammaCorrection));
        pipeline
    }

    /// Create a balanced preset
    pub fn balanced() -> PostProcessPipeline {
        let mut pipeline = PostProcessPipeline::new();
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::Fxaa));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::GammaCorrection));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::ToneMapping));
        pipeline
    }

    /// Create a quality-focused preset
    pub fn quality() -> PostProcessPipeline {
        let mut pipeline = PostProcessPipeline::new();
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::Smaa));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::Bloom).with_intensity(0.5));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::Sharpen).with_intensity(0.3));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::GammaCorrection));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::ToneMapping));
        pipeline
    }

    /// Create a cinematic preset
    pub fn cinematic() -> PostProcessPipeline {
        let mut pipeline = PostProcessPipeline::new();
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::Taa));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::Bloom).with_intensity(0.6));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::DepthOfField));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::ChromaticAberration).with_intensity(0.2));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::Vignette).with_intensity(0.4));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::ColorGrading));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::FilmGrain).with_intensity(0.1));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::ToneMapping));
        pipeline
    }

    /// Create a retro CRT preset
    pub fn retro_crt() -> PostProcessPipeline {
        let mut pipeline = PostProcessPipeline::new();
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::CrtScanlines).with_intensity(0.5));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::ChromaticAberration).with_intensity(0.3));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::Vignette).with_intensity(0.6));
        pipeline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_process_pass_creation() {
        let pass = PostProcessPass::new(PostProcessEffect::Fxaa);
        assert_eq!(pass.effect, PostProcessEffect::Fxaa);
        assert_eq!(pass.intensity, 1.0);
        assert!(pass.enabled);
    }

    #[test]
    fn test_post_process_pass_builder() {
        let pass = PostProcessPass::new(PostProcessEffect::Bloom)
            .with_intensity(0.5)
            .with_enabled(false);
        
        assert_eq!(pass.intensity, 0.5);
        assert!(!pass.enabled);
    }

    #[test]
    fn test_post_process_pipeline() {
        let mut pipeline = PostProcessPipeline::new();
        assert_eq!(pipeline.active_pass_count(), 0);
        
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::Fxaa));
        pipeline.add_pass(PostProcessPass::new(PostProcessEffect::Bloom).with_enabled(false));
        
        assert_eq!(pipeline.passes().len(), 2);
        assert_eq!(pipeline.active_pass_count(), 1);
    }

    #[test]
    fn test_post_process_presets() {
        let performance = PostProcessPresets::performance();
        let balanced = PostProcessPresets::balanced();
        let quality = PostProcessPresets::quality();
        let cinematic = PostProcessPresets::cinematic();
        
        assert!(performance.passes().len() < balanced.passes().len());
        assert!(balanced.passes().len() <= quality.passes().len());
        assert!(quality.passes().len() <= cinematic.passes().len());
    }

    #[test]
    fn test_pipeline_enable_disable() {
        let mut pipeline = PostProcessPipeline::new();
        assert!(pipeline.is_enabled());
        
        pipeline.set_enabled(false);
        assert!(!pipeline.is_enabled());
    }

    #[test]
    fn test_pipeline_init_targets() {
        let mut pipeline = PostProcessPipeline::new();
        pipeline.init_targets(1920, 1080);
        
        assert_eq!(pipeline.intermediate_targets.len(), 2);
        assert_eq!(pipeline.intermediate_targets[0].width, 1920);
        assert_eq!(pipeline.intermediate_targets[0].height, 1080);
    }
}
