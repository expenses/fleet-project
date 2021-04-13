#version 450

layout (location = 0) in vec2 uv;

layout(set = 0, binding = 0) uniform sampler u_sampler;
layout(set = 0, binding = 1) uniform texture2D u_texture;

layout(push_constant) uniform BlurSettings {
    float blur_scale;
    float blur_strength;
    int blur_direction;
};

layout (location = 0) out vec4 colour;

void main() {
	float weights[5] = {
	    0.227027,
	    0.1945946,
	    0.1216216,
	    0.054054,
	    0.016216,
    };

 	// Get size of single texel
	vec2 tex_offset = 1.0 / textureSize(sampler2D(u_texture, u_sampler), 0) * blur_scale;
	// Current fragment's contribution
	vec3 result = textureLod(sampler2D(u_texture, u_sampler), uv, 0).rgb * weights[0];
	for (int i = 1; i < 5; ++i) {
        float blur_weight = weights[i] * blur_strength;

        vec2 offset = tex_offset * i;
        offset[blur_direction] = 0.0;

		result += textureLod(sampler2D(u_texture, u_sampler), uv + offset, 0).rgb * blur_weight;
		result += textureLod(sampler2D(u_texture, u_sampler), uv - offset, 0).rgb * blur_weight;
	}
	colour = vec4(result, 1.0);
}
