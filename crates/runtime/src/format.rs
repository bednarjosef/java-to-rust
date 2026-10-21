//! `String.format`, `String.formatted` and `System.out.printf`: Java's `java.util.Formatter` in the
//! US locale (the decimal point is `.` and the grouping separator `,`), for the conversions
//! `%b %h %s %c %d %o %x %e %f %g %n %%` (and their upper-case forms) with argument indexes, flags,
//! width and precision. Floating-point values are rounded the way Java rounds them: half up, from
//! the shortest decimal that reads back as the value (the digits of `Double.toString`), so
//! `String.format("%.1f", 0.15)` is `0.2` where C's `printf` gives `0.1`. The format is checked
//! before anything is written, and a bad format or argument throws the exception Java throws.
//! Hexadecimal floating point (`%a`) and dates (`%t`) are not supported.

use crate::{double_to_string, float_to_string, shortest_digits, string, throw};

/// An argument of a format, as its Java value.
#[derive(Clone, Copy, Debug)]
pub enum Arg<'a> {
    Null,
    Bool(bool),
    Char(u16),
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    /// A `String`.
    Str(&'a str),
    /// Any other object, by the text its `toString` gives.
    Text(&'a str),
}

impl Arg<'_> {
    /// The Java class of the argument, for error messages.
    fn class(&self) -> &'static str {
        match self {
            Arg::Null => "null",
            Arg::Bool(_) => "java.lang.Boolean",
            Arg::Char(_) => "java.lang.Character",
            Arg::Byte(_) => "java.lang.Byte",
            Arg::Short(_) => "java.lang.Short",
            Arg::Int(_) => "java.lang.Integer",
            Arg::Long(_) => "java.lang.Long",
            Arg::Float(_) => "java.lang.Float",
            Arg::Double(_) => "java.lang.Double",
            Arg::Str(_) => "java.lang.String",
            Arg::Text(_) => "java.lang.Object",
        }
    }
}

const LEFT_JUSTIFY: u32 = 1;
const UPPERCASE: u32 = 1 << 1;
const ALTERNATE: u32 = 1 << 2;
const PLUS: u32 = 1 << 3;
const LEADING_SPACE: u32 = 1 << 4;
const ZERO_PAD: u32 = 1 << 5;
const GROUP: u32 = 1 << 6;
const PARENTHESES: u32 = 1 << 7;
const PREVIOUS: u32 = 1 << 8;

fn flag_text(flags: u32) -> String {
    let mut s = String::new();
    for (bit, c) in [
        (LEFT_JUSTIFY, '-'),
        (UPPERCASE, '^'),
        (ALTERNATE, '#'),
        (PLUS, '+'),
        (LEADING_SPACE, ' '),
        (ZERO_PAD, '0'),
        (GROUP, ','),
        (PARENTHESES, '('),
        (PREVIOUS, '<'),
    ] {
        if flags & bit != 0 {
            s.push(c);
        }
    }
    s
}

/// One `%` specifier.
#[derive(Clone, Debug)]
struct Spec {
    /// 0 for the next argument, -1 for the previous one (`<`), -2 for no argument (`%n`, `%%`),
    /// or the 1-based argument index.
    index: i32,
    flags: u32,
    width: i32,
    precision: i32,
    /// The conversion, in lower case; upper case is the `UPPERCASE` flag.
    conversion: char,
}

impl Spec {
    fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    /// The specifier as Java's error messages show it.
    fn text(&self) -> String {
        let mut s = format!("%{}", flag_text(self.flags & !UPPERCASE));
        if self.index > 0 {
            s.push_str(&format!("{}$", self.index));
        }
        if self.width != -1 {
            s.push_str(&self.width.to_string());
        }
        if self.precision != -1 {
            s.push_str(&format!(".{}", self.precision));
        }
        s.push(if self.has(UPPERCASE) { self.conversion.to_ascii_uppercase() } else { self.conversion });
        s
    }
}

enum Item {
    Text(String),
    Spec(Spec),
}

/// Why a format fails: Java's exception, or a conversion that the translation does not support.
#[derive(Debug)]
pub struct FormatError {
    pub class: &'static str,
    pub message: String,
}

fn fail<T>(class: &'static str, message: impl Into<String>) -> Result<T, FormatError> {
    Err(FormatError { class, message: message.into() })
}

