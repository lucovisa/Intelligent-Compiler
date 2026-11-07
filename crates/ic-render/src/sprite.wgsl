struct Camera {
    center: vec2<f32>,
    half: vec2<f32>,
};
@group(0) @binding(0) var<uniform> cam: Camera;

struct VsIn {
    @builtin(vertex_index) vi: u32,
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) rot: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-0.5, -0.5), vec2<f32>(0.5, -0.5), vec2<f32>(0.5, 0.5),
        vec2<f32>(-0.5, -0.5), vec2<f32>(0.5, 0.5), vec2<f32>(-0.5, 0.5),
    );
    let local = corners[in.vi] * in.size;
    let s = sin(in.rot);
    let c = cos(in.rot);
    let rotated = vec2<f32>(local.x * c - local.y * s, local.x * s + local.y * c);
    let world = in.pos + rotated;
    let ndc = (world - cam.center) / cam.half;

    var out: VsOut;
    out.clip = vec4<f32>(ndc, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
