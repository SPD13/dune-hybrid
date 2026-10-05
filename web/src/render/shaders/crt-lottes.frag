#version 300 es
// PUBLIC DOMAIN CRT STYLED SCAN-LINE SHADER
//
//   by Timothy Lottes
//
// This is more along the style of a really good CGA arcade monitor.
// With RGB inputs instead of NTSC.
// The shadow mask example has the mask rotated 90 degrees for less chromatic aberration.
//
// Left it unoptimized to show the theory behind the algorithm.
//
// It is an example what I personally would want as a display option for pixel art games.
// Please take and use, change, or whatever.
//
// SPDX-License-Identifier: LicenseRef-Public-Domain
// Ported to GLSL ES 3.00 for dune-hybrid: texelFetch sampling of the
// 320x200 picture, bloom optional, uStrength blends with the plain picture.
precision highp float;
uniform sampler2D uSrc;   // the game picture, rows top-down
uniform vec2 uSrcSize;
uniform float uStrength;  // 0 = plain sharp picture, 1 = full effect
uniform int uBloom;
in vec2 vUv;
out vec4 o;

const float hardScan = -8.0;
const float hardPix = -3.0;
const float warpX = 0.031;
const float warpY = 0.041;
const float maskDark = 0.5;
const float maskLight = 1.5;
const float brightBoost = 1.0;
const float hardBloomPix = -1.5;
const float hardBloomScan = -2.0;
const float bloomAmount = 0.15;
const float shape = 2.0;

vec3 ToLinear(vec3 c) { return pow(c, vec3(2.2)); }
vec3 ToSrgb(vec3 c) { return pow(max(c, 0.0), vec3(1.0 / 2.2)); }

// Nearest emulated sample given floating point position and texel offset.
vec3 Fetch(vec2 pos, vec2 off) {
  ivec2 p = ivec2(floor(pos * uSrcSize + off));
  return ToLinear(brightBoost * texelFetch(uSrc, clamp(p, ivec2(0), ivec2(uSrcSize) - 1), 0).rgb);
}

// Distance in emulated pixels to nearest texel.
vec2 Dist(vec2 pos) {
  pos = pos * uSrcSize;
  return -((pos - floor(pos)) - vec2(0.5));
}

// 1D Gaussian.
float Gaus(float pos, float scale) { return exp2(scale * pow(abs(pos), shape)); }

// 3-tap Gaussian filter along horz line.
vec3 Horz3(vec2 pos, float off) {
  vec3 b = Fetch(pos, vec2(-1.0, off));
  vec3 c = Fetch(pos, vec2( 0.0, off));
  vec3 d = Fetch(pos, vec2( 1.0, off));
  float dst = Dist(pos).x;
  float wb = Gaus(dst - 1.0, hardPix);
  float wc = Gaus(dst + 0.0, hardPix);
  float wd = Gaus(dst + 1.0, hardPix);
  return (b*wb + c*wc + d*wd) / (wb + wc + wd);
}

// 5-tap Gaussian filter along horz line.
vec3 Horz5(vec2 pos, float off) {
  vec3 a = Fetch(pos, vec2(-2.0, off));
  vec3 b = Fetch(pos, vec2(-1.0, off));
  vec3 c = Fetch(pos, vec2( 0.0, off));
  vec3 d = Fetch(pos, vec2( 1.0, off));
  vec3 e = Fetch(pos, vec2( 2.0, off));
  float dst = Dist(pos).x;
  float wa = Gaus(dst - 2.0, hardPix);
  float wb = Gaus(dst - 1.0, hardPix);
  float wc = Gaus(dst + 0.0, hardPix);
  float wd = Gaus(dst + 1.0, hardPix);
  float we = Gaus(dst + 2.0, hardPix);
  return (a*wa + b*wb + c*wc + d*wd + e*we) / (wa + wb + wc + wd + we);
}

