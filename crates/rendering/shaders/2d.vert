#version 450

layout(location = 0) in vec2 pos;
layout(location = 1) in vec3 colour;

layout(location = 0) out vec4 out_colour;

void main() {
    gl_Position = vec4(pos, 0.0, 1.0);
    out_colour = vec4(colour, 1.0);
}
