#version 450

layout (location = 0) in vec2 uv;

layout(set = 0, binding = 0) uniform sampler u_sampler;
layout(set = 0, binding = 1) uniform texture2D u_texture;

layout (location = 0) out vec4 colour;

void main() {
    colour = textureLod(sampler2D(u_texture, u_sampler), uv, 0);
}
