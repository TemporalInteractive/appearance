@include ::color
@include appearance-path-tracer-gpu::shared/ray
@include appearance-path-tracer-gpu::shared/material/disney_bsdf

@include appearance-path-tracer-gpu::helpers/trace

///
/// BINDING DEPENDENCIES:
/// appearance-path-tracer-gpu::shared/vertex_pool_bindings
/// appearance-path-tracer-gpu::shared/material/material_pool_bindings
/// appearance-path-tracer-gpu::shared/sky_bindings
///

fn InlinePathTracer::trace(origin: vec3<f32>, direction: vec3<f32>, scene: acceleration_structure) -> vec3<f32> {
    
}