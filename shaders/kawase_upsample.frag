#version 450

layout (location = 0) in vec2 uv;

layout(set = 0, binding = 0) uniform sampler u_sampler;
layout(set = 0, binding = 1) uniform texture2D u_texture;

layout (location = 0) out vec4 colour;

layout(push_constant) uniform Settings {
    vec2 half_offset_per_pixel;
};

// https://community.arm.com/cfs-file/__key/communityserver-blogs-components-weblogfiles/00-00-00-20-66/siggraph2015_2D00_mmg_2D00_marius_2D00_notes.pdf
// from https://github.com/JujuAdams/Kawase
vec4 upsample(vec2 uv, vec2 halfpixel){
    vec4 sum = textureLod(sampler2D(u_texture, u_sampler), uv + vec2(-halfpixel.x * 2.0, 0.0), 0);
    sum += textureLod(sampler2D(u_texture, u_sampler), uv + vec2(-halfpixel.x, halfpixel.y), 0) * 2.0;
    sum += textureLod(sampler2D(u_texture, u_sampler), uv + vec2(0.0, halfpixel.y * 2.0), 0);
    sum += textureLod(sampler2D(u_texture, u_sampler), uv + vec2(halfpixel.x, halfpixel.y), 0) * 2.0;
    sum += textureLod(sampler2D(u_texture, u_sampler), uv + vec2(halfpixel.x * 2.0, 0.0), 0);
    sum += textureLod(sampler2D(u_texture, u_sampler), uv + vec2(halfpixel.x, -halfpixel.y), 0) * 2.0;
    sum += textureLod(sampler2D(u_texture, u_sampler), uv + vec2(0.0, -halfpixel.y * 2.0), 0);
    sum += textureLod(sampler2D(u_texture, u_sampler), uv + vec2(-halfpixel.x, -halfpixel.y), 0) * 2.0;

    return sum / 12.0;
}

void main() {
    colour = upsample(uv, half_offset_per_pixel);
}
