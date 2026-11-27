//! The GPU side: one pipeline, one instance buffer, a glyph atlas (text and icons) and an image
//! atlas (covers and page tiles). Copied from Tally RS, which adapted Feel the Beat's renderer and
//! added clipping, so a frame is a few draw calls, one per clip region, instead of one.
//!
//! [`crate::draw::Canvas`] fills a list of [`Inst`] and [`Batch`]es; [`Gfx::render`] draws them.

use bytemuck::{Pod, Zeroable};

pub const KIND_BOX: f32 = 0.0;
pub const KIND_GLYPH: f32 = 1.0;
pub const KIND_ARC: f32 = 2.0;
/// A picture from the image atlas, cut to a rounded rectangle.
pub const KIND_IMAGE: f32 = 5.0;
/// The gradient runs left to right instead of top to bottom.
pub const FLAG_HGRAD: f32 = 64.0;
/// An upright rectangle with square corners and hard edges: no distance field. Set by the canvas.
pub const FLAG_PLAIN: f32 = 128.0;
/// An arc with flat ends.
pub const FLAG_BUTT: f32 = 256.0;
/// A picture with hard edges on whole pixels: no smoothing, so tiles meet without a seam.
pub const FLAG_HARD: f32 = 512.0;

pub const ATLAS_SIZE: u32 = 2048;
/// The picture atlas's side where the GPU allows it ([`Gfx::images_size`]): nine page tiles of
/// 800 × 1020. Some GLES 3.0 phones stop at 2048: there the canvas keeps pictures at half size
/// ([`crate::draw::Canvas::add_image`]).
pub const IMAGES_SIZE: u32 = 3072;

/// A run of instances drawn under one clip rectangle (pixels: x, y, w, h), or none.
#[derive(Clone, Copy, Debug)]
pub struct Batch {
    pub start: u32,
    pub end: u32,
    pub clip: Option<[u32; 4]>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Default)]
pub struct Inst {
    /// x, y, width, height.
    pub rect: [f32; 4],
    /// Atlas rectangle for glyphs: u0, v0, u1, v1.
    pub uv: [f32; 4],
    /// Colour at the top (or left) and at the bottom (or right).
    pub c0: [u8; 4],
    pub c1: [u8; 4],
    /// Corner radius, outline thickness (0 fills), glow reach, kind + flags.
    pub p: [f32; 4],
    /// Rotation in radians, arc start, arc end, unused.
    pub e: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Globals {
    pub screen: [f32; 2],
    /// `_pad[0]` is 1 when the target converts to sRGB on write; set by the renderer.
    pub _pad: [f32; 2],
}

/// A glyph rectangle waiting to be copied into the atlas texture.
pub struct AtlasUpload {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub pixels: Vec<u8>,
}

pub struct Gfx {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    globals: wgpu::Buffer,
    atlas: wgpu::Texture,
    images: wgpu::Texture,
    /// The picture atlas's side: [`IMAGES_SIZE`], or less where the GPU stops short of it.
    images_size: u32,
    instances: wgpu::Buffer,
    instance_capacity: usize,
    pub format: wgpu::TextureFormat,
}

/// A window (or the phone's screen) to present to.
pub struct Screen<'w> {
    surface: wgpu::Surface<'w>,
    config: wgpu::SurfaceConfiguration,
}

fn instance() -> wgpu::Instance {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    // Vulkan where there is one; GLES for phones whose Vulkan driver is not usable.
    desc.backends = wgpu::Backends::VULKAN | wgpu::Backends::GL;
    wgpu::Instance::new(desc)
}

