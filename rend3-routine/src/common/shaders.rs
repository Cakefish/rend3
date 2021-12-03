use std::borrow::Cow;

use rend3::RendererMode;
use wgpu::{Device, ShaderModule, ShaderModuleDescriptor, ShaderModuleDescriptorSpirV, ShaderSource};

use crate::shaders::{SPIRV_SHADERS, WGSL_SHADERS};

/// # Safety
///
/// The shader must be valid, match all the respective definitions, and otherwise meet wgpu's validation requirements
pub unsafe fn mode_safe_shader(
    device: &Device,
    mode: RendererMode,
    label: &str,
    cpu_source: &str,
    gpu_source: &str,
) -> ShaderModule {
    let shader_dir = match mode {
        RendererMode::CPUPowered => &SPIRV_SHADERS,
        RendererMode::GPUPowered => &SPIRV_SHADERS,
    };

    let source = shader_dir
        .get_file(match mode {
            RendererMode::CPUPowered => cpu_source,
            RendererMode::GPUPowered => gpu_source,
        })
        .unwrap()
        .contents();

    let use_unsafe = mode == RendererMode::GPUPowered;

    match use_unsafe {
        false => device.create_shader_module(&ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::util::make_spirv(source),
        }),
        true => device.create_shader_module_spirv(&ShaderModuleDescriptorSpirV {
            label: Some(label),
            source: wgpu::util::make_spirv_raw(source),
        }),
    }
}
