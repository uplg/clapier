//! The Violet API choreography dialect, encoded to binary `.chor`.
//!
//! A choreography arrives as the comma-separated text the 2006 platform
//! accepted (`fps,t,type,args,...`) and leaves as the binary opcode
//! stream the rabbit's player understands. Byte-faithful port of
//! `net.violet.platform.chor.DanceGenerator` from the released Violet
//! OS sources, including its quirks: LED bytes carry the DSL value
//! untouched, ear angles are degrees converted to twentieths of a turn,
//! and every command of a shared tick after the first gets a zero
//! delay.
//!
//! Grammar, tokens separated by commas:
//!   fps
//!   t , led     , led   , r     , g , b
//!   t , motor   , motor , angle , delay(ignored) , dir
//!   t , palette , led   , index
//!
//! `t` counts frames at `fps` frames per second. The one deliberate
//! departure from the Java: a time gap that does not fit the format's
//! single delay byte is an error here, where Violet silently truncated.

use std::collections::BTreeMap;

const CMD_TEMPO: u8 = 0x01;
const CMD_LED: u8 = 0x07;
const CMD_MOTOR: u8 = 0x08;
const CMD_PALETTE: u8 = 0x0E;
const PALETTE_0: u8 = 0xF0;
const TEETH: u32 = 20;

#[derive(Debug, PartialEq, Eq)]
pub enum ChorError {
    Empty,
    BadNumber(String),
    BadType(String),
    Truncated,
    BadFps(u32),
    BadLed(u32),
    BadMotor(u32),
    BadAngle(u32),
    BadPalette(u32),
    GapTooLong { frames: u32 },
}

impl std::fmt::Display for ChorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "empty choreography"),
            Self::BadNumber(t) => write!(f, "not a number: {t}"),
            Self::BadType(t) => write!(f, "unknown action type: {t}"),
            Self::Truncated => write!(f, "truncated choreography"),
            Self::BadFps(v) => write!(f, "fps out of range: {v}"),
            Self::BadLed(v) => write!(f, "led out of range: {v}"),
            Self::BadMotor(v) => write!(f, "motor out of range: {v}"),
            Self::BadAngle(v) => write!(f, "angle out of range: {v}"),
            Self::BadPalette(v) => write!(f, "palette index out of range: {v}"),
            Self::GapTooLong { frames } => {
                write!(f, "gap of {frames} frames does not fit a delay byte")
            }
        }
    }
}

impl std::error::Error for ChorError {}

enum Action {
    Led { led: u8, r: u8, g: u8, b: u8 },
    Motor { motor: u8, teeth: u8, dir: u8 },
    Palette { led: u8, index: u8 },
}

impl Action {
    fn append(&self, out: &mut Vec<u8>) {
        match *self {
            Self::Led { led, r, g, b } => out.extend([CMD_LED, led, r, g, b, 0, 0]),
            Self::Motor { motor, teeth, dir } => out.extend([CMD_MOTOR, motor, teeth, dir]),
            Self::Palette { led, index } => out.extend([CMD_PALETTE, led, PALETTE_0 + index]),
        }
    }
}

/// The frame duration in hundredths of a second, like the Java.
fn frame_dur_10ms(fps: u32) -> u32 {
    ((100.0 / f64::from(fps)).round() as u32).max(1)
}

/// A tick in frames converted to delay-byte units, like the Java.
fn frames_in_10ms_unit(frames: u32, fps: u32, dur: u32) -> u32 {
    (f64::from(frames) * 100.0 / f64::from(fps * dur)).round() as u32
}

fn take<'a>(tokens: &[&'a str], i: &mut usize) -> Option<&'a str> {
    let t = tokens.get(*i).copied();
    if t.is_some() {
        *i += 1;
    }
    t
}

fn take_num(tokens: &[&str], i: &mut usize) -> Result<Option<u32>, ChorError> {
    match take(tokens, i) {
        None => Ok(None),
        Some(t) => t
            .parse()
            .map(Some)
            .map_err(|_| ChorError::BadNumber(t.to_string())),
    }
}

fn need(
    tokens: &[&str],
    i: &mut usize,
    max: u32,
    err: fn(u32) -> ChorError,
) -> Result<u32, ChorError> {
    let v = take_num(tokens, i)?.ok_or(ChorError::Truncated)?;
    if v > max { Err(err(v)) } else { Ok(v) }
}

