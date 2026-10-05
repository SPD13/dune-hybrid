#version 300 es
// SPDX-License-Identifier: Apache-2.0
// Final pass: resample to the canvas (nearest, sharp-bilinear or smooth),
// optional light scanlines, flip to screen orientation.
// The sharp-bilinear method follows Themaister's public-domain shader.
precision highp float;
uniform sampler2D uSrc;
uniform vec2 uSrcSize;
uniform vec2 uOutSize;
uniform int uMode;      // 0 nearest, 1 sharp, 2 smooth
uniform float uScan;    // scanline strength 0..1
in vec2 vUv;
out vec4 o;
void main() {
  vec2 uv = vec2(vUv.x, 1.0 - vUv.y);
  vec3 c;
  if (uMode == 0) {
    c = texelFetch(uSrc, ivec2(min(uv * uSrcSize, uSrcSize - 1.0)), 0).rgb;
  } else if (uMode == 1) {
    vec2 texel = uv * uSrcSize;
    vec2 scale = max(floor(uOutSize / uSrcSize), vec2(1.0));
    vec2 range = 0.5 - 0.5 / scale;
    vec2 cd = fract(texel) - 0.5;
    vec2 f = (cd - clamp(cd, -range, range)) * scale + 0.5;
    c = texture(uSrc, (floor(texel) + f) / uSrcSize).rgb;
  } else {
    c = texture(uSrc, uv).rgb;
  }
  if (uScan > 0.0) {
    // Beam profile across each of the game's 200 lines.
    float d = fract(uv.y * 200.0) - 0.5;
    float beam = exp(-d * d * 10.0);
    c *= mix(1.0, 0.45 + 0.7 * beam, uScan);
  }
  o = vec4(c, 1.0);
}