const UNKNOWN: &str = "java.util.UnknownFormatConversionException";
const UNSUPPORTED: &str = "java.lang.UnsupportedOperationException";

fn mismatch<T>(flags: u32, conversion: char) -> Result<T, FormatError> {
    fail(
        "java.util.FormatFlagsConversionMismatchException",
        format!("Conversion = {conversion}, Flags = {}", flag_text(flags)),
    )
}

fn is_conversion(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '%'
}

fn valid_conversion(c: char) -> bool {
    matches!(
        c,
        'b' | 'B'
            | 's'
            | 'S'
            | 'h'
            | 'H'
            | 'c'
            | 'C'
            | 'd'
            | 'o'
            | 'x'
            | 'X'
            | 'e'
            | 'E'
            | 'g'
            | 'G'
            | 'f'
            | 'a'
            | 'A'
            | 'n'
            | '%'
    )
}

/// Splits a format into text and specifiers, checking each specifier as Java does.
fn parse(format: &str) -> Result<Vec<Item>, FormatError> {
    let chars: Vec<char> = format.chars().collect();
    let mut items = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '%' {
            text.push(chars[i]);
            i += 1;
            continue;
        }
        if !text.is_empty() {
            items.push(Item::Text(std::mem::take(&mut text)));
        }
        i += 1;
        let Some(&first) = chars.get(i) else { return fail(UNKNOWN, "Conversion = '%'") };
        let digits = |i: &mut usize| {
            let from = *i;
            while chars.get(*i).is_some_and(char::is_ascii_digit) {
                *i += 1;
            }
            chars[from..*i].iter().collect::<String>()
        };
        // %(\d+\$)?([-#+ 0,(\<]*)?(\d+)?(\.\d+)?([tT])?([a-zA-Z%])
        let mut index = 0;
        let mut j = i;
        let arg = digits(&mut j);
        if !arg.is_empty() && chars.get(j) == Some(&'$') {
            index = match arg.parse::<i32>() {
                Ok(n) if n > 0 => n,
                _ => {
                    return fail(
                        "java.util.IllegalFormatArgumentIndexException",
                        format!("Illegal format argument index = {arg}"),
                    );
                }
            };
            i = j + 1;
        }
        let mut flags = 0;
        while let Some(&c) = chars.get(i) {
            let flag = match c {
                '-' => LEFT_JUSTIFY,
                '#' => ALTERNATE,
                '+' => PLUS,
                ' ' => LEADING_SPACE,
                '0' => ZERO_PAD,
                ',' => GROUP,
                '(' => PARENTHESES,
                '<' => PREVIOUS,
                _ => break,
            };
            if flags & flag != 0 {
                return fail("java.util.DuplicateFormatFlagsException", format!("Flags = '{c}'"));
            }
            flags |= flag;
            i += 1;
        }
        if flags & PREVIOUS != 0 {
            index = -1;
        }
        let width_text = digits(&mut i);
        let width = if width_text.is_empty() {
            -1
        } else {
            match width_text.parse::<i32>() {
                Ok(w) => w,
                Err(_) => return fail("java.util.IllegalFormatWidthException", i32::MIN.to_string()),
            }
        };
        let mut precision = -1;
        if chars.get(i) == Some(&'.') {
            i += 1;
            let p = digits(&mut i);
            if p.is_empty() {
                return fail(UNKNOWN, format!("Conversion = '{first}'"));
            }
            precision = match p.parse::<i32>() {
                Ok(p) => p,
                Err(_) => return fail("java.util.IllegalFormatPrecisionException", i32::MIN.to_string()),
            };
        }
        let Some(&c) = chars.get(i) else { return fail(UNKNOWN, format!("Conversion = '{first}'")) };
        if (c == 't' || c == 'T') && chars.get(i + 1).is_some_and(|&c| is_conversion(c)) {
            return fail(UNSUPPORTED, "dates in a format (%t)");
        }
        if !is_conversion(c) {
            return fail(UNKNOWN, format!("Conversion = '{first}'"));
        }
        if !valid_conversion(c) {
            return fail(UNKNOWN, format!("Conversion = '{c}'"));
        }
        i += 1;
        let mut spec = Spec { index, flags, width, precision, conversion: c };
        if c.is_ascii_uppercase() {
            spec.flags |= UPPERCASE;
            spec.conversion = c.to_ascii_lowercase();
        }
        if matches!(spec.conversion, 'n' | '%') {
            spec.index = -2;
        }
        check(&spec)?;
        items.push(Item::Spec(spec));
    }
    if !text.is_empty() {
        items.push(Item::Text(text));
    }
    Ok(items)
}