/// Encode a comma-separated Violet choreography into `.chor` bytes.
///
/// # Errors
///
/// Returns a [`ChorError`] when the text does not parse, a value is out
/// of its era-defined range, or a pause overflows the delay byte.
pub fn encode_cdl(cdl: &str) -> Result<Vec<u8>, ChorError> {
    let tokens: Vec<&str> = cdl
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect();
    let i = &mut 0usize;

    let fps = take_num(&tokens, i)?.ok_or(ChorError::Empty)?;
    if fps == 0 || fps > 100 {
        return Err(ChorError::BadFps(fps));
    }

    // TreeMap semantics: ticks sorted, same-tick actions in text order.
    let mut ticks: BTreeMap<u32, Vec<Action>> = BTreeMap::new();
    while let Some(t) = take_num(&tokens, i)? {
        let Some(kind) = take(&tokens, i) else {
            return Err(ChorError::Truncated);
        };
        let action = match kind {
            "led" => Action::Led {
                led: need(&tokens, i, 4, ChorError::BadLed)? as u8,
                r: need(&tokens, i, 255, |v| ChorError::BadNumber(v.to_string()))? as u8,
                g: need(&tokens, i, 255, |v| ChorError::BadNumber(v.to_string()))? as u8,
                b: need(&tokens, i, 255, |v| ChorError::BadNumber(v.to_string()))? as u8,
            },
            "motor" => {
                let motor = need(&tokens, i, 1, ChorError::BadMotor)? as u8;
                let angle = need(&tokens, i, 360, ChorError::BadAngle)?;
                let _delay = take_num(&tokens, i)?.ok_or(ChorError::Truncated)?;
                let dir = need(&tokens, i, 1, ChorError::BadMotor)? as u8;
                Action::Motor {
                    motor,
                    teeth: (angle * TEETH / 360) as u8,
                    dir,
                }
            }
            "palette" => Action::Palette {
                led: need(&tokens, i, 4, ChorError::BadLed)? as u8,
                index: need(&tokens, i, 7, ChorError::BadPalette)? as u8,
            },
            other => return Err(ChorError::BadType(other.to_string())),
        };
        ticks.entry(t).or_default().push(action);
    }
    if ticks.is_empty() {
        return Err(ChorError::Empty);
    }

    let dur = frame_dur_10ms(fps);
    // Header: 4 size bytes patched below, 1 NOP the player skips.
    let mut out = vec![0u8; 5];
    out.extend([CMD_TEMPO, dur as u8]);
    let mut last = 0u32;
    for (tick, actions) in &ticks {
        let t_out = frames_in_10ms_unit(*tick, fps, dur);
        let mut dt = t_out - last;
        last = t_out;
        if dt > 255 {
            return Err(ChorError::GapTooLong { frames: *tick });
        }
        for action in actions {
            out.push(dt as u8);
            action.append(&mut out);
            dt = 0;
        }
    }
    out.extend([0, 0, 0, 0]);
    let size = (out.len() - 8) as u32;
    out[..4].copy_from_slice(&size.to_be_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_the_reference_dance() {
        // fps 10: one frame is exactly one delay unit.
        let bytes =
            encode_cdl("10,0,led,0,255,0,0,5,motor,0,90,0,0,10,palette,2,3").expect("encode");
        let expected = [
            0, 0, 0, 20, 0, // size then the NOP
            0x01, 10, // tempo
            0, 0x07, 0, 255, 0, 0, 0, 0, // led at t0
            5, 0x08, 0, 5, 0, // motor at t5: 90 degrees = 5 teeth
            5, 0x0E, 2, 0xF3, // palette at t10
            0, 0, 0, 0,
        ];
        assert_eq!(bytes, expected);
    }

    #[test]
    fn co_ticked_actions_share_one_delay() {
        let bytes = encode_cdl("10,2,led,0,1,2,3,2,led,1,4,5,6").expect("encode");
        // First led carries dt 2, the second dt 0.
        assert_eq!(bytes[7], 2);
        assert_eq!(bytes[15], 0);
    }

    #[test]
    fn ticks_sort_like_a_treemap() {
        let a = encode_cdl("10,5,led,0,1,1,1,0,led,1,2,2,2").expect("encode");
        // The t0 action must be emitted first even though written second.
        assert_eq!(a[8], 0x07);
        assert_eq!(a[9], 1); // led 1 (t0) before led 0 (t5)
    }

    #[test]
    fn rejects_the_era_boundaries() {
        assert_eq!(encode_cdl(""), Err(ChorError::Empty));
        assert_eq!(encode_cdl("0,0,led,0,1,1,1"), Err(ChorError::BadFps(0)));
        assert_eq!(
            encode_cdl("10,0,dance,0"),
            Err(ChorError::BadType("dance".into()))
        );
        assert_eq!(encode_cdl("10,0,led,9,1,1,1"), Err(ChorError::BadLed(9)));
        assert_eq!(
            encode_cdl("10,0,motor,0,400,0,0"),
            Err(ChorError::BadAngle(400))
        );
        assert_eq!(encode_cdl("10,0,led,0,1,1"), Err(ChorError::Truncated));
        assert!(matches!(
            encode_cdl("1,0,led,0,1,1,1,60000,led,0,0,0,0"),
            Err(ChorError::GapTooLong { .. })
        ));
    }
}
