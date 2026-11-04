struct Camera { eye: vec4<f32>, right: vec4<f32>, up: vec4<f32>, forward: vec4<f32>, projection: vec4<f32> }
@group(0) @binding(0) var<uniform> camera: Camera;
struct Out { @builtin(position) position: vec4<f32>, @location(0) color: vec3<f32> }
@vertex fn vertex(@location(0) position: vec3<f32>, @location(1) color: vec3<f32>) -> Out {
    let delta = position - camera.eye.xyz;
    let depth = dot(delta, camera.forward.xyz);
    let near = camera.projection.z;
    let far = camera.projection.w;
    var out: Out;
    out.position = vec4<f32>(dot(delta, camera.right.xyz)*camera.projection.x,
        dot(delta, camera.up.xyz)*camera.projection.y,
        depth*far/(far-near)-far*near/(far-near), depth);
    out.color = color;
    return out;
}
@fragment fn fragment(in: Out) -> @location(0) vec4<f32> { return vec4<f32>(in.color, 1.0); }
