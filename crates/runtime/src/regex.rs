//! The regular expressions of `String.split`, `replaceAll`, `replaceFirst` and `matches`: the part
//! of `java.util.regex` that programs commonly use, matched by backtracking in Java's order, so
//! the same match is found. Supported: literal characters and escapes (`\t`, `\n`, `\.`, `\Q..\E`,
//! `\xhh`, `\uhhhh`), `.`, the classes `\d \D \s \S \w \W`, character sets (`[a-z_]`, `[^,;]`),
//! groups (capturing and `(?:..)`), `|`, `^`, `$`, and the greedy and reluctant quantifiers `*`,
//! `+`, `?`, `{n}`, `{n,}` and `{n,m}`. Anything else (back references, lookaround, flags,
//! possessive quantifiers, Unicode properties, boundaries, set unions and intersections) is
//! rejected: the translator checks a constant pattern when it translates the call, and any other
//! pattern fails when it is compiled at run time.

use crate::throw;
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
    lazy: bool,
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
}

#[derive(Clone, Copy)]
enum Class {
    Digit,
    Space,
    Word,
}

impl Class {
    fn contains(self, c: char) -> bool {
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
    fn contains(&self, c: char) -> bool {
        let inside = self.ranges.iter().any(|&(lo, hi)| lo <= c && c <= hi)
            || self.classes.iter().any(|&(class, negated)| class.contains(c) != negated);
        inside != self.negated
    }
}

fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
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
            malformed: false,
        })
    }

    /// A pattern that Java rejects with `PatternSyntaxException`.
    fn malformed<T>(&self, what: &str) -> Result<T, Problem> {
        Err(Problem {
            message: format!("{what} in the regular expression \"{}\"", self.pattern),
            malformed: true,
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
            if c == '\\' && self.chars.get(self.at + 1) == Some(&'Q') {
                self.at += 2;
                while self.at < self.chars.len() {
                    if self.chars[self.at] == '\\' && self.chars.get(self.at + 1) == Some(&'E') {
                        self.at += 2;
                        break;
                    }
                    seq.push(Piece { node: Node::Char(self.chars[self.at]), min: 1, max: 1, lazy: false });
                    self.at += 1;
                }
                continue;
            }
            let node = self.atom()?;
            let (min, max, lazy) = self.quantifier()?;
            seq.push(Piece { node, min, max, lazy });
        }
        Ok(seq)
    }

    fn atom(&mut self) -> Result<Node, Problem> {
        let c = self.next().expect("seq checks for the end");
        Ok(match c {
            '(' => {
                let capture = if self.peek() == Some('?') {
                    if self.chars.get(self.at + 1) != Some(&':') {
                        return self.error("lookaround, flags and named groups");
                    }
                    self.at += 2;
                    None
                } else {
                    self.groups += 1;
                    Some(self.groups)
                };
                let alt = self.alt()?;
                if self.next() != Some(')') {
                    return self.malformed("an unclosed group");
                }
                Node::Group(alt, capture)
            }
            '[' => Node::Set(self.set()?),
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '\\' => match self.escape(false)? {
                Escaped::Char(c) => Node::Char(c),
                Escaped::Class(class, negated) => {
                    Node::Set(Set { classes: vec![(class, negated)], ..Set::default() })
                }
            },
            '*' | '+' | '?' | '{' => return self.malformed("a dangling quantifier"),
            c => Node::Char(c),
        })
    }

    fn number(&mut self) -> Option<u32> {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        self.chars[start..self.at].iter().collect::<String>().parse().ok()
    }

    fn quantifier(&mut self) -> Result<(u32, u32, bool), Problem> {
        let (min, max) = match self.peek() {
            Some('*') => (0, u32::MAX),
            Some('+') => (1, u32::MAX),
            Some('?') => (0, 1),
            Some('{') => {
                self.at += 1;
                let Some(min) = self.number() else { return self.malformed("a malformed repetition") };
                let max = if self.peek() == Some(',') {
                    self.at += 1;
                    if self.peek() == Some('}') { u32::MAX } else { self.number().unwrap_or(0) }
                } else {
                    min
                };
                if self.peek() != Some('}') || max < min {
                    return self.malformed("a malformed repetition");
                }
                (min, max)
            }
            _ => return Ok((1, 1, false)),
        };
        self.at += 1;
        let lazy = self.peek() == Some('?');
        if lazy {
            self.at += 1;
        } else if self.peek() == Some('+') {
            return self.error("possessive quantifiers");
        }
        Ok((min, max, lazy))
    }

    fn hex(&mut self, digits: usize) -> Result<char, Problem> {
        let text: String = self.chars.get(self.at..self.at + digits).unwrap_or_default().iter().collect();
        self.at += digits;
        match u32::from_str_radix(&text, 16).ok().and_then(char::from_u32) {
            Some(c) if text.len() == digits => Ok(c),
            _ => self.malformed("a malformed escape"),
        }
    }

    /// The escape after a `\`.
    fn escape(&mut self, in_set: bool) -> Result<Escaped, Problem> {
        let Some(c) = self.next() else { return self.malformed("a trailing backslash") };
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
            'x' => Escaped::Char(self.hex(2)?),
            'u' => Escaped::Char(self.hex(4)?),
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
            let Some(c) = self.next() else { return self.malformed("an unclosed set") };
            let item = match c {
                ']' if !first => return Ok(set),
                '[' | ']' => return self.error("nested sets"),
                '&' if self.peek() == Some('&') => return self.error("set intersections"),
                '\\' => self.escape(true)?,
                c => Escaped::Char(c),
            };
            first = false;
            match item {
                Escaped::Class(class, negated) => set.classes.push((class, negated)),
                Escaped::Char(lo) => {
                    if self.peek() == Some('-') && self.chars.get(self.at + 1).is_some_and(|&c| c != ']') {
                        self.at += 1;
                        let hi = match self.next() {
                            Some('\\') => match self.escape(true)? {
                                Escaped::Char(c) => c,
                                Escaped::Class(..) => return self.error("a class in a range"),
                            },
                            Some('[') => return self.error("nested sets"),
                            Some(c) => c,
                            None => return self.malformed("an unclosed set"),
                        };
                        if hi < lo {
                            return self.malformed("an illegal character range");
                        }
                        set.ranges.push((lo, hi));
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
    let mut parser = Parser { chars: pattern.chars().collect(), at: 0, groups: 0, pattern };
    let alt = parser.alt()?;
    if parser.at < parser.chars.len() {
        return parser.malformed("an unmatched ')'");
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

/// Why a pattern cannot be compiled.
#[derive(Debug)]
pub struct Problem {
    pub message: String,
    /// Whether Java rejects the pattern too, rather than the runtime not supporting it.
    pub malformed: bool,
}

/// Checks that a pattern is supported: the error names what is not. A malformed pattern is
/// supported, as it throws `PatternSyntaxException` as in Java.
pub fn check(pattern: &str) -> Result<(), String> {
    match compile(pattern) {
        Err(problem) if !problem.malformed => Err(problem.message),
        _ => Ok(()),
    }
}

fn compiled(pattern: &str) -> Regex {
    compile(pattern).unwrap_or_else(|problem| {
        if problem.malformed {
            throw("java.util.regex.PatternSyntaxException", &problem.message)
        }
        throw("java.lang.UnsupportedOperationException", &format!("{} not supported", problem.message))
    })
}

/// A match: its start and end, in characters, and those of each group.
type Captures = Vec<Option<(usize, usize)>>;

struct Matcher<'r> {
    regex: &'r Regex,
    input: Vec<char>,
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
        if p.lazy && n >= p.min && k(pos) {
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
        !p.lazy && n >= p.min && k(pos)
    }

    fn atom(&self, node: &Node, pos: usize, k: &mut dyn FnMut(usize) -> bool) -> bool {
        let input = &self.input;
        match node {
            Node::Char(c) => input.get(pos) == Some(c) && k(pos + 1),
            Node::Any => input.get(pos).is_some_and(|&c| !is_line_terminator(c)) && k(pos + 1),
            Node::Set(set) => input.get(pos).is_some_and(|&c| set.contains(c)) && k(pos + 1),
            Node::Start => pos == 0 && k(pos),
            Node::End => {
                let rest = &input[pos..];
                let at_end = rest.is_empty()
                    || rest == ['\r', '\n']
                    || (rest.len() == 1 && is_line_terminator(rest[0]));
                at_end && k(pos)
            }
            Node::Group(alt, None) => self.alt(alt, pos, k),
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
    fn find(&self, from: usize) -> Option<Captures> {
        for start in from..=self.input.len() {
            *self.caps.borrow_mut() = vec![None; self.regex.groups + 1];
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
/// match, the next search starts one character later. Positions are byte offsets.
fn matches(regex: &Regex, s: &str, mut each: impl FnMut(&[Option<(usize, usize)>]) -> bool) {
    if let Some(literal) = &regex.literal
        && !literal.is_empty()
    {
        for (start, m) in s.match_indices(literal.as_str()) {
            if !each(&[Some((start, start + m.len()))]) {
                return;
            }
        }
        return;
    }
    let matcher = Matcher { regex, input: s.chars().collect(), caps: RefCell::new(Vec::new()) };
    let mut offsets: Vec<usize> = s.char_indices().map(|(i, _)| i).collect();
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

/// `s.split(regex, limit)`.
pub fn split(s: &str, regex: &str, limit: i32) -> Vec<String> {
    let regex = compiled(regex);
    let mut parts = Vec::new();
    let mut index = 0;
    let limited = limit > 0;
    matches(&regex, s, |caps| {
        let (start, end) = caps[0].expect("the whole match");
        if !limited || parts.len() < limit as usize - 1 {
            // A match of nothing at the start never makes an empty first part.
            if !(index == 0 && start == 0 && start == end) {
                parts.push(s[index..start].to_string());
                index = end;
            }
            true
        } else {
            false
        }
    });
    if index == 0 {
        return vec![s.to_string()];
    }
    if !limited || parts.len() < limit as usize {
        parts.push(s[index..].to_string());
    }
    if limit == 0 {
        while parts.last().is_some_and(String::is_empty) {
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

fn replace(s: &str, regex: &str, replacement: &str, all: bool) -> String {
    let regex = compiled(regex);
    let pieces = parse_replacement(replacement, regex.groups).unwrap_or_else(|(class, m)| throw(class, &m));
    let mut out = String::new();
    let mut index = 0;
    matches(&regex, s, |caps| {
        let (start, end) = caps[0].expect("the whole match");
        out.push_str(&s[index..start]);
        for piece in &pieces {
            match piece {
                Replacement::Text(text) => out.push_str(text),
                Replacement::Group(g) => {
                    if let Some((a, b)) = caps.get(*g).copied().flatten() {
                        out.push_str(&s[a..b]);
                    }
                }
            }
        }
        index = end;
        all
    });
    out.push_str(&s[index..]);
    out
}

/// `s.replaceAll(regex, replacement)`.
pub fn replace_all(s: &str, regex: &str, replacement: &str) -> String {
    replace(s, regex, replacement, true)
}

/// `s.replaceFirst(regex, replacement)`.
pub fn replace_first(s: &str, regex: &str, replacement: &str) -> String {
    replace(s, regex, replacement, false)
}

/// `s.matches(regex)`: whether the whole string matches.
pub fn matches_all(s: &str, regex: &str) -> bool {
    let regex = compiled(regex);
    let matcher = Matcher {
        regex: &regex,
        input: s.chars().collect(),
        caps: RefCell::new(vec![None; regex.groups + 1]),
    };
    let len = matcher.input.len();
    matcher.alt(&regex.alt, 0, &mut |end| end == len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(s: &str, regex: &str, limit: i32) -> Vec<String> {
        split(s, regex, limit)
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
    fn replacement_follows_java() {
        assert_eq!(replace_all("a1b22", "\\d", "#"), "a#b##");
        assert_eq!(replace_all("abc", "", "-"), "-a-b-c-");
        assert_eq!(replace_all("john smith", "(\\w+) (\\w+)", "$2 $1"), "smith john");
        assert_eq!(replace_first("aaa", "a", "b"), "baa");
        assert_eq!(replace_all("a.b", ".", "x"), "xxx");
        assert_eq!(replace_all("aaa", "a*?", "-"), "-a-a-a-");
        assert!(matches_all("abc123", "[a-z]+\\d{3}"));
        assert!(!matches_all("abc", "ab"));
        assert!(matches_all("ab", "(a|ab)b?"));
    }

    #[test]
    fn unsupported_patterns_are_rejected() {
        for p in ["(?i)a", "a++", "\\bword", "[a-z&&[^b]]", "\\1", "\\p{L}"] {
            assert!(check(p).is_err(), "{p}");
        }
        // Malformed patterns throw as in Java, so they are not rejected.
        for p in ["a{", "(a", "a)", "*a", "[z-a]", "\\"] {
            assert!(check(p).is_ok(), "{p}");
            assert!(compile(p).is_err_and(|problem| problem.malformed), "{p}");
        }
        assert!(check_replacement("a", "${name}").is_err());
        assert!(check_replacement("a", "$1").is_ok());
        assert!(check_replacement("(a)", "$1").is_ok());
    }
}
