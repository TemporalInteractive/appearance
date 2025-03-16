@include ::random
@include ::color
@include appearance-path-tracer-gpu::shared/ray
@include appearance-path-tracer-gpu::shared/gbuffer
@include appearance-path-tracer-gpu::shared/material/disney_bsdf
@include appearance-path-tracer-gpu::shared/restir/gi_reservoir

@include appearance-path-tracer-gpu::shared/vertex_pool_bindings
@include appearance-path-tracer-gpu::shared/material/material_pool_bindings
@include appearance-path-tracer-gpu::shared/sky_bindings
@include appearance-path-tracer-gpu::shared/gbuffer_bindings

@include appearance-path-tracer-gpu::helpers/nee
@include appearance-path-tracer-gpu::helpers/inline_path_tracer

const NUM_SAMPLES: u32 = 5;

struct Constants {
    resolution: vec2<u32>,
    spatial_pass_count: u32,
    spatial_pass_idx: u32,
    pixel_radius: f32,
    seed: u32,
    unbiased: u32,
    _padding0: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<storage, read> in_rays: array<Ray>;

@group(0)
@binding(2)
var<storage, read_write> payloads: array<Payload>;

@group(0)
@binding(3)
var scene: acceleration_structure;

@group(0)
@binding(4)
var<storage, read> in_reservoirs: array<PackedGiReservoir>;

@group(0)
@binding(5)
var<storage, read_write> out_reservoirs: array<PackedGiReservoir>;

@group(0)
@binding(6)
var<storage, read_write> prev_reservoirs: array<PackedGiReservoir>;

@group(0)
@binding(7)
var<storage, read> light_sample_ctxs: array<LightSampleCtx>;

fn mirror(x: i32, max: i32) -> u32 {
    return u32(abs(((x + max) % (2 * max)) - max));
}

fn mirror_pixel(pixel: vec2<i32>) -> vec2<u32> {
    return vec2<u32>(
        mirror(pixel.x, i32(constants.resolution.x)),
        mirror(pixel.y, i32(constants.resolution.y))
    );
}

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id >= constants.resolution)) { return; }
    let flat_id: u32 = id.y * constants.resolution.x + id.x;

    let ray: Ray = in_rays[flat_id];
    var origin: vec3<f32> = ray.origin;
    var direction: vec3<f32> = PackedNormalizedXyz10::unpack(ray.direction, 0);

    var payload: Payload = payloads[flat_id];
    if (payload.t < 0.0) { return; } // TODO: indirect dispatch with pids

    let light_sample_ctx: LightSampleCtx = light_sample_ctxs[flat_id];
    
    var rng: u32 = payload.rng;
    let throughput: vec3<f32> = PackedRgb9e5::unpack(payload.throughput);

    let tex_coord: vec2<f32> = light_sample_ctx.hit_tex_coord;
    let material_idx: u32 = light_sample_ctx.hit_material_idx;
    let material_descriptor: MaterialDescriptor = material_descriptors[material_idx];
    let material: Material = Material::from_material_descriptor(material_descriptor, tex_coord);
    let disney_bsdf = DisneyBsdf::from_material(material);

    let hit_point_ws = origin + direction * payload.t;
    let front_facing_shading_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(light_sample_ctx.front_facing_shading_normal_ws, 0);
    let tangent_to_world: mat3x3<f32> = build_orthonormal_basis(front_facing_shading_normal_ws);
    let world_to_tangent: mat3x3<f32> = transpose(tangent_to_world);

    let front_facing_clearcoat_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(light_sample_ctx.front_facing_clearcoat_normal_ws, 0);
    let clearcoat_tangent_to_world: mat3x3<f32> = build_orthonormal_basis(front_facing_clearcoat_normal_ws);
    let clearcoat_world_to_tangent: mat3x3<f32> = transpose(clearcoat_tangent_to_world);

    let reservoir: GiReservoir = PackedGiReservoir::unpack(in_reservoirs[flat_id]);

    var combined_reservoir = GiReservoir::new();
    var combined_sample_count: f32 = reservoir.sample_count;
    GiReservoir::update(&combined_reservoir, reservoir.selected_phat * reservoir.contribution_weight * reservoir.sample_count, &rng, reservoir.sample_point_ws, reservoir.selected_phat, reservoir.phat_rng);

    let center_gbuffer_texel: GBufferTexel = gbuffer[flat_id];
    let center_depth_cs: f32 = GBufferTexel::depth_cs(center_gbuffer_texel, 0.001, 10000.0);
    let center_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(center_gbuffer_texel.normal_ws, 0);

    let center_id = vec2<i32>(i32(id.x), i32(id.y));
    var radius: f32 = (30.0 / 1920.0) * f32(constants.resolution.x); // TODO: 10 percent, half until 3 pixels min
    if (constants.spatial_pass_idx == 0) {
        radius *= 4.0;
    } else {
        radius *= 2.5;
    }
    let sampling_radius_offset: f32 = interleaved_gradient_noise_animated(id, constants.seed * 3 + constants.spatial_pass_idx);
    var pixel_seed: vec2<u32>;
    if (constants.spatial_pass_idx == 0) {
        pixel_seed = vec2<u32>(id.x >> 2, id.y >> 2);
    } else {
        pixel_seed = vec2<u32>(id.x >> 1, id.y >> 1);
    }
    let angle_seed: u32 = hash_combine(pixel_seed.x, hash_combine(pixel_seed.y, constants.seed * 3 + constants.spatial_pass_idx));
    let sampling_angle_offset: f32 = f32(angle_seed) * (1.0 / f32(0xFFFFFFFF)) * TWO_PI;

    for (var i: u32 = 0; i < NUM_SAMPLES; i += 1) {
        let angle: f32 = f32(i) * GOLDEN_ANGLE + sampling_angle_offset;
        let current_radius: f32 = pow(f32(i) / f32(NUM_SAMPLES), 0.5) * radius + sampling_radius_offset;
        let offset = vec2<i32>(current_radius * vec2<f32>(cos(angle), sin(angle)));
        let neighbour_id = mirror_pixel(center_id + offset);
        let flat_neighbour_id: u32 = neighbour_id.y * constants.resolution.x + neighbour_id.x;

        if (flat_neighbour_id == flat_id) {
            continue;
        }

        var valid_neighbour_reservoir: bool = true;
        let neighbour_gbuffer_texel: GBufferTexel = gbuffer[flat_neighbour_id];
        let neighbour_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(neighbour_gbuffer_texel.normal_ws, 0);
        if (constants.unbiased == 0) {
            let neighbour_depth_cs: f32 = GBufferTexel::depth_cs(neighbour_gbuffer_texel, 0.001, 10000.0);
            let valid_delta_depth: bool = (abs(center_depth_cs - neighbour_depth_cs) / center_depth_cs) < 0.1;
            let valid_delta_normal: bool = dot(center_normal_ws, neighbour_normal_ws) > 0.906; // 25 degrees

            valid_neighbour_reservoir = valid_delta_depth && valid_delta_normal;
        }

        if (valid_neighbour_reservoir) {
            var neighbour_reservoir: GiReservoir = PackedGiReservoir::unpack(in_reservoirs[flat_neighbour_id]);

            let w_out_worldspace: vec3<f32> = -direction;
            let w_in_worldspace: vec3<f32> = normalize(neighbour_reservoir.sample_point_ws - hit_point_ws);

            var shading_pdf: f32;
            let reflectance: vec3<f32> = DisneyBsdf::evaluate(disney_bsdf, front_facing_shading_normal_ws,
                tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
                w_out_worldspace, w_in_worldspace, &shading_pdf);
            var cos_in: f32 = abs(dot(w_in_worldspace, front_facing_shading_normal_ws));
            //cos_in *= jacobianDiffuse(center_gbuffer_texel.position_ws, neighbour_gbuffer_texel.position_ws, neighbour_normal_ws, w_in_worldspace, payload.t);

            if (cos_in > 0.0) {
                let local_throughput: vec3<f32> = cos_in * reflectance;
                let gi_origin: vec3<f32> = hit_point_ws + w_in_worldspace * 0.0001;
                let gi_direction: vec3<f32> = w_in_worldspace;
                var throughput_result: vec3<f32> = throughput * local_throughput;
                var phat_rng: u32 = neighbour_reservoir.phat_rng;
                var sample_point_ws: vec3<f32>;
                let contribution: vec3<f32> = InlinePathTracer::trace(gi_origin, gi_direction, RESTIR_GI_PHAT_MAX_BOUNCES, &throughput_result, &sample_point_ws, &phat_rng, scene);
                neighbour_reservoir.selected_phat = linear_to_luma(contribution);

                GiReservoir::update(&combined_reservoir, neighbour_reservoir.selected_phat * neighbour_reservoir.contribution_weight * neighbour_reservoir.sample_count, &rng, neighbour_reservoir.sample_point_ws, neighbour_reservoir.selected_phat, neighbour_reservoir.phat_rng);
                combined_sample_count += neighbour_reservoir.sample_count;
            }
        }
    }

    combined_reservoir.sample_count = combined_sample_count;
    if (combined_reservoir.selected_phat > 0.0) {
        combined_reservoir.contribution_weight = (1.0 / combined_reservoir.selected_phat) * (1.0 / combined_reservoir.sample_count * combined_reservoir.weight_sum);
    }

    out_reservoirs[flat_id] = PackedGiReservoir::new(combined_reservoir);
    // if (constants.spatial_pass_idx == constants.spatial_pass_count - 1) {
    //     prev_reservoirs[flat_id] = PackedGiReservoir::new(combined_reservoir);
    // }

    payload.rng = rng;
    payloads[flat_id] = payload;
}