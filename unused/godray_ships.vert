#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;

layout(location = 3) in vec3 rotation_1;
layout(location = 4) in vec3 rotation_2;
layout(location = 5) in vec3 rotation_3;
layout(location = 6) in vec3 translation;

layout(push_constant) uniform PushConstants {
    mat4 perspective_view;
    vec3 light_dir;
};

layout(location = 0) out vec3 out_colour;

void main() {
    mat3 rotation = mat3(rotation_1, rotation_2, rotation_3);

    vec3 transformed_position = rotation * position + translation;
    gl_Position = perspective_view * vec4(transformed_position, 1.0);

    out_colour = vec3(0.0);
}