fn check_bad_flags(spec: &Spec, bad: u32) -> Result<(), FormatError> {
    if spec.flags & bad != 0 {
        return mismatch(spec.flags & bad, spec.conversion);
    }
    Ok(())
}

fn missing_width<T>(spec: &Spec) -> Result<T, FormatError> {
    fail("java.util.MissingFormatWidthException", spec.text())
}

fn check_numeric(spec: &Spec) -> Result<(), FormatError> {
    if spec.width == -1 && spec.flags & (LEFT_JUSTIFY | ZERO_PAD) != 0 {
        return missing_width(spec);
    }
    if (spec.has(PLUS) && spec.has(LEADING_SPACE)) || (spec.has(LEFT_JUSTIFY) && spec.has(ZERO_PAD)) {
        return fail("java.util.IllegalFormatFlagsException", format!("Flags = '{}'", flag_text(spec.flags)));
    }
    Ok(())
}

fn bad_precision<T>(precision: i32) -> Result<T, FormatError> {
    fail("java.util.IllegalFormatPrecisionException", precision.to_string())
}

/// Java's `FormatSpecifier.check`.
fn check(spec: &Spec) -> Result<(), FormatError> {
    match spec.conversion {
        'b' | 'h' | 's' => {
            if spec.conversion != 's' && spec.has(ALTERNATE) {
                return mismatch(ALTERNATE, spec.conversion);
            }
            if spec.width == -1 && spec.has(LEFT_JUSTIFY) {
                return missing_width(spec);
            }
            check_bad_flags(spec, PLUS | LEADING_SPACE | ZERO_PAD | GROUP | PARENTHESES)
        }
        'c' => {
            if spec.precision != -1 {
                return bad_precision(spec.precision);
            }
            check_bad_flags(spec, ALTERNATE | PLUS | LEADING_SPACE | ZERO_PAD | GROUP | PARENTHESES)?;
            if spec.width == -1 && spec.has(LEFT_JUSTIFY) {
                return missing_width(spec);
            }
            Ok(())
        }
        'd' | 'o' | 'x' => {
            check_numeric(spec)?;
            if spec.precision != -1 {
                return bad_precision(spec.precision);
            }
            check_bad_flags(spec, if spec.conversion == 'd' { ALTERNATE } else { GROUP })
        }
        'e' | 'f' | 'g' | 'a' => {
            check_numeric(spec)?;
            match spec.conversion {
                'a' => {
                    check_bad_flags(spec, PARENTHESES | GROUP)?;
                    fail(UNSUPPORTED, "hexadecimal floating point in a format (%a)")
                }
                'e' => check_bad_flags(spec, GROUP),
                'g' => check_bad_flags(spec, ALTERNATE),
                _ => Ok(()),
            }
        }
        '%' => {
            if spec.precision != -1 {
                return bad_precision(spec.precision);
            }
            if spec.flags != LEFT_JUSTIFY && spec.flags != 0 {
                return fail(
                    "java.util.IllegalFormatFlagsException",
                    format!("Flags = '{}'", flag_text(spec.flags)),
                );
            }
            if spec.width == -1 && spec.has(LEFT_JUSTIFY) {
                return missing_width(spec);
            }
            Ok(())
        }
        _ => {
            // 'n'
            if spec.precision != -1 {
                return bad_precision(spec.precision);
            }
            if spec.width != -1 {
                return fail("java.util.IllegalFormatWidthException", spec.width.to_string());
            }
            if spec.flags != 0 {
                return fail(
                    "java.util.IllegalFormatFlagsException",
                    format!("Flags = '{}'", flag_text(spec.flags)),
                );
            }
            Ok(())
        }
    }
}

/// Checks that a constant format uses only what is supported. A format that Java rejects is
/// accepted here: the translation throws the same exception when it runs.
pub fn check_supported(format: &str) -> Result<(), String> {
    match parse(format) {
        Err(e) if e.class == UNSUPPORTED => Err(e.message),
        _ => Ok(()),
    }
}

