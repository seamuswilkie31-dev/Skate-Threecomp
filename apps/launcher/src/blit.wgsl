@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vertex(@builtin(vertex_index) index: u32) -> Out {
    var points = array<vec2<f32>, 3>(vec2<f32>(-1.0,-1.0), vec2<f32>(3.0,-1.0), vec2<f32>(-1.0,3.0));
    var out: Out;
    out.position=vec4<f32>(points[index],0.0,1.0);
    out.uv=vec2<f32>(points[index].x*0.5+0.5,0.5-points[index].y*0.5);
    return out;
}
@fragment fn fragment(in: Out) -> @location(0) vec4<f32> { return textureSample(image,image_sampler,in.uv); }