impl Gfx {
    /// Set up for a window. `target` is anything wgpu can make a surface from.
    pub fn with_surface<'w>(
        target: impl Into<wgpu::SurfaceTarget<'w>>,
        width: u32,
        height: u32,
    ) -> Result<(Gfx, Screen<'w>), String> {
        let instance = instance();
        let surface = instance.create_surface(target).map_err(|e| format!("no surface: {e}"))?;
        Self::for_surface(instance, surface, width, height)
    }

    /// Set up for a native window known only by its raw handles (Android).
    ///
    /// # Safety
    /// The window behind `window` must stay alive until the returned [`Screen`] is dropped.
    pub unsafe fn with_raw(
        display: raw_window_handle::RawDisplayHandle,
        window: raw_window_handle::RawWindowHandle,
        width: u32,
        height: u32,
    ) -> Result<(Gfx, Screen<'static>), String> {
        let instance = instance();
        let target = wgpu::SurfaceTargetUnsafe::RawHandle { raw_display_handle: Some(display), raw_window_handle: window };
        let surface = unsafe { instance.create_surface_unsafe(target) }.map_err(|e| format!("no surface: {e}"))?;
        Self::for_surface(instance, surface, width, height)
    }

    /// A screen for a new native window, on the device already set up. Android destroys and
    /// recreates the window whenever the app leaves and returns; the device, the pipeline and the
    /// glyph atlas all survive that.
    ///
    /// # Safety
    /// As [`Self::with_raw`].
    pub unsafe fn raw_screen(
        &self,
        display: raw_window_handle::RawDisplayHandle,
        window: raw_window_handle::RawWindowHandle,
        width: u32,
        height: u32,
    ) -> Result<Screen<'static>, String> {
        let target = wgpu::SurfaceTargetUnsafe::RawHandle { raw_display_handle: Some(display), raw_window_handle: window };
        let surface = unsafe { self.instance.create_surface_unsafe(target) }.map_err(|e| format!("no surface: {e}"))?;
        self.screen(surface, width, height)
    }

    fn for_surface<'w>(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'w>,
        width: u32,
        height: u32,
    ) -> Result<(Gfx, Screen<'w>), String> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            ..Default::default()
        }))
        .map_err(|e| format!("no graphics adapter: {e}"))?;
        let caps = surface.get_capabilities(&adapter);
        // Colours are authored in display space and blended there, like every 2D toolkit, so the
        // target should not convert on write: take a plain format where the surface has one.
        // Where it has only sRGB ones the shader converts instead (reinterpreting the surface's
        // format needs a driver feature cheap phones lack).
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or("the surface supports no format")?;
        let gfx = Gfx::new(instance, adapter, format)?;
        let screen = gfx.screen(surface, width, height)?;
        Ok((gfx, screen))
    }

    fn screen<'w>(&self, surface: wgpu::Surface<'w>, width: u32, height: u32) -> Result<Screen<'w>, String> {
        let caps = surface.get_capabilities(&self.adapter);
        if !caps.formats.contains(&self.format) {
            return Err("the window's format changed".into());
        }
        let mut config = surface
            .get_default_config(&self.adapter, width.max(1), height.max(1))
            .ok_or("the surface cannot be configured")?;
        config.format = self.format;
        config.view_formats = vec![];
        config.present_mode = wgpu::PresentMode::Fifo;
        // Two frames may be queued. One would put a touch on screen a refresh sooner, but it
        // leaves Android two buffers to share with the compositor, and the Moto G Fast then
        // presents every other refresh: 33 ms between frames, measured, against 16.7 with two.
        config.desired_maximum_frame_latency = 2;
        config.alpha_mode = caps
            .alpha_modes
            .iter()
            .copied()
            .find(|m| *m == wgpu::CompositeAlphaMode::Opaque)
            .unwrap_or(caps.alpha_modes[0]);
        surface.configure(&self.device, &config);
        Ok(Screen { surface, config })
    }

    /// Which driver is drawing and into what, for logs and benchmark reports.
    pub fn adapter_name(&self) -> String {
        let info = self.adapter.get_info();
        format!("{} ({:?}), {:?}", info.name, info.backend, self.format)
    }

    /// Set up with no window at all, to draw into a texture (screenshots, benchmarks).
    pub fn headless() -> Result<Gfx, String> {
        let instance = instance();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
            ..Default::default()
        }))
        .map_err(|e| format!("no graphics adapter: {e}"))?;
        Gfx::new(instance, adapter, wgpu::TextureFormat::Rgba8Unorm)
    }

    fn new(instance: wgpu::Instance, adapter: wgpu::Adapter, format: wgpu::TextureFormat) -> Result<Gfx, String> {
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("leaf"),
            required_features: wgpu::Features::empty(),
            // The lowest common denominator: nothing here needs more, and old phones offer no more.
            required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))
        .map_err(|e| format!("no graphics device: {e}"))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyphs"),
            size: wgpu::Extent3d { width: ATLAS_SIZE, height: ATLAS_SIZE, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas.create_view(&wgpu::TextureViewDescriptor::default());
        // A texture past the device's limit fails validation, which panics.
        let images_size = IMAGES_SIZE.min(device.limits().max_texture_dimension_2d);
        let images = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("images"),
            size: wgpu::Extent3d { width: images_size, height: images_size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let images_view = images.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyphs"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("leaf"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("leaf"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&atlas_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&images_view) },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("leaf"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![
            0 => Float32x4, 1 => Float32x4, 2 => Unorm8x4, 3 => Unorm8x4, 4 => Float32x4, 5 => Float32x4
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("leaf"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Inst>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attributes,
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let instance_capacity = 2048;
        let instances = Self::instance_buffer(&device, instance_capacity);
        Ok(Gfx { instance, adapter, device, queue, pipeline, bind_group, globals, atlas, images, images_size, instances, instance_capacity, format })
    }

    fn instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instances"),
            size: (capacity * std::mem::size_of::<Inst>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    pub fn upload_glyphs(&self, uploads: &mut Vec<AtlasUpload>) {
        for up in uploads.drain(..) {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: up.x, y: up.y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &up.pixels,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(up.w), rows_per_image: Some(up.h) },
                wgpu::Extent3d { width: up.w, height: up.h, depth_or_array_layers: 1 },
            );
        }
    }

    /// The picture atlas's side, in pixels: the canvas places pictures in that much room
    /// ([`crate::draw::Canvas::set_images_size`]).
    pub fn images_size(&self) -> u32 {
        self.images_size
    }

    /// Copy pictures waiting in the canvas into the image atlas.
    pub fn upload_images(&self, uploads: &mut Vec<AtlasUpload>) {
        for up in uploads.drain(..) {
            // Placed for a larger atlas than this one: writing it would fail validation.
            if up.x + up.w > self.images_size || up.y + up.h > self.images_size {
                continue;
            }
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.images,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: up.x, y: up.y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &up.pixels,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(up.w * 4), rows_per_image: Some(up.h) },
                wgpu::Extent3d { width: up.w, height: up.h, depth_or_array_layers: 1 },
            );
        }
    }

    /// Hands the queued uploads to the GPU and waits for them. A frame drawn to a window does this
    /// as it is submitted; frames drawn only to settle a screenshot must, or every upload's staging
    /// copy stays in memory (a reader shot once held 22 GB of them).
    pub fn flush(&self) {
        self.queue.submit(std::iter::empty());
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }

    /// Draw `instances` over `clear` into `view`.
    pub fn render(&mut self, view: &wgpu::TextureView, globals: &Globals, clear: [f32; 3], instances: &[Inst], batches: &[Batch]) {
        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().next_power_of_two();
            self.instances = Self::instance_buffer(&self.device, self.instance_capacity);
        }
        let mut globals = *globals;
        globals._pad[0] = if self.format.is_srgb() { 1.0 } else { 0.0 };
        self.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        self.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(instances));
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear[0] as f64,
                            g: clear[1] as f64,
                            b: clear[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.instances.slice(..));
            let (sw, sh) = (globals.screen[0] as u32, globals.screen[1] as u32);
            for b in batches {
                if b.end <= b.start {
                    continue;
                }
                match b.clip {
                    Some([x, y, w, h]) => {
                        let (x, y) = (x.min(sw), y.min(sh));
                        let (w, h) = (w.min(sw - x), h.min(sh - y));
                        if w == 0 || h == 0 {
                            continue;
                        }
                        pass.set_scissor_rect(x, y, w, h);
                    }
                    None => pass.set_scissor_rect(0, 0, sw.max(1), sh.max(1)),
                }
                pass.draw(0..4, b.start..b.end);
            }
        }
        self.queue.submit(Some(encoder.finish()));
    }

    /// Draw one frame into a texture and read it back as RGBA rows. For screenshots.
    pub fn render_to_pixels(&mut self, width: u32, height: u32, globals: &Globals, clear: [f32; 3], instances: &[Inst], batches: &[Batch]) -> Vec<u8> {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shot"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.render(&view, globals, clear, instances, batches);
        let row = (width * 4).next_multiple_of(256);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shot"),
            size: (row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(height) },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        self.queue.submit(Some(encoder.finish()));
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let Ok(data) = slice.get_mapped_range() else { return Vec::new() };
        let mut out = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            let start = (y * row) as usize;
            out.extend_from_slice(&data[start..start + (width * 4) as usize]);
        }
        out
    }
}

impl Screen<'_> {
    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn resize(&mut self, gfx: &Gfx, width: u32, height: u32) {
        if width == 0 || height == 0 || (width, height) == self.size() {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&gfx.device, &self.config);
    }

    /// Draw and present one frame, waiting for the display. False when the frame was skipped.
    pub fn frame(&mut self, gfx: &mut Gfx, globals: &Globals, clear: [f32; 3], instances: &[Inst], batches: &[Batch]) -> bool {
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&gfx.device, &self.config);
                return false;
            }
            // Hidden or late: skip the frame rather than stall the loop.
            _ => return false,
        };
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
        gfx.render(&view, globals, clear, instances, batches);
        gfx.queue.present(texture);
        true
    }
}