/// Whether a constant format never throws with arguments of the kinds of `args`, whatever their
/// values: its conversions fit them, and no `%c` takes a number, whose value may not be a code
/// point. The translator asks it of each `String.format` with a literal format.
pub fn never_fails(format: &str, args: &[Arg]) -> bool {
    let Ok(items) = parse(format) else { return false };
    let numeric_char = items.iter().any(|item| matches!(item, Item::Spec(spec) if spec.conversion == 'c'))
        && args.iter().any(|a| matches!(a, Arg::Byte(_) | Arg::Short(_) | Arg::Int(_)));
    !numeric_char && try_format(format, args).is_ok()
}

/// `String.format(format, args)` with the arguments in an `Object[]`, each by the text of its
/// `toString` (`None` for null). The text does not tell a number from a string, so a conversion
/// other than `%s` and `%S` (and `%n` and `%%`, which take none) is not supported, and fails
/// loudly when it runs.
pub fn format_objects(format: &str, texts: &[Option<String>]) -> String {
    if let Ok(items) = parse(format)
        && let Some(spec) = items.iter().find_map(|item| match item {
            Item::Spec(spec) if !matches!(spec.conversion, 's' | 'S' | 'n' | '%') => Some(spec),
            _ => None,
        })
    {
        let message = format!("%{} of an element of an Object[] in a format not supported", spec.conversion);
        throw(UNSUPPORTED, &message);
    }
    let args: Vec<Arg> = texts.iter().map(|t| t.as_deref().map_or(Arg::Null, Arg::Text)).collect();
    self::format(format, &args)
}

/// `String.format(format, args)`.
pub fn format(format: &str, args: &[Arg]) -> String {
    try_format(format, args).unwrap_or_else(|e| {
        let message = if e.class == UNSUPPORTED { format!("{} not supported", e.message) } else { e.message };
        throw(e.class, &message)
    })
}

fn try_format(format: &str, args: &[Arg]) -> Result<String, FormatError> {
    let items = parse(format)?;
    let mut out = String::new();
    let mut last: i32 = -1;
    let mut last_ordinary: i32 = -1;
    for item in &items {
        let spec = match item {
            Item::Text(text) => {
                out.push_str(text);
                continue;
            }
            Item::Spec(spec) => spec,
        };
        let arg = match spec.index {
            -2 => Arg::Null,
            -1 => {
                if last < 0 || last as usize >= args.len() {
                    return fail(
                        "java.util.MissingFormatArgumentException",
                        format!("Format specifier '{}'", spec.text()),
                    );
                }
                args[last as usize]
            }
            0 => {
                last_ordinary += 1;
                last = last_ordinary;
                if last as usize >= args.len() {
                    return fail(
                        "java.util.MissingFormatArgumentException",
                        format!("Format specifier '{}'", spec.text()),
                    );
                }
                args[last as usize]
            }
            n => {
                last = n - 1;
                if last as usize >= args.len() {
                    return fail(
                        "java.util.MissingFormatArgumentException",
                        format!("Format specifier '{}'", spec.text()),
                    );
                }
                args[last as usize]
            }
        };
        print(&mut out, spec, arg)?;
    }
    Ok(out)
}

fn conversion_error<T>(spec: &Spec, arg: Arg) -> Result<T, FormatError> {
    fail("java.util.IllegalFormatConversionException", format!("{} != {}", spec.conversion, arg.class()))
}

