//! The regular expressions of `String.split`, `replaceAll`, `replaceFirst` and `matches`: the part
//! of `java.util.regex` that programs commonly use, matched by backtracking in Java's order, so
//! the same match is found. Supported: literal characters and escapes (`\t`, `\n`, `\.`, `\Q..\E`,
//! `\xhh`, `\uhhhh`), `.`, the classes `\d \D \s \S \w \W`, the line break `\R`, character sets
//! (`[a-z_]`, `[^,;]`), groups (capturing, `(?:..)` and atomic `(?>..)`), lookahead (`(?=..)`,
//! `(?!..)`), back references (`\1`), `|`, `^`, `$`, the word boundaries `\b` and `\B`, and the
//! greedy, reluctant and possessive quantifiers `*`, `+`, `?`, `{n}`, `{n,}` and `{n,m}`. Anything
//! else (lookbehind, flags, named groups, Unicode properties, set unions and intersections) is
//! rejected: the translator checks a constant pattern when it translates the call (and has the
//! JDK's own `Pattern` run it where the program has it, `lower::regex`), and any other pattern
//! fails when it is compiled at run time.
//!
//! A malformed pattern throws `PatternSyntaxException` with the description and the index that
//! Java's `Pattern` gives it: the parser reads as Java's does, with a quote written out as escapes
//! first (`without_quotes`) and a cursor that runs past the end of the pattern where Java's does,
//! so it finds each error at Java's position.
//!
//! The input is matched as code points, as Java's `Pattern` matches: a lone surrogate is a code
//! point of its own, which `.` and a negated set match. A pattern or a replacement string with a
//! lone surrogate is not supported.

use crate::{JString, Wtf8, throw};
use std::cell::RefCell;

/// A compiled pattern.
pub struct Regex {
    alt: Alt,
    groups: usize,
    /// The pattern's text when it matches only itself, which is searched for directly.
    literal: Option<String>,
}

type Alt = Vec<Vec<Piece>>;

struct Piece {
    node: Node,
    min: u32,
    max: u32,
    greed: Greed,
}

/// How a quantifier repeats: as many times as it can and then fewer, as few as it can and then
/// more, or as many as it can and never fewer (`+`, as Java's `POSSESSIVE`).
#[derive(Clone, Copy, PartialEq)]
enum Greed {
    Greedy,
    Lazy,
    Possessive,
}

enum Node {
    Char(char),
    /// `.`: any character but a line terminator.
    Any,
    Set(Set),
    /// `^`: the start of the input.
    Start,
    /// `$`: the end of the input, or before a line terminator that ends it.
    End,
    /// A group, with its number if it captures.
    Group(Alt, Option<usize>),
    /// `(?>..)`: a group that keeps the first way it matches, never backtracking into it.
    Atomic(Alt),
    /// `(?=..)` or, negated, `(?!..)`: whether the group matches here, without consuming input.
    /// The groups it captures in a match stay captured, as in Java.
    Ahead(Alt, bool),
    /// `\b` or, negated, `\B`: a word boundary, as Java's `Bound` tells it.
    Boundary(bool),
    /// `\R`: a line break, `\r\n` or one of `\n \u000B \f \r \u0085 \u2028 \u2029`.
    LineBreak,
    /// `\n`: the text that group `n` matched last, if it did.
    BackRef(usize),
}

#[derive(Clone, Copy)]
enum Class {
    Digit,
    Space,
    Word,
}

impl Class {
    /// Whether code point `c` is in the class: never a lone surrogate.
    fn contains(self, c: u32) -> bool {
        let Some(c) = char::from_u32(c) else { return false };
        match self {
            Class::Digit => c.is_ascii_digit(),
            Class::Space => matches!(c, ' ' | '\t' | '\n' | '\u{B}' | '\u{C}' | '\r'),
            Class::Word => c.is_ascii_alphanumeric() || c == '_',
        }
    }
}

#[derive(Default)]
struct Set {
    negated: bool,
    ranges: Vec<(char, char)>,
    /// Predefined classes inside the set, each possibly negated (`\D`).
    classes: Vec<(Class, bool)>,
}

impl Set {
    /// Whether code point `c` is in the set; a range such as `\u0000-\uFFFF` spans the surrogates.
    fn contains(&self, c: u32) -> bool {
        let inside = self.ranges.iter().any(|&(lo, hi)| lo as u32 <= c && c <= hi as u32)
            || self.classes.iter().any(|&(class, negated)| class.contains(c) != negated);
        inside != self.negated
    }
}

fn is_line_terminator(c: u32) -> bool {
    matches!(c, 0x0A | 0x0D | 0x85 | 0x2028 | 0x2029)
}

