#version 300 es
/*
   Hyllian's xBR-lv2 Shader

   Copyright (C) 2011-2016 Hyllian - sergiogdb@gmail.com

   Permission is hereby granted, free of charge, to any person obtaining a copy
   of this software and associated documentation files (the "Software"), to deal
   in the Software without restriction, including without limitation the rights
   to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
   copies of the Software, and to permit persons to whom the Software is
   furnished to do so, subject to the following conditions:

   The above copyright notice and this permission notice shall be included in
   all copies or substantial portions of the Software.

   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
   FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
   AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
   OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
   THE SOFTWARE.

   Incorporates some of the ideas from SABR shader. Thanks to Joshua Street.

   SPDX-License-Identifier: MIT
   Ported to GLSL ES 3.00 (texelFetch neighbourhood, scale as a uniform,
   CORNER_C + SMOOTH_TIPS, small_details off) for dune-hybrid, with an
   amount control: the game's 4-pixel-wide font loses its holes ("a", "e")
   under full-strength xBR, so the result can be blended with the source.
*/
precision highp float;
uniform sampler2D uSrc;
uniform ivec2 uSrcSize;
uniform float uScale;
uniform float uAmount; // 1 = full xBR; lower keeps more of the original pixel
out vec4 o;

#define XBR_EQ_THRESHOLD 15.0
#define lv2_cf 2.0

const vec3 rgbw = vec3(14.352, 28.176, 5.472);
const vec4 Ao = vec4( 1.0, -1.0, -1.0, 1.0 );
const vec4 Bo = vec4( 1.0,  1.0, -1.0,-1.0 );
const vec4 Co = vec4( 1.5,  0.5, -0.5, 0.5 );
const vec4 Ax = vec4( 1.0, -1.0, -1.0, 1.0 );
const vec4 Bx = vec4( 0.5,  2.0, -0.5,-2.0 );
const vec4 Cx = vec4( 1.0,  1.0, -0.5, 0.0 );
const vec4 Ay = vec4( 1.0, -1.0, -1.0, 1.0 );
const vec4 By = vec4( 2.0,  0.5, -2.0,-0.5 );
const vec4 Cy = vec4( 2.0,  0.0, -1.0, 0.5 );
const vec4 Ci = vec4(0.25);

vec3 at(ivec2 p) { return texelFetch(uSrc, clamp(p, ivec2(0), uSrcSize - 1), 0).rgb; }
vec4 df(vec4 A, vec4 B) { return abs(A - B); }
vec4 diff(vec4 A, vec4 B) { return vec4(notEqual(A, B)); }
vec4 eq(vec4 A, vec4 B) { return step(df(A, B), vec4(XBR_EQ_THRESHOLD)); }
vec4 neq(vec4 A, vec4 B) { return vec4(1.0) - eq(A, B); }
vec4 wd(vec4 a, vec4 b, vec4 c, vec4 d, vec4 e, vec4 f, vec4 g, vec4 h) {
  return df(a,b) + df(a,c) + df(d,e) + df(d,f) + 4.0*df(g,h);
}
float c_df(vec3 c1, vec3 c2) { vec3 d = abs(c1 - c2); return d.r + d.g + d.b; }

