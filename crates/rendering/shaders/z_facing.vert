#version 450

layout(location = 0) in vec2 position;

layout(location = 1) in vec3 translation;
layout(location = 2) in float scale;
layout(location = 3) in vec4 colour;

layout(push_constant) uniform PushConstants {
    mat4 perspective;
    mat4 view;
};

layout(location = 0) out vec4 out_colour;

void main() {
    vec3 view_space_center = (view * vec4(translation, 1.0)).xyz;

    vec3 adjusted_position = vec3(position.x, position.y, 0.0) * scale + view_space_center;
    gl_Position = perspective * vec4(adjusted_position, 1.0);

    out_colour = colour;
}