struct Parser<'p> {
    chars: Vec<char>,
    at: usize,
    groups: usize,
    pattern: &'p str,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek();
        self.at += 1;
        c
    }

    /// What the translation does not support: `what` is a noun phrase.
    fn error<T>(&self, what: &str) -> Result<T, Problem> {
        Err(Problem {
            message: format!("{what} in the regular expression \"{}\"", self.pattern),
            syntax: None,
        })
    }

    /// A pattern that Java rejects with `PatternSyntaxException`: its description and index, as
    /// Java's `Pattern` gives them (its `cursor - 1` where it finds the error, in code points;
    /// `at` is that cursor at the same point, as this parser reads as Java's does).
    fn malformed<T>(&self, description: &str, index: i32) -> Result<T, Problem> {
        Err(Problem {
            message: syntax_message(description, self.pattern, index),
            syntax: Some((description.to_string(), index)),
        })
    }

    fn alt(&mut self) -> Result<Alt, Problem> {
        let mut alt = vec![self.seq()?];
        while self.peek() == Some('|') {
            self.at += 1;
            alt.push(self.seq()?);
        }
        Ok(alt)
    }

    fn seq(&mut self) -> Result<Vec<Piece>, Problem> {
        let mut seq = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            // A `{` where an atom would be repeats nothing, as in Java (`{2}` matches "").
            let node = if c == '{' { Node::Group(vec![Vec::new()], None) } else { self.atom()? };
            let (min, max, greed) = self.quantifier()?;
            seq.push(Piece { node, min, max, greed });
        }
        Ok(seq)
    }

    fn atom(&mut self) -> Result<Node, Problem> {
        let c = self.next().expect("seq checks for the end");
        Ok(match c {
            '(' => {
                // The group's kind, from what follows `(?`: `None` for a capturing group.
                let kind = if self.peek() == Some('?') {
                    let kind = self.chars.get(self.at + 1).copied();
                    match kind {
                        Some(':' | '>' | '=' | '!') => {}
                        Some('$' | '@') => return self.malformed("Unknown group type", self.at as i32 + 1),
                        // Flags (none, as in `(?)`, too), lookbehind and named groups.
                        Some('i' | 'd' | 'm' | 's' | 'u' | 'c' | 'x' | 'U' | '-' | ')' | '<') => {
                            return self.error("lookbehind, flags and named groups");
                        }
                        _ => return self.malformed("Unknown inline modifier", self.at as i32 + 1),
                    }
                    self.at += 2;
                    kind
                } else {
                    self.groups += 1;
                    None
                };
                let number = self.groups;
                let alt = self.alt()?;
                let end = self.at;
                if self.next() != Some(')') {
                    return self.malformed("Unclosed group", end as i32);
                }
                match kind {
                    None => Node::Group(alt, Some(number)),
                    Some('>') => Node::Atomic(alt),
                    Some('=') => Node::Ahead(alt, false),
                    Some('!') => Node::Ahead(alt, true),
                    _ => Node::Group(alt, None),
                }
            }
            '[' => Node::Set(self.set()?),
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '\\' if self.peek() == Some('R') => {
                self.at += 1;
                Node::LineBreak
            }
            '\\' if matches!(self.peek(), Some('b' | 'B')) => {
                // `\b{g}` is a grapheme boundary.
                if self.chars.get(self.at..self.at + 3) == Some(&['b', '{', 'g'][..]) {
                    return self.error("grapheme boundaries");
                }
                Node::Boundary(self.next() == Some('B'))
            }
            '\\' if self.peek().is_some_and(|c| matches!(c, '1'..='9')) => Node::BackRef(self.group_number()),
            '\\' => match self.escape(false)? {
                Escaped::Char(c) => Node::Char(c),
                Escaped::Class(class, negated) => {
                    Node::Set(Set { classes: vec![(class, negated)], ..Set::default() })
                }
            },
            '*' | '+' | '?' => {
                return self.malformed(&format!("Dangling meta character '{c}'"), self.at as i32 - 1);
            }
            c => Node::Char(c),
        })
    }

    /// The number of a back reference, after its `\`: its first digit, and each further one as
    /// long as the number stays one of a group opened before it, as Java reads it.
    fn group_number(&mut self) -> usize {
        let mut n = 0;
        while let Some(digit) = self.peek().and_then(|c| c.to_digit(10)) {
            let more = n * 10 + digit as usize;
            if n > 0 && more > self.groups {
                break;
            }
            n = more;
            self.at += 1;
        }
        n
    }

    /// The digits of a repetition's bound, at least one; one that takes it past Java's `int` is
    /// an error there.
    fn bound(&mut self) -> Result<u32, Problem> {
        let mut n: i32 = 0;
        while let Some(digit) = self.peek().and_then(|c| c.to_digit(10)) {
            let Some(more) = n.checked_mul(10).and_then(|n| n.checked_add(digit as i32)) else {
                return self.malformed("Illegal repetition range", self.at as i32);
            };
            n = more;
            self.at += 1;
        }
        Ok(n as u32)
    }

    fn quantifier(&mut self) -> Result<(u32, u32, Greed), Problem> {
        let (min, max) = match self.peek() {
            Some('*') => (0, u32::MAX),
            Some('+') => (1, u32::MAX),
            Some('?') => (0, 1),
            Some('{') => {
                self.at += 1;
                if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    return self.malformed("Illegal repetition", self.at as i32);
                }
                let min = self.bound()?;
                let max = if self.peek() == Some(',') {
                    self.at += 1;
                    if self.peek() == Some('}') { u32::MAX } else { self.bound()? }
                } else {
                    min
                };
                if self.peek() != Some('}') {
                    return self.malformed("Unclosed counted closure", self.at as i32);
                }
                if max < min {
                    return self.malformed("Illegal repetition range", self.at as i32);
                }
                (min, max)
            }
            _ => return Ok((1, 1, Greed::Greedy)),
        };
        self.at += 1;
        let greed = match self.peek() {
            Some('?') => Greed::Lazy,
            Some('+') => Greed::Possessive,
            _ => return Ok((min, max, Greed::Greedy)),
        };
        self.at += 1;
        Ok((min, max, greed))
    }

    /// The hexadecimal digit at the cursor, read as Java's `read()` does: past the end too.
    fn read_hex(&mut self) -> Option<u32> {
        let digit = self.peek().and_then(|c| c.to_digit(16));
        self.at += 1;
        digit
    }

    /// The code unit of `\xhh`, after the `x`.
    fn hex_escape(&mut self) -> Result<char, Problem> {
        let illegal = |p: &Self| p.malformed("Illegal hexadecimal escape sequence", p.at as i32 - 1);
        if self.peek() == Some('{') && self.chars.get(self.at + 1).is_some_and(|c| c.is_ascii_hexdigit()) {
            return self.error("the escape \\x{..}");
        }
        let Some(high) = self.read_hex() else { return illegal(self) };
        let Some(low) = self.read_hex() else { return illegal(self) };
        Ok(char::from_u32(high * 16 + low).expect("a code unit below 0x100"))
    }

    /// The code unit of the four digits of `\uhhhh`, after the `u`.
    fn unit_escape(&mut self) -> Result<u32, Problem> {
        let mut n = 0;
        for _ in 0..4 {
            let Some(digit) = self.read_hex() else {
                return self.malformed("Illegal Unicode escape sequence", self.at as i32 - 1);
            };
            n = n * 16 + digit;
        }
        Ok(n)
    }

    /// The character of `\uhhhh`, after the `u`: a high surrogate followed by the `\uhhhh` of a
    /// low one is their pair, as in Java.
    fn unicode_escape(&mut self) -> Result<char, Problem> {
        let n = self.unit_escape()?;
        if (0xD800..0xDC00).contains(&n) {
            let after = self.at;
            if self.next() == Some('\\') && self.next() == Some('u') {
                let low = self.unit_escape()?;
                if (0xDC00..0xE000).contains(&low) {
                    let pair = 0x10000 + ((n - 0xD800) << 10) + (low - 0xDC00);
                    return Ok(char::from_u32(pair).expect("a supplementary code point"));
                }
            }
            self.at = after;
        }
        match char::from_u32(n) {
            Some(c) => Ok(c),
            None => self.error("a lone surrogate"),
        }
    }

    /// The escape after a `\`. One at the end of the pattern is the character 0 that Java reads
    /// past it, with the cursor one further, as Java's: what it ends then says so.
    fn escape(&mut self, in_set: bool) -> Result<Escaped, Problem> {
        let Some(c) = self.next() else { return Ok(Escaped::Char('\0')) };
        let illegal = |p: &Self| p.malformed("Illegal/unsupported escape sequence", p.at as i32 - 1);
        Ok(match c {
            'd' => Escaped::Class(Class::Digit, false),
            'D' => Escaped::Class(Class::Digit, true),
            's' => Escaped::Class(Class::Space, false),
            'S' => Escaped::Class(Class::Space, true),
            'w' => Escaped::Class(Class::Word, false),
            'W' => Escaped::Class(Class::Word, true),
            't' => Escaped::Char('\t'),
            'n' => Escaped::Char('\n'),
            'r' => Escaped::Char('\r'),
            'f' => Escaped::Char('\u{C}'),
            'a' => Escaped::Char('\u{7}'),
            'e' => Escaped::Char('\u{1B}'),
            'x' => Escaped::Char(self.hex_escape()?),
            'u' => Escaped::Char(self.unicode_escape()?),
            // Java has no such escape, or none in a set (`atom` reads `\R`, `\b`, `\B` and back
            // references outside one).
            'C' | 'E' | 'F' | 'I' | 'J' | 'K' | 'L' | 'M' | 'O' | 'Q' | 'T' | 'U' | 'Y' | 'g' | 'i' | 'j'
            | 'l' | 'm' | 'o' | 'q' | 'y' => return illegal(self),
            'R' | 'b' | 'B' | '1'..='9' | 'A' | 'G' | 'X' | 'Z' | 'z' | 'k' if in_set => {
                return illegal(self);
            }
            'k' if self.peek() != Some('<') => {
                let description = "\\k is not followed by '<' for named capturing group";
                return self.malformed(description, self.at as i32);
            }
            c if c.is_ascii_alphanumeric() => {
                let place = if in_set { " in a set" } else { "" };
                return self.error(&format!("the escape \\{c}{place}"));
            }
            c => Escaped::Char(c),
        })
    }

    fn set(&mut self) -> Result<Set, Problem> {
        let mut set = Set::default();
        if self.peek() == Some('^') {
            set.negated = true;
            self.at += 1;
        }
        let mut first = true;
        loop {
            let end = self.at as i32 - 1;
            let Some(c) = self.next() else { return self.malformed("Unclosed character class", end) };
            let item = match c {
                ']' if !first => return Ok(set),
                // A `]` that would leave the set empty is a character of it.
                ']' => Escaped::Char(']'),
                '[' => return self.error("nested sets"),
                '&' if self.peek() == Some('&') => return self.error("set intersections"),
                '\\' => self.escape(true)?,
                c => Escaped::Char(c),
            };
            first = false;
            match item {
                Escaped::Class(class, negated) => set.classes.push((class, negated)),
                Escaped::Char(lo) => {
                    if self.peek() == Some('-') && self.chars.get(self.at + 1) != Some(&']') {
                        self.at += 1;
                        let hi = match self.next() {
                            Some('\\') => self.escape(true)?,
                            Some('[') => return self.error("nested sets"),
                            // Java reads the character 0 past the end.
                            c => Escaped::Char(c.unwrap_or('\0')),
                        };
                        match hi {
                            Escaped::Char(hi) if hi >= lo => set.ranges.push((lo, hi)),
                            _ => return self.malformed("Illegal character range", self.at as i32 - 1),
                        }
                    } else {
                        set.ranges.push((lo, lo));
                    }
                }
            }
        }
    }
}