void main() {
  vec2 sp = gl_FragCoord.xy / uScale;
  ivec2 p = ivec2(floor(sp));
  vec2 fp = fract(sp);

  vec4 delta   = vec4(1.0 / uScale);
  vec4 delta_l = vec4(0.5 / uScale, 1.0 / uScale, 0.5 / uScale, 1.0 / uScale);
  vec4 delta_u = delta_l.yxwz;

  vec3 A1 = at(p + ivec2(-1,-2)), B1 = at(p + ivec2( 0,-2)), C1 = at(p + ivec2( 1,-2));
  vec3 A  = at(p + ivec2(-1,-1)), B  = at(p + ivec2( 0,-1)), C  = at(p + ivec2( 1,-1));
  vec3 D  = at(p + ivec2(-1, 0)), E  = at(p),                F  = at(p + ivec2( 1, 0));
  vec3 G  = at(p + ivec2(-1, 1)), H  = at(p + ivec2( 0, 1)), I  = at(p + ivec2( 1, 1));
  vec3 G5 = at(p + ivec2(-1, 2)), H5 = at(p + ivec2( 0, 2)), I5 = at(p + ivec2( 1, 2));
  vec3 A0 = at(p + ivec2(-2,-1)), D0 = at(p + ivec2(-2, 0)), G0 = at(p + ivec2(-2, 1));
  vec3 C4 = at(p + ivec2( 2,-1)), F4 = at(p + ivec2( 2, 0)), I4 = at(p + ivec2( 2, 1));

  vec4 b = vec4(dot(B,rgbw), dot(D,rgbw), dot(H,rgbw), dot(F,rgbw));
  vec4 c = vec4(dot(C,rgbw), dot(A,rgbw), dot(G,rgbw), dot(I,rgbw));
  vec4 d = b.yzwx;
  vec4 e = vec4(dot(E,rgbw));
  vec4 f = b.wxyz;
  vec4 g = c.zwxy;
  vec4 h = b.zwxy;
  vec4 i = c.wxyz;

  vec4 i4 = vec4(dot(I4,rgbw), dot(C1,rgbw), dot(A0,rgbw), dot(G5,rgbw));
  vec4 i5 = vec4(dot(I5,rgbw), dot(C4,rgbw), dot(A1,rgbw), dot(G0,rgbw));
  vec4 h5 = vec4(dot(H5,rgbw), dot(F4,rgbw), dot(B1,rgbw), dot(D0,rgbw));
  vec4 f4 = h5.yzwx;

  vec4 fx   = Ao*fp.y + Bo*fp.x;
  vec4 fx_l = Ax*fp.y + Bx*fp.x;
  vec4 fx_u = Ay*fp.y + By*fp.x;

  vec4 irlv0 = diff(e,f) * diff(e,h);
  vec4 irlv1 = irlv0 * (neq(f,b) * neq(f,c) + neq(h,d) * neq(h,g)
             + eq(e,i) * (neq(f,f4) * neq(f,i4) + neq(h,h5) * neq(h,i5)) + eq(e,g) + eq(e,c));
  vec4 irlv2l = diff(e,g) * diff(d,g);
  vec4 irlv2u = diff(e,c) * diff(b,c);

  vec4 fx45i = clamp((fx   + delta   - Co - Ci) / (2.0*delta  ), 0.0, 1.0);
  vec4 fx45  = clamp((fx   + delta   - Co     ) / (2.0*delta  ), 0.0, 1.0);
  vec4 fx30  = clamp((fx_l + delta_l - Cx     ) / (2.0*delta_l), 0.0, 1.0);
  vec4 fx60  = clamp((fx_u + delta_u - Cy     ) / (2.0*delta_u), 0.0, 1.0);

  vec4 wd1 = wd(e, c, g, i, h5, f4, h, f);
  vec4 wd2 = wd(h, d, i5, f, i4, b, e, i);

  vec4 edri  = step(wd1, wd2) * irlv0;
  vec4 edr   = step(wd1 + vec4(0.1), wd2) * step(vec4(0.5), irlv1);
  vec4 edr_l = step(lv2_cf*df(f,g), df(h,c)) * irlv2l * edr;
  vec4 edr_u = step(lv2_cf*df(h,c), df(f,g)) * irlv2u * edr;

  fx45  = edr   * fx45;
  fx30  = edr_l * fx30;
  fx60  = edr_u * fx60;
  fx45i = edri  * fx45i;

  vec4 px = step(df(e,f), df(e,h));
  vec4 maximos = max(max(fx30, fx60), max(fx45, fx45i));

  vec3 res1 = E;
  res1 = mix(res1, mix(H, F, px.x), maximos.x);
  res1 = mix(res1, mix(B, D, px.z), maximos.z);
  vec3 res2 = E;
  res2 = mix(res2, mix(F, B, px.y), maximos.y);
  res2 = mix(res2, mix(D, H, px.w), maximos.w);

  vec3 res = mix(res1, res2, step(c_df(E, res1), c_df(E, res2)));
  o = vec4(mix(E, res, uAmount), 1.0);
}
