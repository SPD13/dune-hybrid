#version 300 es
// SPDX-License-Identifier: Apache-2.0
// HD sprites and text: resolve the HD screen (k× texels from the worker's
// compositor: R, G = palette indices a, b; B = weight of b; A = 1 for HD,
// 0 where the pixel falls back) with the live palette. Fallback pixels come
// from the low-resolution picture (after the optional upscaler).
// True-colour texels (A = 128/255): R = palette index i, G, B = the red and
// green of i's reference colour; uTc: RGB = the art's colour, A = the
// reference's blue. Shown as colour × palette[i] ÷ reference per channel
// (`gfx::compose::shade`), so they follow fades.
precision highp float;
uniform sampler2D uHd;
uniform sampler2D uPal;
uniform sampler2D uLow;
uniform sampler2D uTc;
uniform ivec2 uLowSize;
uniform vec2 uHdSize;
uniform int uOverlay;
out vec4 o;

// Below this a reference channel is too dark to divide by: shift instead.
const float DARK = 24.0 / 255.0;

float shade(float art, float now, float rf) {
  return rf >= DARK ? min(1.0, art * now / rf) : clamp(art + now - rf, 0.0, 1.0);
}

void main() {
  ivec2 p = ivec2(gl_FragCoord.xy);
  vec4 t = texelFetch(uHd, p, 0);
  if (t.a > 0.75) {
    vec3 a = texelFetch(uPal, ivec2(int(t.r * 255.0 + 0.5), 0), 0).rgb;
    vec3 b = texelFetch(uPal, ivec2(int(t.g * 255.0 + 0.5), 0), 0).rgb;
    o = vec4(mix(a, b, t.b), 1.0);
    if (uOverlay != 0) o.g = min(1.0, o.g + 0.15);
  } else if (t.a > 0.25) {
    vec4 c = texelFetch(uTc, p, 0);
    vec3 now = texelFetch(uPal, ivec2(int(t.r * 255.0 + 0.5), 0), 0).rgb;
    o = vec4(shade(c.r, now.r, t.g), shade(c.g, now.g, t.b), shade(c.b, now.b, c.a), 1.0);
    if (uOverlay != 0) o.b = min(1.0, o.b + 0.25);
  } else {
    ivec2 q = ivec2((vec2(p) + 0.5) / uHdSize * vec2(uLowSize));
    o = vec4(texelFetch(uLow, min(q, uLowSize - 1), 0).rgb, 1.0);
    if (uOverlay != 0) o.r = min(1.0, o.r + 0.25);
  }
}