enum Escaped {
    Char(char),
    Class(Class, bool),
}

/// Compiles a pattern, or says why it is not supported.
pub fn compile(pattern: &str) -> Result<Regex, Problem> {
    let chars = without_quotes(pattern.chars().collect());
    let mut parser = Parser { chars, at: 0, groups: 0, pattern };
    let alt = parser.alt()?;
    let length = parser.chars.len();
    if parser.at > length {
        return parser.malformed("Unescaped trailing backslash", length as i32);
    }
    if parser.at < length {
        return parser.malformed("Unmatched closing ')'", parser.at as i32 - 1);
    }
    let literal = match alt.as_slice() {
        [seq] if seq.iter().all(|p| matches!(p.node, Node::Char(_)) && p.min == 1 && p.max == 1) => Some(
            seq.iter()
                .map(|p| match p.node {
                    Node::Char(c) => c,
                    _ => unreachable!(),
                })
                .collect(),
        ),
        _ => None,
    };
    Ok(Regex { alt, groups: parser.groups, literal })
}

/// The pattern with each quote (`\\Q..\\E`) written as the escapes of its characters, as Java's
/// `Pattern` reads it before anything else (a digit that starts a quote as `\\x3d`, so that it
/// cannot extend an escape before it): positions in a pattern with a quote are positions in this
/// text, in Java's errors too.
fn without_quotes(chars: Vec<char>) -> Vec<char> {
    let mut i = 0;
    while i + 1 < chars.len() {
        match (chars[i], chars[i + 1]) {
            ('\\', 'Q') => break,
            ('\\', _) => i += 2,
            _ => i += 1,
        }
    }
    if i + 1 >= chars.len() {
        return chars;
    }
    let mut out = chars[..i].to_vec();
    i += 2;
    let (mut quoted, mut begins) = (true, true);
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        if !c.is_ascii() || c.is_ascii_alphabetic() {
            out.push(c);
        } else if c.is_ascii_digit() {
            if begins {
                out.extend(['\\', 'x', '3']);
            }
            out.push(c);
        } else if c != '\\' {
            if quoted {
                out.push('\\');
            }
            out.push(c);
        } else if quoted {
            if chars.get(i) == Some(&'E') {
                i += 1;
                quoted = false;
            } else {
                out.extend(['\\', '\\']);
            }
        } else if chars.get(i) == Some(&'Q') {
            i += 1;
            quoted = true;
            begins = true;
            continue;
        } else {
            out.push(c);
            if i < chars.len() {
                out.push(chars[i]);
                i += 1;
            }
        }
        begins = false;
    }
    out
}

