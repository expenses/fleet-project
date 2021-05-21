#version 450

layout (location = 0) in vec2 uv;

layout(set = 0, binding = 0) uniform sampler u_sampler;
layout(set = 0, binding = 1) uniform texture2D u_texture;

layout (location = 0) out vec4 out_colour;

layout(push_constant) uniform TonemapperSettings {
    float a;
    float b;
    float c;
    float d;
    float crosstalk;
    float saturation;
    float cross_saturation;
} settings;

vec3 lerp(vec3 a, vec3 b, float factor) {
    return (1.0 - factor) * a + factor * b;
}

float tonemap_max(float x) {
    float z = pow(x, settings.a);
    return z / (pow(z, settings.d) * settings.b + settings.c);
}

vec3 tonemap(vec3 colour) {
    float colour_max = max(max(colour.r, colour.g), colour.b);
    vec3 ratio = colour / colour_max;
    float tonemapped_max = tonemap_max(colour_max);

    ratio = pow(ratio, vec3(settings.saturation / settings.cross_saturation));
    ratio = lerp(ratio, vec3(1.0), pow(tonemapped_max, settings.crosstalk));
    ratio = pow(ratio, vec3(settings.cross_saturation));

    return clamp(ratio * tonemapped_max, vec3(0.0), vec3(1.0));
}

void main() {
    vec3 colour = textureLod(sampler2D(u_texture, u_sampler), uv, 1.0).rgb;

    out_colour = vec4(tonemap(colour), 1.0);
}
