#version 460

layout(push_constant) uniform PushConstants {
  mat4 view;
  mat4 proj;
  vec3 camera_pos;
}
pc;

layout(set = 0, binding = 0) uniform ModelBuffer { mat4 model; };

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 tex_coord; // not used
layout(location = 0) out vec3 fragPos;
layout(location = 1) out vec3 fragNormal;

void main() {
  fragPos = vec3(model * vec4(position, 1.0));
  fragNormal = mat3(transpose(inverse(model))) * normal;
  gl_Position = pc.proj * pc.view * vec4(fragPos, 1.0);
}
