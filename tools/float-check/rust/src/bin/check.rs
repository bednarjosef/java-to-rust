//! Runtime side of the float check: reads one bit pattern per line (hex) from stdin and prints
//! `hex TAB float_to_string(...)` (or `double_to_string` with the `double` argument), the same
//! lines `java FloatCheck.java` prints.

use java_runtime::{double_to_string, float_to_string};
use std::io::{self, BufRead, BufWriter, Write};

fn main() -> io::Result<()> {
    let kind = std::env::args().nth(1).unwrap_or_default();
    let is_float = match kind.as_str() {
        "float" => true,
        "double" => false,
        _ => {
            eprintln!("usage: check float|double < bits");
            std::process::exit(2);
        }
    };
    let mut out = BufWriter::with_capacity(1 << 16, io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let line = line?;
        let value = if is_float {
            float_to_string(f32::from_bits(u32::from_str_radix(&line, 16).expect("hex float bits")))
        } else {
            double_to_string(f64::from_bits(u64::from_str_radix(&line, 16).expect("hex double bits")))
        };
        writeln!(out, "{line}\t{value}")?;
    }
    out.flush()
}
