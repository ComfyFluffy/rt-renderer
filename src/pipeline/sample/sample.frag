#version 460

layout(location = 0) in vec3 fragPos;
layout(location = 1) in vec3 fragNormal;
layout(location = 0) out vec4 outColor;

layout(set = 1, binding = 0) uniform Material {
  vec3 ambient;
  vec3 diffuse;
  vec3 specular;
  float shininess;
}
material;
layout(set = 1, binding = 1) uniform Light {
  vec3 position;
  vec3 ambient;
  vec3 diffuse;
  vec3 specular;
}
light;

layout(push_constant) uniform PushConstants {
  mat4 view;
  mat4 proj;
  vec3 camera_pos;
}
pc;

void main() {
  // Ambient
  vec3 ambient = light.ambient * material.ambient;

  // Diffuse
  vec3 norm = normalize(fragNormal);
  vec3 lightDir = normalize(light.position - fragPos);
  float diff = max(dot(norm, lightDir), 0.0);
  vec3 diffuse = light.diffuse * (diff * material.diffuse);

  // Specular
  vec3 viewDir = normalize(pc.camera_pos - fragPos);
  vec3 reflectDir = reflect(-lightDir, norm);
  float spec = pow(max(dot(viewDir, reflectDir), 0.0), material.shininess);
  vec3 specular = light.specular * (spec * material.specular);

  vec3 result = ambient + diffuse + specular;
  outColor = vec4(result, 1.0);
}