// 7-tap Gaussian filter along horz line.
vec3 Horz7(vec2 pos, float off) {
  vec3 a = Fetch(pos, vec2(-3.0, off));
  vec3 b = Fetch(pos, vec2(-2.0, off));
  vec3 c = Fetch(pos, vec2(-1.0, off));
  vec3 d = Fetch(pos, vec2( 0.0, off));
  vec3 e = Fetch(pos, vec2( 1.0, off));
  vec3 f = Fetch(pos, vec2( 2.0, off));
  vec3 g = Fetch(pos, vec2( 3.0, off));
  float dst = Dist(pos).x;
  float wa = Gaus(dst - 3.0, hardBloomPix);
  float wb = Gaus(dst - 2.0, hardBloomPix);
  float wc = Gaus(dst - 1.0, hardBloomPix);
  float wd = Gaus(dst + 0.0, hardBloomPix);
  float we = Gaus(dst + 1.0, hardBloomPix);
  float wf = Gaus(dst + 2.0, hardBloomPix);
  float wg = Gaus(dst + 3.0, hardBloomPix);
  return (a*wa + b*wb + c*wc + d*wd + e*we + f*wf + g*wg) / (wa + wb + wc + wd + we + wf + wg);
}

// Return scanline weight.
float Scan(vec2 pos, float off) { return Gaus(Dist(pos).y + off, hardScan); }

// Return scanline weight for bloom.
float BloomScan(vec2 pos, float off) { return Gaus(Dist(pos).y + off, hardBloomScan); }

// Allow nearest three lines to effect pixel.
vec3 Tri(vec2 pos) {
  vec3 a = Horz3(pos, -1.0);
  vec3 b = Horz5(pos,  0.0);
  vec3 c = Horz3(pos,  1.0);
  return a*Scan(pos, -1.0) + b*Scan(pos, 0.0) + c*Scan(pos, 1.0);
}

// Small bloom.
vec3 Bloom(vec2 pos) {
  vec3 a = Horz5(pos, -2.0);
  vec3 b = Horz7(pos, -1.0);
  vec3 c = Horz7(pos,  0.0);
  vec3 d = Horz7(pos,  1.0);
  vec3 e = Horz5(pos,  2.0);
  return a*BloomScan(pos, -2.0) + b*BloomScan(pos, -1.0) + c*BloomScan(pos, 0.0)
       + d*BloomScan(pos, 1.0) + e*BloomScan(pos, 2.0);
}

// Distortion of scanlines, and end of screen alpha.
vec2 Warp(vec2 pos) {
  pos = pos * 2.0 - 1.0;
  pos *= vec2(1.0 + (pos.y*pos.y)*warpX, 1.0 + (pos.x*pos.x)*warpY);
  return pos * 0.5 + 0.5;
}

// Stretched VGA style shadow mask.
vec3 Mask(vec2 pos) {
  vec3 mask = vec3(maskDark);
  pos.x += pos.y * 3.0;
  pos.x = fract(pos.x * 0.166666666);
  if (pos.x < 0.333) mask.r = maskLight;
  else if (pos.x < 0.666) mask.g = maskLight;
  else mask.b = maskLight;
  return mask;
}

void main() {
  vec2 uv = vec2(vUv.x, 1.0 - vUv.y);
  vec2 pos = Warp(uv);
  vec3 c = Tri(pos);
  if (uBloom != 0) c += Bloom(pos) * bloomAmount;
  c *= Mask(gl_FragCoord.xy * 1.000001);
  if (pos.x <= 0.0001 || pos.x >= 0.9999 || pos.y <= 0.0001 || pos.y >= 0.9999) c = vec3(0.0);
  vec3 crt = ToSrgb(c);
  vec3 plain = texelFetch(uSrc, ivec2(min(uv * uSrcSize, uSrcSize - 1.0)), 0).rgb;
  o = vec4(mix(plain, crt, uStrength), 1.0);
}
