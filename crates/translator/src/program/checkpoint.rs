//! A census's checkpoint: the program as lowering and the walk of the code that can run leave it,
//! saved to the file that `JAVA_TO_RUST_CENSUS_CHECKPOINT` names, so that a census of a large
//! program (the Minecraft server: javac, reach and lowering take a quarter of an hour) can be
//! taken again from there, past changes to the analyses or the generator.
//!
//! With the variable set, a census whose file does not exist yet writes it once the program is
//! lowered, and one whose file exists reads it instead of compiling, reaching and lowering the
//! program. The file is valid only for the same program (it records the main class, sources and
//! libraries, and is refused for others) and for a translator whose javac, reach and lowering
//! give the same: a change to those needs a new checkpoint, which deleting the file makes.

use super::Program;
use serde::{Serialize, de::DeserializeOwned};
use std::io::{BufReader, BufWriter, Write};

/// What a checkpoint is of: the program's main class, sources and libraries.
fn key(program: &Program) -> String {
    format!("{} {:?} {:?} {:?}", program.main, program.sources, program.files, program.libraries)
}

fn path() -> Option<std::path::PathBuf> {
    std::env::var_os("JAVA_TO_RUST_CENSUS_CHECKPOINT").map(Into::into)
}

/// Writes the checkpoint, when one is asked for and its file does not exist.
pub(super) fn save<T: Serialize>(program: &Program, state: &T) {
    let Some(path) = path().filter(|p| !p.exists()) else { return };
    let start = std::time::Instant::now();
    let partial = path.with_extension("partial");
    let written = std::fs::File::create(&partial).map_err(|e| e.to_string()).and_then(|file| {
        let mut out = BufWriter::with_capacity(1 << 20, file);
        bincode::serialize_into(&mut out, &key(program)).map_err(|e| e.to_string())?;
        bincode::serialize_into(&mut out, state).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())
    });
    match written.and_then(|()| std::fs::rename(&partial, &path).map_err(|e| e.to_string())) {
        Ok(()) if crate::timings::enabled() => {
            let size = std::fs::metadata(&path).map_or(0, |m| m.len()) >> 20;
            eprintln!(
                "timing: census: checkpoint written: {:.3} s, {size} MB",
                start.elapsed().as_secs_f64()
            );
        }
        Ok(()) => {}
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            eprintln!("the census checkpoint {} could not be written: {e}", path.display());
        }
    }
}

/// Reads the checkpoint, when one is asked for and its file exists.
///
/// # Panics
///
/// When the file is of another program or cannot be read: the census would not be the one asked
/// for.
pub(super) fn load<T: DeserializeOwned>(program: &Program) -> Option<T> {
    let path = path().filter(|p| p.exists())?;
    let start = std::time::Instant::now();
    let file = std::fs::File::open(&path).unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let mut input = BufReader::with_capacity(1 << 20, file);
    let unreadable =
        |e: bincode::Error| format!("the census checkpoint {} is unreadable: {e}", path.display());
    let of: String =
        bincode::deserialize_from(&mut input).map_err(unreadable).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(of, key(program), "the census checkpoint {} is of another program", path.display());
    let state = bincode::deserialize_from(&mut input).map_err(unreadable).unwrap_or_else(|e| panic!("{e}"));
    if crate::timings::enabled() {
        eprintln!("timing: census: checkpoint read: {:.3} s", start.elapsed().as_secs_f64());
    }
    Some(state)
}

/// `javac::JdkTemps`, whose kinds are kept for the life of the process.
pub(super) mod jdk_temps {
    use crate::ast::{Pos, interned_name};
    use crate::javac::JdkTemps;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(temps: &JdkTemps, s: S) -> Result<S::Ok, S::Error> {
        temps.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<JdkTemps, D::Error> {
        let temps = Vec::<(String, Pos, String, String)>::deserialize(d)?;
        Ok(temps
            .into_iter()
            .map(|(class, pos, kind, name)| (class, pos, interned_name(&kind), name))
            .collect())
    }
}
