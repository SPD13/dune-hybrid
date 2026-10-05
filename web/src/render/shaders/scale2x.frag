#version 300 es
// SPDX-License-Identifier: Apache-2.0
// Scale2x / EPX (Andrea Mazzoleni's AdvMAME2x rules, a public algorithm):
// doubles resolution using only source colours, so palette art stays crisp.
// Run twice for Scale4x.
precision highp float;
uniform sampler2D uSrc;
uniform ivec2 uSrcSize;
out vec4 o;
vec3 at(ivec2 p) {
  return texelFetch(uSrc, clamp(p, ivec2(0), uSrcSize - 1), 0).rgb;
}
void main() {
  ivec2 q = ivec2(gl_FragCoord.xy);
  ivec2 p = q / 2;
  ivec2 s = q - p * 2;
  vec3 E = at(p);
  vec3 B = at(p + ivec2(0, -1));
  vec3 H = at(p + ivec2(0, 1));
  vec3 D = at(p + ivec2(-1, 0));
  vec3 F = at(p + ivec2(1, 0));
  vec3 r = E;
  if (B != H && D != F) {
    if (s == ivec2(0, 0)) r = D == B ? D : E;
    else if (s == ivec2(1, 0)) r = B == F ? F : E;
    else if (s == ivec2(0, 1)) r = D == H ? D : E;
    else r = H == F ? F : E;
  }
  o = vec4(r, 1.0);
}
