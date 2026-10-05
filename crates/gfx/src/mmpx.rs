// MMPX
// by Morgan McGuire and Mara Gagiu
// https://casual-effects.com/research/McGuire2021PixelArt/
// License: MIT (see NOTICE)
//
// Ported to Rust for dune-hybrid from the GLSL adaptation by hunterk, working
// on palette values (u16, so "transparent" can be a value of its own) with
// the luminance supplied by the caller.

//! MMPX 2× magnification of indexed images: every output pixel is one of
//! the input's values, so palette art stays palette art.

/// Magnify `src` (`w` × `h` values) 2×. `luma` gives each value's brightness
/// (used to break ties between foreground and background).
pub fn mmpx2x(src: &[u16], w: usize, h: usize, luma: &dyn Fn(u16) -> f32) -> Vec<u16> {
    let at = |x: isize, y: isize| -> u16 {
        let x = x.clamp(0, w as isize - 1) as usize;
        let y = y.clamp(0, h as isize - 1) as usize;
        src[y * w + x]
    };
    let mut out = vec![0u16; w * h * 4];
    let ow = w * 2;
    for y in 0..h as isize {
        for x in 0..w as isize {
            let src = |c: isize, d: isize| at(x + c, y + d);
            let e = src(0, 0);
            let (a, b, c) = (src(-1, -1), src(0, -1), src(1, -1));
            let (d, f) = (src(-1, 0), src(1, 0));
            let (g, hh, i) = (src(-1, 1), src(0, 1), src(1, 1));
            let (mut j, mut k, mut l, mut m) = (e, e, e, e);
            let same = |p: u16, q: u16| p == q;
            if !(e == a && e == b && e == c && e == d && e == f && e == g && e == hh && e == i) {
                let (p, q, r, s) = (src(0, -2), src(-2, 0), src(2, 0), src(0, 2));
                let (bl, dl, el, fl, hl) = (luma(b), luma(d), luma(e), luma(f), luma(hh));
                let all_eq2 = |v: u16, p: u16, q: u16| v == p && v == q;
                let all_eq3 = |v: u16, p: u16, q: u16, r: u16| v == p && v == q && v == r;
                let all_eq4 = |v: u16, p: u16, q: u16, r: u16, s: u16| v == p && v == q && v == r && v == s;
                let any_eq3 = |v: u16, p: u16, q: u16, r: u16| v == p || v == q || v == r;
                let none_eq2 = |v: u16, p: u16, q: u16| v != p && v != q;
                let none_eq4 = |v: u16, p: u16, q: u16, r: u16, s: u16| v != p && v != q && v != r && v != s;

                // Round some corners and fill in 1:1 slopes, but preserve sharp right angles.
                if same(d, b) && d != hh && d != f && (el >= dl || same(e, a)) && any_eq3(e, a, c, g) && (el < dl || a != d || e != p || e != q) {
                    j = d;
                }
                if same(b, f) && b != d && b != hh && (el >= bl || same(e, c)) && any_eq3(e, a, c, i) && (el < bl || c != b || e != p || e != r) {
                    k = b;
                }
                if same(hh, d) && hh != f && hh != b && (el >= hl || same(e, g)) && any_eq3(e, a, g, i) && (el < hl || g != hh || e != s || e != q) {
                    l = hh;
                }
                if same(f, hh) && f != b && f != d && (el >= fl || same(e, i)) && any_eq3(e, c, g, i) && (el < fl || i != hh || e != r || e != s) {
                    m = f;
                }

                // Clean up disconnected line intersections.
                if e != f && all_eq4(e, c, i, d, q) && all_eq2(f, b, hh) && f != src(3, 0) {
                    k = f;
                    m = f;
                }
                if e != d && all_eq4(e, a, g, f, r) && all_eq2(d, b, hh) && d != src(-3, 0) {
                    j = d;
                    l = d;
                }
                if e != hh && all_eq4(e, g, i, b, p) && all_eq2(hh, d, f) && hh != src(0, 3) {
                    l = hh;
                    m = hh;
                }
                if e != b && all_eq4(e, a, c, hh, s) && all_eq2(b, d, f) && b != src(0, -3) {
                    j = b;
                    k = b;
                }

                // Remove tips of bright triangles on dark backgrounds.
                if bl < el && all_eq4(e, g, hh, i, s) && none_eq4(e, a, d, c, f) {
                    j = b;
                    k = b;
                }
                if hl < el && all_eq4(e, a, b, c, p) && none_eq4(e, d, g, i, f) {
                    l = hh;
                    m = hh;
                }
                if fl < el && all_eq4(e, a, d, g, q) && none_eq4(e, b, c, i, hh) {
                    k = f;
                    m = f;
                }
                if dl < el && all_eq4(e, c, f, i, r) && none_eq4(e, b, a, g, hh) {
                    j = d;
                    l = d;
                }

                // 2:1 and 1:2 slopes of constant colour.
                if hh != b {
                    if hh != a && hh != e && hh != c {
                        if all_eq3(hh, g, f, r) && none_eq2(hh, d, src(2, -1)) {
                            l = m;
                        }
                        if all_eq3(hh, i, d, q) && none_eq2(hh, f, src(-2, -1)) {
                            m = l;
                        }
                    }
                    if b != i && b != g && b != e {
                        if all_eq3(b, a, f, r) && none_eq2(b, d, src(2, 1)) {
                            j = k;
                        }
                        if all_eq3(b, c, d, q) && none_eq2(b, f, src(-2, 1)) {
                            k = j;
                        }
                    }
                }
                if f != d {
                    if d != i && d != e && d != c {
                        if all_eq3(d, a, hh, s) && none_eq2(d, b, src(1, 2)) {
                            j = l;
                        }
                        if all_eq3(d, g, b, p) && none_eq2(d, hh, src(1, -2)) {
                            l = j;
                        }
                    }
                    if f != e && f != a && f != g {
                        if all_eq3(f, c, hh, s) && none_eq2(f, b, src(-1, 2)) {
                            k = m;
                        }
                        if all_eq3(f, i, b, p) && none_eq2(f, hh, src(-1, -2)) {
                            m = k;
                        }
                    }
                }
            }
            let (ox, oy) = (x as usize * 2, y as usize * 2);
            out[oy * ow + ox] = j;
            out[oy * ow + ox + 1] = k;
            out[(oy + 1) * ow + ox] = l;
            out[(oy + 1) * ow + ox + 1] = m;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_images_stay_flat_and_diagonals_round() {
        let luma = |v: u16| v as f32;
        assert_eq!(mmpx2x(&[5; 9], 3, 3, &luma), vec![5; 36]);
        // A 1:1 diagonal edge gets its steps filled.
        #[rustfmt::skip]
        let img = [
            1, 0, 0, 0,
            1, 1, 0, 0,
            1, 1, 1, 0,
            1, 1, 1, 1,
        ];
        let out = mmpx2x(&img, 4, 4, &luma);
        // Every output value comes from the input.
        assert!(out.iter().all(|v| *v == 0 || *v == 1));
        assert_ne!(out, nearest(&img, 4, 4));
    }

    fn nearest(img: &[u16], w: usize, h: usize) -> Vec<u16> {
        let mut out = vec![0; w * h * 4];
        for y in 0..h * 2 {
            for x in 0..w * 2 {
                out[y * w * 2 + x] = img[(y / 2) * w + x / 2];
            }
        }
        out
    }
}
