#version 450
#extension GL_EXT_nonuniform_qualifier: enable

layout(location = 0) in vec3 in_normal;
layout(location = 1) in vec2 in_uv;
layout(location = 2) flat in uint in_diffuse_texture;
layout(location = 3) flat in uint in_emissive_texture;

layout(set = 0, binding = 0) uniform sampler u_sampler;
layout(set = 0, binding = 1) uniform texture2D u_textures[10];

layout(push_constant) uniform PushConstants {
    mat4 perspective_view;
    vec3 light_dir;
    vec3 ambient_light;
};

layout(location = 0) out vec4 colour;
layout(location = 1) out vec4 bloom;

float ambient_factor = 1.0 / 3.0;

void main() {
    vec3 normal = normalize(in_normal);

    float diffuse_factor = max(dot(normal, light_dir), 0.0);

    vec3 diffuse = texture(sampler2D(u_textures[in_diffuse_texture], u_sampler), in_uv).rgb;

    float emissive_factor = texture(sampler2D(u_textures[in_emissive_texture], u_sampler), in_uv).r;

    float colour_factor = max(diffuse_factor, emissive_factor);

    colour = vec4((vec3(colour_factor) + ambient_light) * diffuse, 1.0);
    bloom = vec4(emissive_factor * diffuse, 1.0);
}
