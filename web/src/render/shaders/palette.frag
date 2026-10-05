#version 300 es
// SPDX-License-Identifier: Apache-2.0
// Pass 1: 8-bit palette indices + 256-colour palette -> RGB (320x200).
// Intermediate targets keep image rows top-down (row y = framebuffer row y);
// only the final output pass flips to screen orientation.
precision highp float;
precision highp usampler2D;
uniform highp usampler2D uIndex;
uniform sampler2D uPal;
out vec4 o;
void main() {
  uint i = texelFetch(uIndex, ivec2(gl_FragCoord.xy), 0).r;
  o = vec4(texelFetch(uPal, ivec2(int(i), 0), 0).rgb, 1.0);
}
