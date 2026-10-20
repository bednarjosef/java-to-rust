//! The Rust side of the Character check: prints what the runtime's `Character` functions return,
//! in the format of `CharacterSweep.java`, so that check.sh can diff the two.

use java_runtime as java;
use std::io::{self, Write};

const CHARS_HEADER: &str = "# code isDigit isLetter isLetterOrDigit isAlphabetic isUpperCase \
    isLowerCase isWhitespace isSpaceChar isSurrogate isHighSurrogate isLowSurrogate \
    toUpperCase toLowerCase getNumericValue \
    digit2 digit3 digit4 digit5 digit6 digit7 digit8 digit9 digit10 digit11 digit12 \
    digit13 digit14 digit15 digit16 digit17 digit18 digit19 digit20 digit21 digit22 \
    digit23 digit24 digit25 digit26 digit27 digit28 digit29 digit30 digit31 digit32 \
    digit33 digit34 digit35 digit36";

fn bit(b: bool) -> &'static str {
    if b { " 1" } else { " 0" }
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let mut out = io::BufWriter::new(io::stdout().lock());
    match mode.as_str() {
        "chars" => {
            writeln!(out, "{CHARS_HEADER}").unwrap();
            for c in 0..=0xFFFFu16 {
                print_char(&mut out, c);
            }
        }
        "for-digit" => {
            writeln!(out, "# digit radix forDigit").unwrap();
            for digit in -2..=37 {
                for radix in -2..=38 {
                    writeln!(out, "{digit} {radix} {}", java::for_digit(digit, radix)).unwrap();
                }
            }
        }
        _ => {
            eprintln!("usage: character-check chars | for-digit");
            std::process::exit(2);
        }
    }
}

fn print_char(out: &mut impl Write, c: u16) {
    let mut line = format!("{c:04X}");
    for b in [
        java::is_digit(c),
        java::is_letter(c),
        java::is_letter_or_digit(c),
        java::is_alphabetic(c),
        java::is_upper_case(c),
        java::is_lower_case(c),
        java::is_whitespace(c),
        java::is_space_char(c),
        java::is_surrogate(c),
        java::is_high_surrogate(c),
        java::is_low_surrogate(c),
    ] {
        line.push_str(bit(b));
    }
    line.push_str(&format!(" {:04X} {:04X}", java::to_upper_case(c), java::to_lower_case(c)));
    line.push_str(&format!(" {}", java::char_numeric_value(c)));
    for radix in 2..=36 {
        line.push_str(&format!(" {}", java::digit(c, radix)));
    }
    writeln!(out, "{line}").unwrap();
}