fn print(out: &mut String, spec: &Spec, arg: Arg) -> Result<(), FormatError> {
    match spec.conversion {
        'd' | 'o' | 'x' => {
            let (value, bits) = match arg {
                Arg::Null => return print_text(out, spec, "null"),
                Arg::Byte(v) => (v as i64, 8),
                Arg::Short(v) => (v as i64, 16),
                Arg::Int(v) => (v as i64, 32),
                Arg::Long(v) => (v, 64),
                _ => return conversion_error(spec, arg),
            };
            print_integer(out, spec, value, bits)
        }
        'e' | 'f' | 'g' => {
            let value = match arg {
                Arg::Null => return print_text(out, spec, "null"),
                Arg::Float(v) => v as f64,
                Arg::Double(v) => v,
                _ => return conversion_error(spec, arg),
            };
            print_float(out, spec, value);
            Ok(())
        }
        'c' => {
            let text = match arg {
                Arg::Null => "null".to_string(),
                Arg::Char(c) => string::from_units(&[c]),
                Arg::Byte(_) | Arg::Short(_) | Arg::Int(_) => {
                    let cp = match arg {
                        Arg::Byte(v) => v as i32,
                        Arg::Short(v) => v as i32,
                        Arg::Int(v) => v,
                        _ => unreachable!(),
                    };
                    match u32::try_from(cp).ok().filter(|&cp| cp <= 0x10FFFF) {
                        Some(cp) => match char::from_u32(cp) {
                            Some(c) => c.to_string(),
                            None => "?".to_string(),
                        },
                        None => {
                            return fail(
                                "java.util.IllegalFormatCodePointException",
                                format!("Code point = 0x{cp:x}"),
                            );
                        }
                    }
                }
                _ => return conversion_error(spec, arg),
            };
            print_text(out, spec, &text)
        }
        'b' => {
            let text = match arg {
                Arg::Null => "false",
                Arg::Bool(false) => "false",
                _ => "true",
            };
            print_text(out, spec, text)
        }
        's' => {
            if spec.has(ALTERNATE) {
                return mismatch(ALTERNATE, 's');
            }
            let text = match arg {
                Arg::Null => "null".to_string(),
                Arg::Bool(b) => b.to_string(),
                Arg::Char(c) => string::from_units(&[c]),
                Arg::Byte(v) => v.to_string(),
                Arg::Short(v) => v.to_string(),
                Arg::Int(v) => v.to_string(),
                Arg::Long(v) => v.to_string(),
                Arg::Float(v) => float_to_string(v),
                Arg::Double(v) => double_to_string(v),
                Arg::Str(s) | Arg::Text(s) => s.to_string(),
            };
            print_text(out, spec, &text)
        }
        'h' => {
            let hash = match arg {
                Arg::Null => return print_text(out, spec, "null"),
                Arg::Bool(b) => crate::bool_hash(b),
                Arg::Char(c) => c as i32,
                Arg::Byte(v) => v as i32,
                Arg::Short(v) => v as i32,
                Arg::Int(v) => v,
                Arg::Long(v) => crate::long_hash(v),
                Arg::Float(v) => crate::float_hash(v),
                Arg::Double(v) => crate::double_hash(v),
                Arg::Str(s) => string::string_hash(s),
                Arg::Text(_) => return fail(UNSUPPORTED, "%h of an object"),
            };
            print_text(out, spec, &format!("{:x}", hash as u32))
        }
        'n' => {
            out.push('\n');
            Ok(())
        }
        _ => print_text(out, spec, "%"),
    }
}

/// Java's `print(String)`: cut to the precision, upper-cased for an upper-case conversion, and
/// justified in the width.
fn print_text(out: &mut String, spec: &Spec, text: &str) -> Result<(), FormatError> {
    let mut text = text.to_string();
    if spec.precision != -1 {
        let units = string::units(&text);
        if (spec.precision as usize) < units.len() {
            text = string::from_units(&units[..spec.precision as usize]);
        }
    }
    if spec.has(UPPERCASE) {
        text = string::uppercase(&text);
    }
    justify(out, spec, &text);
    Ok(())
}

/// Pads `text` with spaces to the width, on the left unless the `-` flag is set.
fn justify(out: &mut String, spec: &Spec, text: &str) {
    let len = text.encode_utf16().count() as i32;
    let pad = (spec.width - len).max(0) as usize;
    if spec.has(LEFT_JUSTIFY) {
        out.push_str(text);
        out.extend(std::iter::repeat_n(' ', pad));
    } else {
        out.extend(std::iter::repeat_n(' ', pad));
        out.push_str(text);
    }
}

fn leading_sign(sb: &mut String, spec: &Spec, neg: bool) {
    if !neg {
        if spec.has(PLUS) {
            sb.push('+');
        } else if spec.has(LEADING_SPACE) {
            sb.push(' ');
        }
    } else if spec.has(PARENTHESES) {
        sb.push('(');
    } else {
        sb.push('-');
    }
}

fn trailing_sign(sb: &mut String, spec: &Spec, neg: bool) {
    if neg && spec.has(PARENTHESES) {
        sb.push(')');
    }
}

fn adjust_width(width: i32, spec: &Spec, neg: bool) -> i32 {
    if width != -1 && neg && spec.has(PARENTHESES) { width - 1 } else { width }
}

