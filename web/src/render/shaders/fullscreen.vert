#version 300 es
// SPDX-License-Identifier: Apache-2.0
// Full-screen triangle; vUv spans 0..1 over the viewport.
out vec2 vUv;
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  vUv = p;
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}
