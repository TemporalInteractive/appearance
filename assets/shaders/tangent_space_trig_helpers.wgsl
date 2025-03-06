@include ::math

fn cosThetaTangentSpace(wTangentSpace: vec3<f32>) -> f32 { return wTangentSpace.z; }

fn cos2ThetaTangentSpace(wTangentSpace: vec3<f32>) -> f32 { return sqr(wTangentSpace.z); }

fn absCosThetaTangentSpace(wTangentSpace: vec3<f32>) -> f32 { return abs(wTangentSpace.z); }

fn sin2ThetaFromCos2Theta(cos2Theta: f32) -> f32 { return max(0.0, 1.0 - cos2Theta); }

fn sin2ThetaTangentSpace(wTangentSpace: vec3<f32>) -> f32 {
    return max(0.0, 1.0 - cos2ThetaTangentSpace(wTangentSpace));
}

fn sin2ThetaTangentSpaceIntermediate(wTangentSpace: vec3<f32>, cos2Theta: ptr<function, f32>) -> f32 {
    *cos2Theta = cos2ThetaTangentSpace(wTangentSpace);
    return max(0.0, 1.0 - *cos2Theta);
}

fn sinThetaTangentSpace(wTangentSpace: vec3<f32>) -> f32 {
    return sqrt(sin2ThetaTangentSpace(wTangentSpace));
}

fn tanThetaTangentSpace(wTangentSpace: vec3<f32>) -> f32 {
    return sinThetaTangentSpace(wTangentSpace) / cosThetaTangentSpace(wTangentSpace);
}

fn tan2ThetaTangentSpace(wTangentSpace: vec3<f32>) -> f32 {
    var cos2Theta: f32;
    let sin2Theta: f32 = sin2ThetaTangentSpaceIntermediate(wTangentSpace, &cos2Theta);
    return sin2Theta / cos2Theta;
}

fn tan2ThetaTangentSpaceIntermediate(wTangentSpace: vec3<f32>, cos2Theta: ptr<function, f32>) -> f32 {
    let sin2Theta: f32 = sin2ThetaTangentSpaceIntermediate(wTangentSpace, cos2Theta);
    return sin2Theta / *cos2Theta;
}

fn cosPhiTangentSpace(wTangentSpace: vec3<f32>) -> f32 {
    let sinTheta: f32 = sinThetaTangentSpace(wTangentSpace);
    return select(clamp(wTangentSpace.x / sinTheta, -1.0, 1.0), 1.0, sinTheta == 0.0);
}

fn sinPhiTangentSpace(wTangentSpace: vec3<f32>) -> f32 {
    let sinTheta: f32 = sinThetaTangentSpace(wTangentSpace);
    return select(clamp(wTangentSpace.y / sinTheta, -1.0, 1.0), 0.0, sinTheta == 0.0);
}