/// Why a pattern cannot be compiled.
#[derive(Debug)]
pub struct Problem {
    /// What the runtime does not support, or for a malformed pattern Java's message of its
    /// `PatternSyntaxException` (`syntax_message`).
    pub message: String,
    /// For a pattern that Java rejects too, the description and the index of its
    /// `PatternSyntaxException`.
    pub syntax: Option<(String, i32)>,
}

/// `PatternSyntaxException.getMessage()`: the description, the index when there is one, then on
/// lines of their own the pattern and a `^` under the index (with a tab under each tab before it).
/// Java counts the index in code points but places the `^` by code units, and so does this.
pub fn syntax_message(description: &str, pattern: &str, index: i32) -> String {
    let mut out = description.to_string();
    if index >= 0 {
        out += &format!(" near index {index}");
    }
    out.push('\n');
    out.push_str(pattern);
    let units: Vec<u16> = pattern.encode_utf16().collect();
    if index >= 0 && (index as usize) < units.len() {
        out.push('\n');
        out.extend(units[..index as usize].iter().map(|&u| if u == u16::from(b'\t') { '\t' } else { ' ' }));
        out.push('^');
    }
    out
}

/// Checks that a pattern is supported: the error names what is not. A malformed pattern is
/// supported, as it throws `PatternSyntaxException` as in Java.
pub fn check(pattern: &str) -> Result<(), String> {
    match compile(pattern) {
        Err(problem) if problem.syntax.is_none() => Err(problem.message),
        _ => Ok(()),
    }
}

fn compiled(pattern: &str) -> Regex {
    compile(pattern).unwrap_or_else(|problem| {
        if problem.syntax.is_some() {
            throw("java.util.regex.PatternSyntaxException", &problem.message)
        }
        throw("java.lang.UnsupportedOperationException", &format!("{} not supported", problem.message))
    })
}

/// A match: its start and end, in code points, and those of each group.
type Captures = Vec<Option<(usize, usize)>>;

struct Matcher<'r> {
    regex: &'r Regex,
    /// The input's code points.
    input: Vec<u32>,
    caps: RefCell<Captures>,
}

impl Matcher<'_> {
    fn alt(&self, alt: &Alt, pos: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        alt.iter().any(|seq| self.seq(seq, pos, k))
    }

    fn seq(&self, seq: &[Piece], pos: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        match seq.split_first() {
            None => k(pos),
            Some((piece, rest)) => self.repeat(piece, 0, pos, &mut |q| self.seq(rest, q, k)),
        }
    }

    fn repeat(&self, p: &Piece, n: u32, pos: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        if p.greed == Greed::Possessive {
            return self.possessive(p, pos, k);
        }
        let lazy = p.greed == Greed::Lazy;
        if lazy && n >= p.min && k(pos) {
            return true;
        }
        if n < p.max {
            // An iteration that matches nothing ends the loop once the minimum is reached.
            let more =
                self.atom(&p.node, pos, &mut |q| (q != pos || n < p.min) && self.repeat(p, n + 1, q, k));
            if more {
                return true;
            }
        }
        !lazy && n >= p.min && k(pos)
    }

    /// The first way `node` matches at `pos`, if it does: where it ends. What it captured stays.
    fn first(&self, node: &Node, pos: usize) -> Option<usize> {
        let mut end = None;
        self.atom(node, pos, &mut |q| {
            end = Some(q);
            true
        });
        end
    }

    /// A possessive quantifier, as Java's `Curly` matches one: the atom's first match, `min` times
    /// and then as many more as it matches, up to `max` and stopping at one that matches nothing,
    /// never giving any back.
    fn possessive(&self, p: &Piece, mut pos: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        for _ in 0..p.min {
            match self.first(&p.node, pos) {
                Some(q) => pos = q,
                None => return false,
            }
        }
        for _ in p.min..p.max {
            match self.first(&p.node, pos) {
                Some(q) if q != pos => pos = q,
                _ => break,
            }
        }
        k(pos)
    }

    /// Whether `pos` is at a word boundary, as Java's `Bound.check` tells it: whether a word
    /// character lies on one side and not the other. A word character is ASCII (`[a-zA-Z_0-9]`),
    /// or a non-spacing mark after a letter or a digit, maybe through other marks.
    fn at_boundary(&self, pos: usize) -> bool {
        let input = &self.input;
        let word = |c: u32| Class::Word.contains(c);
        let mark = crate::code_point_chars::is_non_spacing_mark;
        // Java's `hasBaseCharacter`, from the code point at index `i` back, a `char` at a time:
        // the first `char` it reads is that code point when `whole`, and its last one otherwise.
        let based = |mut i: usize, whole: bool| {
            // The `char` that ends the code point at `i`: a low surrogate for one above U+FFFF.
            let last = |c: u32| if c > 0xFFFF { 0xDC00 | (c & 0x3FF) } else { c };
            let mut c = if whole { input[i] } else { last(input[i]) };
            loop {
                if crate::is_letter_or_digit_code_point(c as i32) {
                    return true;
                }
                if !mark(c) || i == 0 {
                    return false;
                }
                i -= 1;
                c = last(input[i]);
            }
        };
        let left = pos > 0 && {
            let c = input[pos - 1];
            word(c) || (mark(c) && based(pos - 1, false))
        };
        let right = pos < input.len() && {
            let c = input[pos];
            word(c) || (mark(c) && based(pos, true))
        };
        left != right
    }

    fn atom(&self, node: &Node, pos: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        let input = &self.input;
        match node {
            Node::Char(c) => input.get(pos) == Some(&(*c as u32)) && k(pos + 1),
            Node::Any => input.get(pos).is_some_and(|&c| !is_line_terminator(c)) && k(pos + 1),
            Node::Set(set) => input.get(pos).is_some_and(|&c| set.contains(c)) && k(pos + 1),
            Node::Start => pos == 0 && k(pos),
            Node::End => {
                let rest = &input[pos..];
                let at_end = rest.is_empty()
                    || rest == [0x0D, 0x0A]
                    || (rest.len() == 1 && is_line_terminator(rest[0]));
                at_end && k(pos)
            }
            Node::LineBreak => match input.get(pos) {
                // `\r\n` is tried first, then `\r` alone.
                Some(0x0D) => (input.get(pos + 1) == Some(&0x0A) && k(pos + 2)) || k(pos + 1),
                Some(&c) => matches!(c, 0x0A | 0x0B | 0x0C | 0x85 | 0x2028 | 0x2029) && k(pos + 1),
                None => false,
            },
            // A group that has not matched (or does not exist) matches nothing.
            Node::BackRef(i) => {
                let Some((start, end)) = self.caps.borrow().get(*i).copied().flatten() else { return false };
                let len = end - start;
                pos + len <= input.len() && input[start..end] == input[pos..pos + len] && k(pos + len)
            }
            Node::Group(alt, None) => self.alt(alt, pos, k),
            Node::Atomic(alt) => {
                let mut end = None;
                self.alt(alt, pos, &mut |q| {
                    end = Some(q);
                    true
                });
                end.is_some_and(k)
            }
            Node::Ahead(alt, negated) => self.alt(alt, pos, &mut |_| true) != *negated && k(pos),
            Node::Boundary(negated) => self.at_boundary(pos) != *negated && k(pos),
            Node::Group(alt, Some(i)) => self.alt(alt, pos, &mut |end| {
                let old = self.caps.borrow()[*i];
                self.caps.borrow_mut()[*i] = Some((pos, end));
                k(end) || {
                    self.caps.borrow_mut()[*i] = old;
                    false
                }
            }),
        }
    }

    /// The first match that starts at or after `from`, as Java's `Matcher.find` finds it.
    /// The groups are cleared once, as Java's `search` clears them: a group that a lookahead
    /// captured in a try that failed later stays captured in the next.
    fn find(&self, from: usize) -> Option<Captures> {
        *self.caps.borrow_mut() = vec![None; self.regex.groups + 1];
        for start in from..=self.input.len() {
            let mut end = None;
            if self.alt(&self.regex.alt, start, &mut |e| {
                end = Some(e);
                true
            }) {
                let mut caps = self.caps.borrow().clone();
                caps[0] = Some((start, end.expect("a match has an end")));
                return Some(caps);
            }
        }
        None
    }
}

