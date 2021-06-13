#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;

layout(location = 3) in vec3 rotation_1;
layout(location = 4) in vec3 rotation_2;
layout(location = 5) in vec3 rotation_3;
layout(location = 6) in vec3 translation;
layout(location = 7) in vec3 colour;
layout(location = 8) in float scale;
layout(location = 9) in uint diffuse_texture;
layout(location = 10) in uint emissive_texture;

layout(push_constant) uniform PushConstants {
    mat4 perspective_view;
    vec3 light_dir;
    vec3 ambient_light;
};

layout(location = 0) out vec3 out_normal;
layout(location = 1) out vec2 out_uv;
layout(location = 2) out uint out_diffuse_texture;
layout(location = 3) out uint out_emissive_texture;


void main() {
    mat3 rotation = mat3(rotation_1, rotation_2, rotation_3);

    vec3 transformed_position = rotation * position * scale + translation;
    gl_Position = perspective_view * vec4(transformed_position, 1.0);

    out_normal = rotation * normal;
    out_uv = uv;
    out_diffuse_texture = diffuse_texture;
    out_emissive_texture = emissive_texture;
}
