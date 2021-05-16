#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 colour;

layout(push_constant) uniform PushConstants {
    mat4 perspective_view;
};

layout(location = 0) out vec4 out_colour;

void main() {
    gl_Position = perspective_view * vec4(position, 1.0);
    out_colour = vec4(colour, 1.0);
}