/// Every match of `regex` in `s`, in order, as Java's repeated `find` gives them: after an empty
/// match, the next search starts one code point later. Positions are byte offsets.
fn matches(regex: &Regex, s: &Wtf8, mut each: impl FnMut(&[Option<(usize, usize)>]) -> bool) {
    if let Some(literal) = &regex.literal
        && !literal.is_empty()
    {
        // The literal has no lone surrogate, so its bytes match whole code points only.
        let literal = Wtf8::new(literal);
        let mut from = 0;
        while let Some(start) = s.find_from(literal, from) {
            from = start + literal.len();
            if !each(&[Some((start, from))]) {
                return;
            }
        }
        return;
    }
    let (mut offsets, input): (Vec<usize>, Vec<u32>) = s.code_point_indices().unzip();
    let matcher = Matcher { regex, input, caps: RefCell::new(Vec::new()) };
    offsets.push(s.len());
    let mut from = 0;
    while from <= matcher.input.len() {
        let Some(caps) = matcher.find(from) else { return };
        let (start, end) = caps[0].expect("the whole match");
        let bytes: Vec<Option<(usize, usize)>> =
            caps.iter().map(|c| c.map(|(a, b)| (offsets[a], offsets[b]))).collect();
        if !each(&bytes) {
            return;
        }
        from = if end == start { end + 1 } else { end };
    }
}

/// `s.split(regex, limit)`. A pattern with a lone surrogate is not supported.
pub fn split(s: &Wtf8, regex: &Wtf8, limit: i32) -> Vec<JString> {
    let regex = compiled(regex.expect_str("split"));
    let mut parts = Vec::new();
    let mut index = 0;
    let limited = limit > 0;
    matches(&regex, s, |caps| {
        let (start, end) = caps[0].expect("the whole match");
        if !limited || parts.len() < limit as usize - 1 {
            // A match of nothing at the start never makes an empty first part.
            if !(index == 0 && start == 0 && start == end) {
                parts.push(JString::from(s.slice(index, start)));
                index = end;
            }
            true
        } else {
            false
        }
    });
    if index == 0 {
        return vec![JString::from(s)];
    }
    if !limited || parts.len() < limit as usize {
        parts.push(JString::from(s.slice(index, s.len())));
    }
    if limit == 0 {
        while parts.last().is_some_and(|p| p.is_empty()) {
            parts.pop();
        }
    }
    parts
}

/// The pieces of a replacement string: text, and references to groups.
enum Replacement {
    Text(String),
    Group(usize),
}

fn parse_replacement(replacement: &str, groups: usize) -> Result<Vec<Replacement>, (&'static str, String)> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut chars = replacement.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(c) => text.push(c),
                None => {
                    return Err((
                        "java.lang.IllegalArgumentException",
                        "character to be escaped is missing".into(),
                    ));
                }
            },
            '$' => {
                let Some(first) = chars.next_if(char::is_ascii_digit) else {
                    let message =
                        if chars.peek() == Some(&'{') { NAMED_GROUPS } else { "Illegal group reference" };
                    return Err(("java.lang.IllegalArgumentException", message.into()));
                };
                let mut group = first.to_digit(10).unwrap() as usize;
                if group > groups {
                    return Err(("java.lang.IndexOutOfBoundsException", format!("No group {group}")));
                }
                // Java takes as many digits as still name a group.
                while let Some(&d) = chars.peek().filter(|d| d.is_ascii_digit()) {
                    let next = group * 10 + d.to_digit(10).unwrap() as usize;
                    if next > groups {
                        break;
                    }
                    group = next;
                    chars.next();
                }
                out.push(Replacement::Text(std::mem::take(&mut text)));
                out.push(Replacement::Group(group));
            }
            c => text.push(c),
        }
    }
    out.push(Replacement::Text(text));
    Ok(out)
}

const NAMED_GROUPS: &str = "named groups";