/// Java's `localizedMagnitude` in the US locale: the digits (and `.`) of `value` from `offset`,
/// grouped by threes before the point with the `,` flag, then zero-padded after anything already
/// in `sb` up to `width` with the `0` flag.
fn magnitude(sb: &mut String, value: &str, offset: usize, spec: &Spec, width: i32) {
    let begin = sb.len();
    let value: Vec<char> = value.chars().collect();
    let dot = value.iter().position(|&c| c == '.').unwrap_or(value.len());
    let mut group = spec.has(GROUP);
    for (j, &c) in value.iter().enumerate().skip(offset) {
        if j == dot {
            sb.push('.');
            group = false;
            continue;
        }
        sb.push(c);
        if group && j + 1 != dot && (dot - j) % 3 == 1 {
            sb.push(',');
        }
    }
    let len = sb.chars().count() as i32;
    if width > len && spec.has(ZERO_PAD) {
        sb.insert_str(begin, &"0".repeat((width - len) as usize));
    }
}

fn print_integer(out: &mut String, spec: &Spec, value: i64, bits: u32) -> Result<(), FormatError> {
    let mut sb = String::new();
    match spec.conversion {
        'd' => {
            let neg = value < 0;
            leading_sign(&mut sb, spec, neg);
            let text = value.to_string();
            magnitude(&mut sb, &text, neg as usize, spec, adjust_width(spec.width, spec, neg));
            trailing_sign(&mut sb, spec, neg);
        }
        conversion => {
            check_bad_flags(spec, PARENTHESES | LEADING_SPACE | PLUS)?;
            // A negative value shows its two's complement in its own width.
            let unsigned = if bits == 64 { value as u64 } else { (value as u64) & ((1u64 << bits) - 1) };
            let (digits, prefix) = if conversion == 'o' {
                (format!("{unsigned:o}"), if spec.has(ALTERNATE) { "0" } else { "" })
            } else {
                let prefix = match (spec.has(ALTERNATE), spec.has(UPPERCASE)) {
                    (true, true) => "0X",
                    (true, false) => "0x",
                    _ => "",
                };
                (format!("{unsigned:x}"), prefix)
            };
            sb.push_str(prefix);
            let len = (digits.len() + prefix.len()) as i32;
            if spec.has(ZERO_PAD) && spec.width > len {
                sb.push_str(&"0".repeat((spec.width - len) as usize));
            }
            sb.push_str(&if spec.has(UPPERCASE) { digits.to_uppercase() } else { digits });
        }
    }
    justify(out, spec, &sb);
    Ok(())
}

/// A decimal `f * 10^e` with `n` digits in `f`, as Java's `FormattedFPDecimal` holds it.
struct Decimal {
    f: u64,
    e: i32,
    n: i32,
}

fn pow10(n: i32) -> u64 {
    10u64.pow(n as u32)
}

impl Decimal {
    /// The shortest decimal of a positive finite double, as `Double.toString` chooses it; zero
    /// has no digits.
    fn of(v: f64) -> Decimal {
        if v == 0.0 {
            return Decimal { f: 0, e: 0, n: 0 };
        }
        let (digits, exp) = shortest_digits(v);
        let n = digits.len() as i32;
        Decimal { f: digits.parse().expect("digits"), e: exp - n + 1, n }
    }

    /// Rounds to the `pp` most significant digits, half up.
    fn round(&mut self, pp: i64) {
        if self.n == 0 || pp < 0 {
            *self = Decimal { f: 0, e: 0, n: 1 };
            return;
        }
        if pp >= self.n as i64 {
            return;
        }
        let p = pp as i32;
        self.e += self.n - p;
        let pow = pow10(self.n - p);
        self.f = (self.f + (pow >> 1)) / pow;
        if p == 0 {
            self.n = 1;
            if self.f == 0 {
                self.e = 0;
            }
            return;
        }
        self.n = p;
        if self.f == pow10(p) {
            self.f /= 10;
            self.e += 1;
        }
    }

    fn exponent_rounded(&self) -> i32 {
        self.n + self.e - 1
    }

    fn digits(&self) -> String {
        if self.n == 0 { String::new() } else { format!("{:0width$}", self.f, width = self.n as usize) }
    }

