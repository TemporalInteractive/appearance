@include ::random
@include appearance-packing::shared/packing

const RESTIR_GI_PHAT_MAX_BOUNCES: u32 = 1;

struct GiReservoir {
    sample_count: f32,
    contribution_weight: f32,
    weight_sum: f32,
    selected_phat: f32,
    sample_point_ws: vec3<f32>,
    phat_rng: u32,
}

struct PackedGiReservoir {
    sample_count: f32,
    contribution_weight: f32,
    weight_sum: f32,
    selected_phat: f32,
    sample_point_ws: vec3<f32>,
    phat_rng: u32,
}

fn GiReservoir::new() -> GiReservoir {
    return GiReservoir(0.0, 0.0, 0.0, 0.0, vec3<f32>(0.0), 0);
}

fn PackedGiReservoir::new(gi_reservoir: GiReservoir) -> PackedGiReservoir {
    return PackedGiReservoir(
        gi_reservoir.sample_count,
        gi_reservoir.contribution_weight,
        gi_reservoir.weight_sum,
        gi_reservoir.selected_phat,
        gi_reservoir.sample_point_ws,
        gi_reservoir.phat_rng,
    );
}

fn PackedGiReservoir::unpack(_self: PackedGiReservoir) -> GiReservoir {
    return GiReservoir(
        _self.sample_count,
        _self.contribution_weight,
        _self.weight_sum,
        _self.selected_phat,
        _self.sample_point_ws,
        _self.phat_rng
    );
}

fn GiReservoir::update(_self: ptr<function, GiReservoir>, sample_weight: f32, rng: ptr<function, u32>, sample_point_ws: vec3<f32>, phat: f32, phat_rng: u32) -> bool {
    (*_self).weight_sum += sample_weight;
    (*_self).sample_count += 1.0;

    if (random_uniform_float(rng) <= (sample_weight / (*_self).weight_sum)) {
        (*_self).sample_point_ws = sample_point_ws;
        (*_self).selected_phat = phat;
        (*_self).phat_rng = phat_rng;
        return true;
    }
    return false;
}

fn GiReservoir::combine(r1: GiReservoir, r2: GiReservoir, rng: ptr<function, u32>) -> GiReservoir {
    var combined_reservoir = GiReservoir::new();
    GiReservoir::update(&combined_reservoir, r1.selected_phat * r1.contribution_weight * r1.sample_count, rng, r1.sample_point_ws, r1.selected_phat, r1.phat_rng);
    GiReservoir::update(&combined_reservoir, r2.selected_phat * r2.contribution_weight * r2.sample_count, rng, r2.sample_point_ws, r2.selected_phat, r2.phat_rng);
    combined_reservoir.sample_count = r1.sample_count + r2.sample_count;

    if (combined_reservoir.selected_phat > 0.0 && combined_reservoir.sample_count * combined_reservoir.weight_sum > 0.0) {
        combined_reservoir.contribution_weight = (1.0 / combined_reservoir.selected_phat) * (1.0 / combined_reservoir.sample_count * combined_reservoir.weight_sum);
    }

    return combined_reservoir;
}

// https://dl.acm.org/doi/pdf/10.1145/2766997 at section 5 under "Jacobians"
// Calculate geometric ratio from base path X (this is the pixel you are sampling from) to the
// neighbor Y that wants to reuse X.
// https://d1qx31qr3h6wln.cloudfront.net/publications/ReSTIR%20GI.pdf
// Jacobian is in form of 1 / X to make handling zero easier.
fn _jacobianDiffuse(surfaceNormal: vec3<f32>, incidentDirX: vec3<f32>, incidentDirY: vec3<f32>,
                    squaredDistX: f32, squaredDistY: f32) -> f32 {
    const kJacobianRejection: f32 = 1e-2;

    let cosThetaX: f32 = abs(dot(surfaceNormal, incidentDirX));
    let cosThetaY: f32 = abs(dot(surfaceNormal, incidentDirY));

    let jacobian: f32 = (cosThetaY * squaredDistX) / (cosThetaX * squaredDistY);

    if (jacobian < kJacobianRejection || cosThetaY <= 0) {
        return 0.0;
    }

    return jacobian;
}

fn jacobianDiffuse(targetOrigin: vec3<f32>, sampleOrigin: vec3<f32>, sampleHitNormal: vec3<f32>, sampleDirection: vec3<f32>, hitT: f32) -> f32 {
    let sampleHitPos: vec3<f32> = sampleOrigin + sampleDirection * hitT;
    let relativeSamplePosFromTarget: vec3<f32> = sampleHitPos - targetOrigin;
    let targetDirection: vec3<f32> = normalize(relativeSamplePosFromTarget);
    let sampleDistSquared: f32 = hitT * hitT;
    let targetDistSquared: f32 = dot(relativeSamplePosFromTarget, relativeSamplePosFromTarget);

    return _jacobianDiffuse(sampleHitNormal, -sampleDirection, -targetDirection, sampleDistSquared, targetDistSquared);
}