/// Checks a replacement string of `replaceAll` for a pattern: the error says why Java would
/// throw, or why it is not supported.
pub fn check_replacement(pattern: &str, replacement: &str) -> Result<(), String> {
    check(pattern)?;
    let Ok(regex) = compile(pattern) else { return Ok(()) };
    match parse_replacement(replacement, regex.groups) {
        Err((_, message)) if message == NAMED_GROUPS => {
            Err(format!("{message} in the replacement \"{replacement}\""))
        }
        _ => Ok(()),
    }
}

/// Whether `pattern` compiles, and `replacement` (if given) is a replacement for it, so that a
/// call with them never throws.
pub fn never_fails(pattern: &str, replacement: Option<&str>) -> bool {
    let Ok(regex) = compile(pattern) else { return false };
    replacement.is_none_or(|r| parse_replacement(r, regex.groups).is_ok())
}

/// The text of `s` with the first match of `regex`, or every one when `all`, replaced. Pieces
/// are pushed one after another, so halves of a pair that the replacing brings together join.
fn replace(s: &Wtf8, regex: &str, replacement: &str, all: bool) -> JString {
    let regex = compiled(regex);
    let pieces = parse_replacement(replacement, regex.groups).unwrap_or_else(|(class, m)| throw(class, &m));
    let mut out = JString::new();
    let mut index = 0;
    matches(&regex, s, |caps| {
        let (start, end) = caps[0].expect("the whole match");
        out.push_wtf8(s.slice(index, start));
        for piece in &pieces {
            match piece {
                Replacement::Text(text) => out.push_str(text),
                Replacement::Group(g) => {
                    if let Some((a, b)) = caps.get(*g).copied().flatten() {
                        out.push_wtf8(s.slice(a, b));
                    }
                }
            }
        }
        index = end;
        all
    });
    out.push_wtf8(s.slice(index, s.len()));
    out
}

/// `s.replaceAll(regex, replacement)`.
pub fn replace_all(s: &Wtf8, regex: &Wtf8, replacement: &Wtf8) -> JString {
    let what = "replaceAll";
    replace(s, regex.expect_str(what), replacement.expect_str(what), true)
}

/// `s.replaceFirst(regex, replacement)`.
pub fn replace_first(s: &Wtf8, regex: &Wtf8, replacement: &Wtf8) -> JString {
    let what = "replaceFirst";
    replace(s, regex.expect_str(what), replacement.expect_str(what), false)
}

/// Whether `regex` matches somewhere in `s`, as `Matcher.find` from the start.
pub fn finds(s: &Wtf8, regex: &Wtf8) -> bool {
    let regex = compiled(regex.expect_str("a regular expression"));
    let mut found = false;
    matches(&regex, s, |_| {
        found = true;
        false
    });
    found
}

