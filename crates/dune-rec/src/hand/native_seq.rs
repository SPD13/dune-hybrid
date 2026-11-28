//! The cutscenes (story sequences) natively (the native mode, Stage C of
//! `docs/readable-port-plan.md`). A sequence is started by `loc_01771`
//! (the script's address, counted as playing, the clock stopped, the room
//! to go back to); the main loop then plays its script (`loc_01707`: a
//! byte op from the table at CS:1475h, its arguments after it, FFh
//! ending). The script is game data in the code segment: the world
//! carries that region ([`World::code_byte`]).
//!
//! Played by the simulator at once: the clock is stopped while a
//! sequence plays, and the only input it takes is the player clicking
//! through its lines, which the simulator does. The display (the rooms
//! drawn, the head, the music) is left out.

use super::world::{nm, World};

/// The sequence record (DS:4774h): playing, the blink, the room and sky
/// to go back to, the markers, the script's next byte.
const PLAYING: u16 = 0x4774;
const RETURN_TO: u16 = 0x4776;
const MARKERS: u16 = 0x4778;
const NEXT: u16 = 0x477a;
/// The clock's stops ([2788h], the clock's).
const CLOCK_STOPPED: u16 = 0x2788;
/// The sky's palette shown ([46E0h]).
const SKY: u16 = 0x46e0;

/// `loc_01771`: the sequence at CS:`script` started: its script, no
/// markers, one more playing, the clock stopped, the room (the player's
/// x low byte) and sky to go back to. (The music, the blink task:
/// display.)
pub fn start_sequence(w: &mut World, script: u16) {
    w.set_word(NEXT, script);
    w.set_word(MARKERS, 0);
    w.set_byte(PLAYING, w.byte(PLAYING).wrapping_add(1));
    w.set_byte(CLOCK_STOPPED, w.byte(CLOCK_STOPPED).wrapping_add(1));
    let ax = (w.byte(SKY) as u16) << 8 | w.byte(4) as u16;
    w.set_word(RETURN_TO, ax);
}

/// The sequence started, played to its end (the simulator: the player
/// clicking through at once).
pub fn play_sequence(w: &mut World) -> Option<()> {
    for _ in 0..200 {
        if w.byte(PLAYING) == 0 {
            return Some(());
        }
        sequence_step(w)?;
    }
    nm("a sequence without its end")
}

/// `loc_01707`: the script's next ops, run on while they chain (ops 0, 4
/// and 6 end in `loc_01707` again) to one that waits (a line said, the
/// clock run, a drawing) or the end: at FFh the room and sky given back,
/// no longer playing, the clock started (`loc_0b2be`). (A click on the
/// sequence's screen's box, `loc_0d6fe`, going to 9ED5h instead, is the
/// player's: not modelled.)
pub fn sequence_step(w: &mut World) -> Option<()> {
    for _ in 0..200 {
        let mut si = w.word(NEXT);
        let op = w.code_byte(si)?;
        si = si.wrapping_add(1);
        if op == 0xff {
            let ax = w.word(RETURN_TO);
            w.set_byte(4, ax as u8);
            w.set_byte(SKY, (ax >> 8) as u8);
            w.set_byte(PLAYING, 0);
            w.set_byte(CLOCK_STOPPED, 0);
            return Some(());
        }
        w.set_word(NEXT, si);
        match op {
            // `loc_013c8`: the room (into the player's x low byte), the
            // markers' list (its count, then them); then drawn, and on.
            0 => {
                w.set_byte(4, w.code_byte(si)?);
                si = si.wrapping_add(1);
                w.set_word(MARKERS, si);
                let n = w.code_byte(si)? as u16;
                si = si.wrapping_add(1).wrapping_add(n);
                w.set_word(NEXT, si);
            }
            // `loc_013e4`: the speaker of the next byte says its next line
            // (`loc_09761`); the player's click goes on.
            2 => {
                let talker = w.code_byte(si)? as u16;
                w.set_word(NEXT, si.wrapping_add(1));
                return super::native_script::speaker_dialogue(w, talker);
            }
            // `loc_013db`: drawn, and on.
            4 => {}
            // `loc_0140b`: the speaker set (its scene drawn), and on.
            6 => {
                let talker = w.code_byte(si)? as u16;
                w.set_word(NEXT, si.wrapping_add(1));
                w.set_word(0x47c4, talker);
            }
            // `loc_01442`: drawn; it waits.
            0x0c => return Some(()),
            // `loc_01422`: the clock run to the evening (slot 0Dh), a step
            // at a time (`loc_00fd9` with CX 1: the clock started, a step
            // waiting run first, then the time on and its step); it waits.
            0x0a => {
                for _ in 0..16 {
                    if w.time() & 0x0f >= 0x0d {
                        break;
                    }
                    w.set_byte(CLOCK_STOPPED, 0);
                    if w.byte(0x46dd) != 0 {
                        super::native_step::time_step(w)?;
                    }
                    let t = w.time().wrapping_add(1);
                    w.set_word(2, t);
                    w.save.day = t;
                    w.set_byte(0x46dd, 1);
                    super::native_step::time_step(w)?;
                }
                return Some(());
            }
            // `loc_013a0`, `loc_013aa`: speaker 0Fh's line.
            0x0e | 0x10 => return super::native_script::speaker_dialogue(w, 0x0f),
            // `loc_01474`: `ret`: it waits for the next click.
            8 => return Some(()),
            0x12 => return nm("a sequence's op 12h (a transition)"),
            0x14 => return nm("a sequence's op 14h (the ending)"),
            0x16 => return nm("a sequence's op 16h (the credits)"),
            _ => return nm("a sequence's op past the table"),
        }
    }
    nm("a sequence chaining without end")
}