    fn plain_chars(&self) -> String {
        let digits = self.digits();
        if self.e >= 0 {
            format!("{digits}{}", "0".repeat(self.e as usize))
        } else if self.n + self.e > 0 {
            let point = (self.n + self.e) as usize;
            format!("{}.{}", &digits[..point], &digits[point..])
        } else {
            format!("0.{}{digits}", "0".repeat((-(self.n + self.e)) as usize))
        }
    }

    fn scientific_chars(&self, prec: i32) -> (String, String) {
        let digits = self.digits();
        let mantissa =
            if prec != 0 { format!("{}.{}", &digits[..1], &digits[1..]) } else { digits[..1].to_string() };
        let er = self.exponent_rounded();
        let exp = format!("{}{:02}", if er >= 0 { '+' } else { '-' }, er.abs());
        (mantissa, exp)
    }
}

/// Java's `addZeros`: pads the fraction of `mant` with zeros to `prec` digits.
fn add_zeros(mant: &mut String, prec: i32) {
    let (has_dot, out_prec) = match mant.find('.') {
        Some(i) => (true, (mant.len() - i - 1) as i32),
        None => (false, 0),
    };
    if out_prec >= prec {
        return;
    }
    if !has_dot {
        mant.push('.');
    }
    mant.push_str(&"0".repeat((prec - out_prec) as usize));
}

fn print_float(out: &mut String, spec: &Spec, value: f64) {
    let mut sb = String::new();
    let neg = value < 0.0 || (value == 0.0 && value.is_sign_negative());
    if value.is_nan() {
        sb.push_str(if spec.has(UPPERCASE) { "NAN" } else { "NaN" });
    } else {
        let v = value.abs();
        leading_sign(&mut sb, spec, neg);
        if v.is_infinite() {
            sb.push_str(if spec.has(UPPERCASE) { "INFINITY" } else { "Infinity" });
        } else {
            print_finite(&mut sb, spec, v, neg);
        }
        trailing_sign(&mut sb, spec, neg);
    }
    justify(out, spec, &sb);
}

