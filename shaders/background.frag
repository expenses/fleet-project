#version 450

layout(location = 0) in vec3 colour;

layout(location = 0) out vec4 out_colour;
layout(location = 1) out vec4 out_bloom;
layout(location = 2) out vec4 out_god_rays;

void main() {
    out_colour = vec4(colour, 1.0);
    out_bloom = vec4(colour * colour, 1.0);
    out_god_rays = vec4(vec3(greaterThan(colour, vec3(1.0))), 1.0);
}
