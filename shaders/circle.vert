#version 450

layout(location = 0) in vec2 position;

layout(location = 1) in vec3 translation;
layout(location = 2) in float scale;
layout(location = 3) in vec4 colour;

layout(push_constant) uniform PushConstants {
    mat4 perspective_view;
};

layout(location = 0) out vec4 out_colour;

void main() {
    vec3 adjusted_position = vec3(position.x, 0.0, position.y) * scale + translation;
    gl_Position = perspective_view * vec4(adjusted_position, 1.0);

    out_colour = colour;
}