fn print_finite(sb: &mut String, spec: &Spec, v: f64, neg: bool) {
    let upper = spec.has(UPPERCASE);
    let width = spec.width;
    match spec.conversion {
        'e' => {
            let prec = if spec.precision == -1 { 6 } else { spec.precision };
            let mut d = Decimal::of(v);
            d.round(prec as i64 + 1);
            let (mut mant, exp) = d.scientific_chars(prec);
            add_zeros(&mut mant, prec);
            if spec.has(ALTERNATE) && prec == 0 {
                mant.push('.');
            }
            let exp = if v == 0.0 { "+00".to_string() } else { exp };
            let new_width =
                if width != -1 { adjust_width(width - exp.len() as i32 - 1, spec, neg) } else { -1 };
            magnitude(sb, &mant, 0, spec, new_width);
            sb.push(if upper { 'E' } else { 'e' });
            sb.push_str(&exp);
        }
        'f' => {
            let prec = if spec.precision == -1 { 6 } else { spec.precision };
            let mut d = Decimal::of(v);
            d.round(d.n as i64 + d.e as i64 + prec as i64);
            let mut mant = d.plain_chars();
            add_zeros(&mut mant, prec);
            if spec.has(ALTERNATE) && prec == 0 {
                mant.push('.');
            }
            magnitude(sb, &mant, 0, spec, adjust_width(width, spec, neg));
        }
        _ => {
            // 'g'
            let mut prec = match spec.precision {
                -1 => 6,
                0 => 1,
                p => p,
            };
            let (mut mant, exp) = if v == 0.0 {
                prec -= 1;
                ("0".to_string(), None)
            } else {
                let mut d = Decimal::of(v);
                d.round(prec as i64);
                let er = d.exponent_rounded();
                if -4 <= er && er < prec {
                    prec -= er + 1;
                    (d.plain_chars(), None)
                } else {
                    let (mant, exp) = d.scientific_chars(prec - 1);
                    prec -= 1;
                    (mant, Some(exp))
                }
            };
            add_zeros(&mut mant, prec);
            if spec.has(ALTERNATE) && prec == 0 {
                mant.push('.');
            }
            let new_width = match (&exp, width) {
                (_, -1) => -1,
                (Some(exp), w) => adjust_width(w - exp.len() as i32 - 1, spec, neg),
                (None, w) => adjust_width(w, spec, neg),
            };
            magnitude(sb, &mant, 0, spec, new_width);
            if let Some(exp) = exp {
                sb.push(if upper { 'E' } else { 'e' });
                sb.push_str(&exp);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(format: &str, args: &[Arg]) -> String {
        match try_format(format, args) {
            Ok(s) => s,
            Err(e) => format!("{}: {}", e.class, e.message),
        }
    }

    #[test]
    fn integers_format_like_java() {
        assert_eq!(
            f("%d|%5d|%-5d|%05d", &[Arg::Int(42), Arg::Int(-42), Arg::Int(7), Arg::Int(-7)]),
            "42|  -42|7    |-0007"
        );
        assert_eq!(
            f("%,d %+d %(d % d", &[Arg::Long(1234567), Arg::Int(5), Arg::Int(-5), Arg::Int(5)]),
            "1,234,567 +5 (5)  5"
        );
        assert_eq!(
            f("%x %X %o %#x %08x", &[Arg::Int(-1), Arg::Int(255), Arg::Int(8), Arg::Int(255), Arg::Int(255)]),
            "ffffffff FF 10 0xff 000000ff"
        );
        assert_eq!(f("%x", &[Arg::Byte(-1)]), "ff");
        assert_eq!(
            f("%d", &[Arg::Str("x")]),
            "java.util.IllegalFormatConversionException: d != java.lang.String"
        );
    }

    #[test]
    fn floats_round_half_up_from_the_shortest_decimal() {
        assert_eq!(
            f(
                "%.1f %.2f %.0f %.0f",
                &[Arg::Double(0.15), Arg::Double(0.125), Arg::Double(0.5), Arg::Double(1.5)]
            ),
            "0.2 0.13 1 2"
        );
        assert_eq!(f("%f", &[Arg::Double(3.25159)]), "3.251590");
        assert_eq!(f("%.3f", &[Arg::Double(-0.0)]), "-0.000");
        assert_eq!(f("%,.2f", &[Arg::Double(1234567.891)]), "1,234,567.89");
        assert_eq!(
            f("%10.3f|%-10.1f|%010.2f", &[Arg::Double(3.25159), Arg::Double(2.5), Arg::Double(-3.5)]),
            "     3.252|2.5       |-000003.50"
        );
        assert_eq!(
            f("%e %.2E %e", &[Arg::Double(12345.678), Arg::Double(0.000123), Arg::Double(0.0)]),
            "1.234568e+04 1.23E-04 0.000000e+00"
        );
        assert_eq!(
            f("%g %g %g", &[Arg::Double(0.0001), Arg::Double(123456789.0), Arg::Double(0.0)]),
            "0.000100000 1.23457e+08 0.00000"
        );
        assert_eq!(f("%f %f", &[Arg::Double(f64::NAN), Arg::Double(f64::NEG_INFINITY)]), "NaN -Infinity");
        assert_eq!(f("%.2f", &[Arg::Float(0.1)]), "0.10");
        assert_eq!(f("%.20f", &[Arg::Double(0.1)]), "0.10000000000000000000");
    }

    #[test]
    fn text_formats_like_java() {
        assert_eq!(
            f(
                "%s|%10s|%-6s|%.2s|%S",
                &[Arg::Str("hi"), Arg::Str("right"), Arg::Str("left"), Arg::Str("cut"), Arg::Str("up")]
            ),
            "hi|     right|left  |cu|UP"
        );
        assert_eq!(
            f(
                "%b %b %B %c %c%n%%",
                &[Arg::Null, Arg::Str("x"), Arg::Bool(true), Arg::Char(65), Arg::Int(0x1F600)]
            ),
            "false true TRUE A 😀\n%"
        );
        assert_eq!(f("%2$s %1$s %<s", &[Arg::Str("a"), Arg::Str("b")]), "b a a");
        assert_eq!(f("%s %s", &[Arg::Double(1.0), Arg::Float(0.1)]), "1.0 0.1");
        assert_eq!(f("%s", &[]), "java.util.MissingFormatArgumentException: Format specifier '%s'");
        assert_eq!(f("%q", &[]), "java.util.UnknownFormatConversionException: Conversion = 'q'");
        assert_eq!(f("%h", &[Arg::Str("hello")]), "5e918d2");
        assert!(check_supported("%a").is_err());
        assert!(check_supported("%tY").is_err());
        assert!(check_supported("%d %s").is_ok());
    }
}
