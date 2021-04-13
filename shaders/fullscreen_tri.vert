#version 450

layout (location = 0) out vec2 out_uv;

void main() {
    uint vertex_index = gl_VertexIndex;
    out_uv = vec2((vertex_index << 1) & 2, vertex_index & 2);
    gl_Position = vec4(out_uv * 2.0 + -1.0, 0.0, 1.0);
    out_uv.y = 1.0 - out_uv.y;
}
