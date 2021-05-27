#version 450

layout(location = 0) in vec4 colour;

layout(location = 0) out vec4 out_colour;
layout(location = 1) out vec4 bloom_colour;

void main() {
    out_colour = colour;
    bloom_colour = vec4(colour.rgb, 1.0);
}
