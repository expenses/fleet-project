#version 450

layout(location = 0) in vec3 in_normal;
layout(location = 1) in vec2 in_uv;

layout(set = 0, binding = 0) uniform sampler u_sampler;
layout(set = 0, binding = 1) uniform texture2D u_diffuse;
layout(set = 0, binding = 2) uniform texture2D u_emission;

layout(push_constant) uniform PushConstants {
    mat4 perspective_view;
    vec3 light_dir;
};

layout(location = 0) out vec4 colour;
layout(location = 1) out vec4 bloom;

float ambient_factor = 0.075;

void main() {
    vec3 normal = normalize(in_normal);
    float diffuse_factor = max(dot(normal, light_dir), 0.0);

    vec3 diffuse = texture(sampler2D(u_diffuse, u_sampler), in_uv).rgb;

    float emissive_factor = texture(sampler2D(u_emission, u_sampler), in_uv).r;

    float colour_factor = max(max(diffuse_factor, emissive_factor), ambient_factor);

    colour = vec4(colour_factor * diffuse, 1.0);
    bloom = vec4(emissive_factor * diffuse, 1.0);
}