/// `s.matches(regex)`: whether the whole string matches.
pub fn matches_all(s: &Wtf8, regex: &Wtf8) -> bool {
    let regex = compiled(regex.expect_str("matches"));
    let matcher = Matcher {
        regex: &regex,
        input: s.code_points().collect(),
        caps: RefCell::new(vec![None; regex.groups + 1]),
    };
    let len = matcher.input.len();
    matcher.alt(&regex.alt, 0, &mut |end| end == len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(s: &str, regex: &str, limit: i32) -> Vec<String> {
        split(Wtf8::new(s), Wtf8::new(regex), limit).iter().map(JString::to_string).collect()
    }

    fn w(s: &str) -> &Wtf8 {
        Wtf8::new(s)
    }

    #[test]
    fn split_follows_java() {
        assert_eq!(parts("a,b,,c,,", ",", 0), ["a", "b", "", "c"]);
        assert_eq!(parts("a,b,,c,,", ",", -1), ["a", "b", "", "c", "", ""]);
        assert_eq!(parts("a,b,c", ",", 2), ["a", "b,c"]);
        assert_eq!(parts(",a", ",", 0), ["", "a"]);
        assert!(parts(",", ",", 0).is_empty());
        assert_eq!(parts("", ",", 0), [""]);
        assert_eq!(parts("abc", "", 0), ["a", "b", "c"]);
        assert_eq!(parts("  hello   world ", "\\s+", 0), ["", "hello", "world"]);
        assert_eq!(parts("a1b22c", "\\d+", 0), ["a", "b", "c"]);
        assert_eq!(parts("x.y.z", "\\.", 0), ["x", "y", "z"]);
        assert_eq!(parts("a;b,c", "[,;]", 0), ["a", "b", "c"]);
        assert_eq!(parts("one, two,three", ",\\s*", 0), ["one", "two", "three"]);
        assert_eq!(parts("aXbYc", "X|Y", 0), ["a", "b", "c"]);
    }

    #[test]
    fn lone_surrogates_are_code_points_of_the_input() {
        let s = crate::JString::from_units(&[0x61, 0xD83D, 0x2C, 0xDE00, 0x2C, 0x62]);
        let units = |p: &JString| p.units().collect::<Vec<u16>>();
        let pieces = split(&s, w(","), 0);
        assert_eq!(
            pieces.iter().map(units).collect::<Vec<_>>(),
            [vec![0x61, 0xD83D], vec![0xDE00], vec![0x62]]
        );
        // Each surrogate is a code point that a negated set matches.
        let pieces = split(&s, w("[^,]"), 0);
        assert_eq!(pieces.iter().map(units).collect::<Vec<_>>(), [vec![], vec![], vec![0x2C], vec![0x2C]]);
        assert!(matches_all(&s, w(".+")));
        assert!(!matches_all(&s, w("[\\w,]+")));
        // Removing what lies between the halves joins them into their pair.
        assert_eq!(replace_all(&s, w(","), w("")).to_string(), "a\u{1F600}b");
    }

    #[test]
    fn replacement_follows_java() {
        assert_eq!(replace_all(w("a1b22"), w("\\d"), w("#")), "a#b##");
        assert_eq!(replace_all(w("abc"), w(""), w("-")), "-a-b-c-");
        assert_eq!(replace_all(w("john smith"), w("(\\w+) (\\w+)"), w("$2 $1")), "smith john");
        assert_eq!(replace_first(w("aaa"), w("a"), w("b")), "baa");
        assert_eq!(replace_all(w("a.b"), w("."), w("x")), "xxx");
        assert_eq!(replace_all(w("aaa"), w("a*?"), w("-")), "-a-a-a-");
        assert!(matches_all(w("abc123"), w("[a-z]+\\d{3}")));
        assert!(!matches_all(w("abc"), w("ab")));
        assert!(matches_all(w("ab"), w("(a|ab)b?")));
        assert_eq!(replace_all(w("a..b...c"), w("(\\.)(\\1)+"), w("/")), "a/b/c");
        assert_eq!(replace_all(w("abab cdc"), w("(\\w)(\\w)\\1\\2"), w("<$0>")), "<abab> cdc");
        // `\11` is group 1 then `1` when there is no group 11.
        assert!(matches_all(w("aa1"), w("(a)\\11")));
        assert!(!matches_all(w("a"), w("\\2(a)")));
        assert_eq!(parts("a\r\nb\nc\rd\u{2028}e", "\\R", 0), ["a", "b", "c", "d", "e"]);
        assert!(matches_all(w("\r\n"), w("\\R")));
        assert!(matches_all(w("\r\n"), w("\\R\\n")));
    }

    #[test]
    fn boundaries_lookahead_and_possessives_follow_java() {
        let zeros = w("((?:(?:^|:)0+\\b){2,8}):?(?!\\S*\\b\\1:0+\\b)(\\S*)");
        assert_eq!(replace_all(w("fe80:0:0:0:0:0:0:1"), zeros, w("::$2")), "fe80::1");
        assert_eq!(replace_all(w("1:0:0:2:0:0:0:3"), zeros, w("::$2")), "1:0:0:2::3");
        assert_eq!(replace_all(w("0:0:0:0:0:0:0:0"), zeros, w("::$2")), "::");
        assert_eq!(parts("one two\u{e9}three", "\\b", 0), ["one", " ", "two", "\u{e9}", "three"]);
        assert_eq!(replace_all(w("hello world"), w("\\b"), w("|")), "|hello| |world|");
        assert_eq!(replace_all(w("hello world"), w("\\B"), w("-")), "h-e-l-l-o w-o-r-l-d");
        // A non-spacing mark after a letter is a word character; one that starts a word is not.
        let marks = w("e\u{301}x \u{301}y a\u{301}\u{302}");
        assert_eq!(replace_all(marks, w("\\b"), w("|")), "|e\u{301}x| \u{301}|y| |a\u{301}\u{302}|");
        // Java reads a mark above U+FFFF before a position as its low surrogate: no word character.
        assert_eq!(replace_all(w("x\u{1D167}y"), w("\\b"), w("|")), "|x\u{1D167}|y|");
        assert!(!matches_all(w("aaa"), w("a++a")));
        assert!(matches_all(w("aab"), w("a*+b")));
        assert!(matches_all(w("abab"), w("(ab){1,}+")));
        assert_eq!(replace_all(w("aaa"), w("(a?)++"), w("<$1>")), "<><>");
        assert_eq!(replace_all(w("aaab"), w("(?>a|aa)b"), w("X")), "aaX");
        assert!(!matches_all(w("ab"), w("(?>a|ab)")));
        assert_eq!(parts("a1b2c3", "(?=\\d)", 0), ["a", "1b", "2c", "3"]);
        assert_eq!(replace_all(w("foobar foobaz"), w("foo(?!bar)"), w("X")), "foobar Xbaz");
        assert_eq!(replace_all(w("abc"), w("(?=(b))"), w("[$1]")), "a[b]bc");
        // A group that a lookahead captured stays captured when the match fails after it.
        assert_eq!(replace_all(w("xy"), w("(?:(?=(x))z|y)"), w("<$1>")), "x<x>");
    }

    #[test]
    fn unsupported_patterns_are_rejected() {
        for p in [
            "(?i)a",
            "(?<=a)b",
            "(?<n>a)",
            "\\b{g}",
            "[a-z&&[^b]]",
            "\\0",
            "\\p{L}",
            "(?)",
            "\\x{41}",
            "\\ud800",
        ] {
            assert!(check(p).is_err(), "{p}");
        }
        // Malformed patterns throw as in Java, so they are not rejected.
        for p in ["a{", "(a", "a)", "*a", "[z-a]", "\\", "[\\b]"] {
            assert!(check(p).is_ok(), "{p}");
            assert!(compile(p).is_err_and(|problem| problem.syntax.is_some()), "{p}");
        }
        assert!(check_replacement("a", "${name}").is_err());
        assert!(check_replacement("a", "$1").is_ok());
        assert!(check_replacement("(a)", "$1").is_ok());
    }

    /// Malformed patterns, each with the description and the index of the
    /// `PatternSyntaxException` that Java's `Pattern.compile` throws for it (from a JVM's run).
    const MALFORMED: &[(&str, &str, i32)] = &[
        ("(ab", "Unclosed group", 3),
        ("a(b(c)", "Unclosed group", 6),
        ("(", "Unclosed group", 1),
        ("\t(x", "Unclosed group", 3),
        ("(?:a", "Unclosed group", 4),
        ("(?=a", "Unclosed group", 4),
        ("(?>", "Unclosed group", 3),
        ("((a)", "Unclosed group", 4),
        ("(a\\", "Unclosed group", 4),
        ("((a\\", "Unclosed group", 5),
        ("*a", "Dangling meta character '*'", 0),
        ("a|+b", "Dangling meta character '+'", 2),
        ("?", "Dangling meta character '?'", 0),
        ("a**", "Dangling meta character '*'", 2),
        ("a+++", "Dangling meta character '+'", 3),
        ("x{2}?+", "Dangling meta character '+'", 5),
        ("(*)", "Dangling meta character '*'", 1),
        ("a|*", "Dangling meta character '*'", 2),
        ("{", "Illegal repetition", 1),
        ("x{", "Illegal repetition", 2),
        ("{a", "Illegal repetition", 1),
        ("a{", "Illegal repetition", 2),
        ("a{x}", "Illegal repetition", 2),
        ("a{,2}", "Illegal repetition", 2),
        ("a{2}{", "Illegal repetition", 5),
        ("a{2,1}", "Illegal repetition range", 5),
        ("a{1", "Unclosed counted closure", 3),
        ("a{1,", "Unclosed counted closure", 4),
        ("a{1,2", "Unclosed counted closure", 5),
        ("a{1x}", "Unclosed counted closure", 3),
        ("a{1,x}", "Unclosed counted closure", 4),
        ("a{99999999999}", "Illegal repetition range", 11),
        ("a{2147483648}", "Illegal repetition range", 11),
        ("a{1,2147483648}", "Illegal repetition range", 13),
        ("\\x4", "Illegal hexadecimal escape sequence", 3),
        ("\\xzz", "Illegal hexadecimal escape sequence", 2),
        ("\\x", "Illegal hexadecimal escape sequence", 2),
        ("[\\x4]", "Illegal hexadecimal escape sequence", 4),
        ("[a-\\x4]", "Illegal hexadecimal escape sequence", 6),
        ("\\u12", "Illegal Unicode escape sequence", 4),
        ("\\u12g4", "Illegal Unicode escape sequence", 4),
        ("\\u", "Illegal Unicode escape sequence", 2),
        ("\\uD800\\u12", "Illegal Unicode escape sequence", 10),
        ("\\", "Unescaped trailing backslash", 1),
        ("ab\\", "Unescaped trailing backslash", 3),
        ("[\\", "Unclosed character class", 2),
        ("[a\\", "Unclosed character class", 3),
        ("[a-\\", "Illegal character range", 4),
        ("[\\R]", "Illegal/unsupported escape sequence", 2),
        ("[\\b]", "Illegal/unsupported escape sequence", 2),
        ("[\\1]", "Illegal/unsupported escape sequence", 2),
        ("[\\B]", "Illegal/unsupported escape sequence", 2),
        ("[\\9]", "Illegal/unsupported escape sequence", 2),
        ("[a-\\R]", "Illegal/unsupported escape sequence", 4),
        ("[\\A]", "Illegal/unsupported escape sequence", 2),
        ("[\\Z]", "Illegal/unsupported escape sequence", 2),
        ("[\\k]", "Illegal/unsupported escape sequence", 2),
        ("[\\Q]", "Unclosed character class", 2),
        ("\\y", "Illegal/unsupported escape sequence", 1),
        ("\\i", "Illegal/unsupported escape sequence", 1),
        ("[\\y]", "Illegal/unsupported escape sequence", 2),
        ("\\E", "Illegal/unsupported escape sequence", 1),
        ("\\C", "Illegal/unsupported escape sequence", 1),
        ("\\k", "\\k is not followed by '<' for named capturing group", 2),
        ("\\kx", "\\k is not followed by '<' for named capturing group", 2),
        ("[abc", "Unclosed character class", 3),
        ("[^", "Unclosed character class", 1),
        ("[]", "Unclosed character class", 1),
        ("[^]", "Unclosed character class", 2),
        ("[", "Unclosed character class", 0),
        ("[a-", "Illegal character range", 3),
        ("[z-a]", "Illegal character range", 3),
        ("[a-\\d]", "Illegal character range", 4),
        ("[b-a-c]", "Illegal character range", 3),
        ("a)", "Unmatched closing ')'", 0),
        ("ab)c", "Unmatched closing ')'", 1),
        (")", "Unmatched closing ')'", -1),
        ("(a))", "Unmatched closing ')'", 2),
        ("(?", "Unknown inline modifier", 2),
        ("(?$a)", "Unknown group type", 2),
        ("(?@", "Unknown group type", 2),
        ("(?*)", "Unknown inline modifier", 2),
        ("(?a)", "Unknown inline modifier", 2),
        ("(?z)", "Unknown inline modifier", 2),
    ];

    #[test]
    fn malformed_patterns_are_described_as_in_java() {
        for &(pattern, description, index) in MALFORMED {
            let problem = compile(pattern).err();
            let syntax = problem.and_then(|p| p.syntax);
            assert_eq!(syntax, Some((description.to_string(), index)), "{pattern}");
        }
    }

    #[test]
    fn syntax_messages_are_javas() {
        let message = |p: &str| compile(p).err().map(|problem| problem.message);
        let javas = [
            ("a(b", "Unclosed group near index 3\na(b"),
            (")", "Unmatched closing ')'\n)"),
            ("a\t\tb{", "Illegal repetition near index 5\na\t\tb{"),
            ("\u{1F600}{", "Illegal repetition near index 2\n\u{1F600}{\n  ^"),
            ("\\Qa.b\\E)", "Unmatched closing ')' near index 3\n\\Qa.b\\E)\n   ^"),
            ("\\Q(\\E(", "Unclosed group near index 3\n\\Q(\\E(\n   ^"),
            ("[\\Qab]", "Unclosed character class near index 4\n[\\Qab]\n    ^"),
            ("\\Q1\\E{", "Illegal repetition near index 5\n\\Q1\\E{\n     ^"),
            ("x\\Q\\\\E\\y", "Illegal/unsupported escape sequence near index 4\nx\\Q\\\\E\\y\n    ^"),
            ("(?:\\Q)\\E", "Unclosed group near index 5\n(?:\\Q)\\E\n     ^"),
            ("\ta{x}", "Illegal repetition near index 3\n\ta{x}\n\t  ^"),
            ("a\t\tb{x}", "Illegal repetition near index 5\na\t\tb{x}\n \t\t  ^"),
            ("\u{1F600}a{x}", "Illegal repetition near index 3\n\u{1F600}a{x}\n   ^"),
        ];
        for (pattern, java) in javas {
            assert_eq!(message(pattern).as_deref(), Some(java), "{pattern}");
        }
    }

    #[test]
    fn patterns_that_java_reads_differently_from_how_they_look() {
        // A `{` where an atom would be repeats nothing.
        assert!(matches_all(w(""), w("{1}")));
        assert!(!matches_all(w("{1}"), w("{1}")));
        assert!(matches_all(w("a"), w("a{1}{2}")));
        // A `]` first in a set is one of its characters.
        assert!(matches_all(w("]"), w("[]a]")));
        assert!(matches_all(w("b"), w("[^]a]")));
        // A quote's characters are escaped, in a set too; a digit that starts one stays a digit.
        assert!(matches_all(w("a.b"), w("\\Qa.b\\E")));
        assert!(!matches_all(w("axb"), w("\\Qa.b\\E")));
        assert!(matches_all(w("-"), w("[\\Qa-z\\E]")));
        assert!(!matches_all(w("b"), w("[\\Qa-z\\E]")));
        assert!(matches_all(w("12"), w("\\Q12\\E")));
        assert!(matches_all(w("x"), w("x\\Q")));
        // A pair of surrogates written as two escapes is one code point.
        assert!(matches_all(w("\u{1F600}"), w("\\uD83D\\uDE00")));
    }
}
