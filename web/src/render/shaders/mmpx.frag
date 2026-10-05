#version 300 es
// MMPX
// by Morgan McGuire and Mara Gagiu
// https://casual-effects.com/research/McGuire2021PixelArt/
// License: MIT
// adapted for glsl by hunterk
//
// SPDX-License-Identifier: MIT
// Ported to GLSL ES 3.00 (texelFetch neighbourhood, one 2x pass) for
// dune-hybrid. Keeps the source colours only, so palette art stays crisp.
precision highp float;
uniform sampler2D uSrc;
uniform ivec2 uSrcSize;
out vec4 o;

ivec2 P0;
vec3 src(int c, int d) { return texelFetch(uSrc, clamp(P0 + ivec2(c, d), ivec2(0), uSrcSize - 1), 0).rgb; }

float luma(vec3 col) { return dot(col, vec3(0.2126, 0.7152, 0.0722)); }
bool same(vec3 B, vec3 A0) { return all(equal(B, A0)); }
bool notsame(vec3 B, vec3 A0) { return any(notEqual(B, A0)); }
bool all_eq2(vec3 B, vec3 A0, vec3 A1) { return same(B,A0) && same(B,A1); }
bool all_eq3(vec3 B, vec3 A0, vec3 A1, vec3 A2) { return same(B,A0) && same(B,A1) && same(B,A2); }
bool all_eq4(vec3 B, vec3 A0, vec3 A1, vec3 A2, vec3 A3) { return same(B,A0) && same(B,A1) && same(B,A2) && same(B,A3); }
bool any_eq3(vec3 B, vec3 A0, vec3 A1, vec3 A2) { return same(B,A0) || same(B,A1) || same(B,A2); }
bool none_eq2(vec3 B, vec3 A0, vec3 A1) { return notsame(B,A0) && notsame(B,A1); }
bool none_eq4(vec3 B, vec3 A0, vec3 A1, vec3 A2, vec3 A3) { return notsame(B,A0) && notsame(B,A1) && notsame(B,A2) && notsame(B,A3); }

void main() {
  ivec2 q = ivec2(gl_FragCoord.xy);
  P0 = q / 2;
  ivec2 quad = q - P0 * 2;

  vec3 E = src(0, 0);
  vec3 A = src(-1,-1), B = src(0,-1), C = src(1,-1);
  vec3 D = src(-1, 0),                F = src(1, 0);
  vec3 G = src(-1, 1), H = src(0, 1), I = src(1, 1);

  // Default to nearest; skip constant 3x3 neighbourhoods.
  vec3 J = E, K = E, L = E, M = E;
  o = vec4(E, 1.0);
  if (same(E,A) && same(E,B) && same(E,C) && same(E,D) && same(E,F) && same(E,G) && same(E,H) && same(E,I)) return;

  vec3 P = src(0,-2), Q = src(-2,0), R = src(2,0), S = src(0,2);
  float Bl = luma(B), Dl = luma(D), El = luma(E), Fl = luma(F), Hl = luma(H);

  // Round some corners and fill in 1:1 slopes, but preserve sharp right angles.
  if ((same(D,B) && notsame(D,H) && notsame(D,F)) && ((El>=Dl) || same(E,A)) && any_eq3(E,A,C,G) && ((El<Dl) || notsame(A,D) || notsame(E,P) || notsame(E,Q))) J=D;
  if ((same(B,F) && notsame(B,D) && notsame(B,H)) && ((El>=Bl) || same(E,C)) && any_eq3(E,A,C,I) && ((El<Bl) || notsame(C,B) || notsame(E,P) || notsame(E,R))) K=B;
  if ((same(H,D) && notsame(H,F) && notsame(H,B)) && ((El>=Hl) || same(E,G)) && any_eq3(E,A,G,I) && ((El<Hl) || notsame(G,H) || notsame(E,S) || notsame(E,Q))) L=H;
  if ((same(F,H) && notsame(F,B) && notsame(F,D)) && ((El>=Fl) || same(E,I)) && any_eq3(E,C,G,I) && ((El<Fl) || notsame(I,H) || notsame(E,R) || notsame(E,S))) M=F;

  // Clean up disconnected line intersections.
  if ((notsame(E,F) && all_eq4(E,C,I,D,Q) && all_eq2(F,B,H)) && notsame(F,src(3,0))) K=M=F;
  if ((notsame(E,D) && all_eq4(E,A,G,F,R) && all_eq2(D,B,H)) && notsame(D,src(-3,0))) J=L=D;
  if ((notsame(E,H) && all_eq4(E,G,I,B,P) && all_eq2(H,D,F)) && notsame(H,src(0,3))) L=M=H;
  if ((notsame(E,B) && all_eq4(E,A,C,H,S) && all_eq2(B,D,F)) && notsame(B,src(0,-3))) J=K=B;

  // Remove tips of bright triangles on dark backgrounds.
  if ((Bl<El) && all_eq4(E,G,H,I,S) && none_eq4(E,A,D,C,F)) J=K=B;
  if ((Hl<El) && all_eq4(E,A,B,C,P) && none_eq4(E,D,G,I,F)) L=M=H;
  if ((Fl<El) && all_eq4(E,A,D,G,Q) && none_eq4(E,B,C,I,H)) K=M=F;
  if ((Dl<El) && all_eq4(E,C,F,I,R) && none_eq4(E,B,A,G,H)) J=L=D;

  // 2:1 and 1:2 slopes of constant colour.
  if (notsame(H,B)) {
    if (notsame(H,A) && notsame(H,E) && notsame(H,C)) {
      if (all_eq3(H,G,F,R) && none_eq2(H,D,src(2,-1))) L=M;
      if (all_eq3(H,I,D,Q) && none_eq2(H,F,src(-2,-1))) M=L;
    }
    if (notsame(B,I) && notsame(B,G) && notsame(B,E)) {
      if (all_eq3(B,A,F,R) && none_eq2(B,D,src(2,1))) J=K;
      if (all_eq3(B,C,D,Q) && none_eq2(B,F,src(-2,1))) K=J;
    }
  }
  if (notsame(F,D)) {
    if (notsame(D,I) && notsame(D,E) && notsame(D,C)) {
      if (all_eq3(D,A,H,S) && none_eq2(D,B,src(1,2))) J=L;
      if (all_eq3(D,G,B,P) && none_eq2(D,H,src(1,-2))) L=J;
    }
    if (notsame(F,E) && notsame(F,A) && notsame(F,G)) {
      if (all_eq3(F,C,H,S) && none_eq2(F,B,src(-1,2))) K=M;
      if (all_eq3(F,I,B,P) && none_eq2(F,H,src(-1,-2))) M=K;
    }
  }

  o.rgb = quad.x == 0 ? (quad.y == 0 ? J : L) : (quad.y == 0 ? K : M);
}
