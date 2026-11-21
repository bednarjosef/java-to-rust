//! Whole programs: a Java program spread over many files and packages, translated into one Rust
//! crate in which every package is a module (`com.example.util` becomes `com::example::util`),
//! written as one file or as a file per module (`output::Layout`). javac compiles all of it in
//! one run, so that names across files resolve, and only what is reachable from `main` is
//! lowered and translated.

mod census;
mod checkpoint;
mod modules;
mod reach;
mod survey;

pub use census::{
    Census, census_program, generator_again, messages_found_anew, round_by_round, rounds_of_analyses,
    runs_of_generator,
};
pub use modules::{Contents as ModuleContents, loader_name, loader_resources};
pub use reach::{
    ANNOTATION_VALUES, CHANGED_PROPERTY, CLASS_KINDS, CLEANER_CHECK, FIELD_OFFSET_VALUE, FORMAT_IN_LOCALE,
    FieldUpdate, GENERIC_TYPES, ILLEGAL_ACCESS, IN_MODULE, LOCALE_LANGUAGE, Lookup, LookupKind, MODULE_CALLS,
    NATIVE_ACCESS_CALLS, NATIVE_ACCESS_MODULES, PRINT_FORMATTED, REGEX_CALLS, Reach, STATIC_FIELD_VALUES,
    STRING_STREAMS, SUN_MISC_UNSAFE, TRANSLATED_BUFFER_VIEWS, TRANSLATED_LANG_ACCESS,
    TRANSLATED_LANG_ACCESS_CLASS, TRANSLATED_MODULES, TRANSLATED_NESTED, TRANSLATED_PROPERTIES,
    TRANSLATED_SEGMENTS, TRANSLATED_UNSAFE_MEMORY, UNLINKED, UNSET_PROPERTIES, array_for_name,
    buffer_view_access, buffer_view_method, caller_query, charset_coder, charset_coding, class_bound,
    class_for_name, class_name, component_array, computed_for_name, constructs_services, created_by_name,
    direct_stream_call, field_offset_named, field_offset_value, in_jdk, is_ensure_initialized, literal_array,
    literal_class, live_branch, lookup, may_be_malformed, module_call, module_for_name, offset_numbers,
    provided, recreated_exception, redirected_call, redirected_field, release_query, segment_access,
    standard_stream_field, stream_of_call, sun_misc_forwarded, sun_misc_memory, sun_static_field,
    unsafe_access_helper, unsafe_access_helpers, unsafe_field_access, unsafe_field_update,
    unset_property_ifs, untranslated, updater_function, writes_string_array,
};
pub use survey::Survey;

use crate::ast::{Error, Pos};
use crate::output::ModuleTree;
use crate::value::Value;
use crate::{analysis, codegen, javac, lower};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

/// The sources of a program and its entry point.
#[derive(Clone, Debug, Default)]
pub struct Program {
    /// The qualified name of the class whose `main` starts the program, such as
    /// `com.example.Main`.
    pub main: String,
    /// Source roots, laid out by package (`src/com/example/Main.java`): every `.java` file under
    /// them is compiled.
    pub sources: Vec<PathBuf>,
    /// Single source files, compiled with those under the source roots.
    pub files: Vec<PathBuf>,
    /// Library roots, such as the JDK's own sources: javac reads a file from them only when the
    /// program uses one of its classes, and only what is reachable from `main` is translated.
    pub libraries: Vec<PathBuf>,
    /// javac's compile errors do not stop the translation. A piece of code (a method, initializer
    /// or class) whose only errors name classes missing from the roots throws
    /// `NoClassDefFoundError` where it would use one, as on the JVM (`java/MissingClasses.java`);
    /// a piece with any other error is rejected (see `javac::Session::start`). For code that is
    /// known to have some, such as a library whose optional dependencies are not given, or a
    /// decompiler's output.
    pub keep_going: bool,
    /// The class files of the program and its libraries name their locals (they were compiled
    /// with `-g`, as the Minecraft server's were): the message of a `NullPointerException` shows
    /// a local by its name, as it shows the JDK's (`javac::program_names`).
    pub named_locals: bool,
    /// The message of a `NullPointerException` that a `catch` may read, where the translator is
    /// not certain of the JVM's wording, is approximated rather than the code rejected: the JVM's
    /// words for what failed without where the null came from, or no message where even that is
    /// not certain (`analysis::npe_messages`). A census lists each such place.
    pub approximate_messages: bool,
    /// The packages that a module exports beyond its declaration, as Java's `--add-exports
    /// module/package=to` adds them: the module, the package, and the module it is exported to,
    /// `ALL-UNNAMED` for the class path's code (`Reach::added_exports`, `lower/modules.rs`).
    pub add_exports: Vec<(String, String, String)>,
    /// For a census only: its translation is written, and each piece of rejected code in it ends
    /// the program with the rejection's message, where a census otherwise leaves a rejected
    /// initializer out (`lower::census_lower_program`, `census::Stubs`).
    pub ends_rejected: bool,
    /// The classes that the lookups of each kind also look in: those that the flow of `Class`
    /// values of a translation found that they get, which reachability did not take for
    /// candidates (`analysis::reflection_table::missing`). A translation that finds such classes
    /// is made again with them; empty to start with.
    pub flow_classes: reach::FlowClasses,
}

/// A translation error with the file it is in, when it is in one.
#[derive(Debug)]
pub struct ProgramError {
    pub file: Option<String>,
    pub error: Error,
}

impl fmt::Display for ProgramError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.file {
            Some(file) => write!(f, "{file}:{}", self.error),
            None if self.error.pos == Pos::default() => write!(f, "{}", self.error.message),
            // A position whose file is not known, such as one that several of the JDK's files have.
            None => write!(f, "{}", self.error),
        }
    }
}

impl std::error::Error for ProgramError {}

fn fail(message: String) -> ProgramError {
    ProgramError { file: None, error: Error { pos: Pos::default(), message } }
}

/// Translates a whole program into the source of one Rust program that uses the `java-runtime`
/// crate, as one file.
pub fn translate_program(program: &Program) -> Result<String, ProgramError> {
    translate_program_modules(program).map(|tree| tree.one_file())
}

/// Translates a whole program into the modules of a Rust program that uses the `java-runtime`
/// crate, one per package, for a layout (`output::Layout`) to write as files.
///
/// An `Arrays.sort` or `Arrays.fill` of a `String[]` that is an `Object[]` calls the JDK's method
/// for an `Object[]` (`Reach::string_array_writes`), which a walk reaches only with the writes:
/// without them, lowering rejects it. The translation takes the writes when one can run with
/// them. When the walk without them reaches what it would with them (`load`), as it does where
/// other code reaches the JDK's method, the program is lowered with them first, over the same
/// units and reach: if lowering lowered none, the writes change nothing; if one can run, that is
/// the translation; otherwise it is lowered again without them. A write can run with them and
/// not without them only when the walk reaches it solely through what the writes themselves
/// pass to the JDK's method (`JAVA_TO_RUST_STRING_WRITES_CHECK=1` checks that it does not).
/// Otherwise the program is translated without the writes, and again with them, reached by a
/// walk of their own, where a rejected one can run.
///
/// Where the flow of `Class` values finds that a lookup gets classes that reachability did not
/// take for its candidates (`analysis::reflection_table::missing`), the program is translated
/// again with them (`Program::flow_classes`).
pub fn translate_program_modules(program: &Program) -> Result<ModuleTree, ProgramError> {
    let (units, reach, asked, _) = load(program, Reaching::Walk(false), None)?;
    let Some(asked) = asked else {
        return match translate_reached(program, units, reach, Writes::Without)? {
            Translated::Done(tree) => Ok(tree),
            Translated::Flow(missing) => translate_program_modules(&with_flow_classes(program, missing)),
            Translated::Again(_, _) => {
                let (units, reach, ..) = load(program, Reaching::Walk(true), None)?;
                match translate_reached(program, units, reach, Writes::With)? {
                    Translated::Done(tree) => Ok(tree),
                    Translated::Flow(missing) => {
                        translate_program_modules(&with_flow_classes(program, missing))
                    }
                    Translated::Again(..) => unreachable!("a translation with the writes is not taken again"),
                }
            }
        };
    };
    let (mut units, mut reach, mut writes) = (units, reach, Writes::First);
    loop {
        match translate_reached(program, units, reach, writes)? {
            Translated::Done(tree) => return Ok(tree),
            Translated::Flow(missing) => {
                return translate_program_modules(&with_flow_classes(program, missing));
            }
            Translated::Again(again, next) => {
                // javac's trees were let go: javac is asked for the same units again.
                (units, reach, writes) =
                    load(program, Reaching::Replay(asked.clone(), again, next != Writes::Without), None)
                        .map(|(units, reach, ..)| (units, reach, next))?;
            }
        }
    }
}

/// How a translation or a census (`census::census_reached`) takes the writes of `String` arrays
/// as `Object` arrays (`Reach::string_array_writes`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Writes {
    /// With them, over a reach that is the same with them as without: the translation goes on
    /// with them when lowering lowered none (they change nothing) or one can run, and is taken
    /// without them otherwise.
    First,
    /// With them.
    With,
    /// Without them, and with them instead where a rejected one can run.
    Without,
}

/// What `translate_reached` gives: the translation, or the reach to translate again with, and
/// how, or the classes that the lookups of each kind get that reachability did not take for
/// their candidates.
enum Translated {
    Done(ModuleTree),
    Again(Box<Reach>, Writes),
    Flow(Vec<(crate::ast::TableKind, String)>),
}

/// `program` with the lookups of each kind also looking in the classes `missing`
/// (`Program::flow_classes`), each of which it did not look in before.
pub(crate) fn with_flow_classes(program: &Program, missing: Vec<(crate::ast::TableKind, String)>) -> Program {
    let mut program = program.clone();
    for (kind, class) in missing {
        let kind = match kind {
            crate::ast::TableKind::Method => LookupKind::Method,
            crate::ast::TableKind::Field => LookupKind::Field,
            crate::ast::TableKind::Constructor => LookupKind::Constructor,
        };
        let added = program.flow_classes.entry(kind).or_default().insert(class);
        assert!(added, "a class missing from the reflection table that a translation added before");
    }
    program
}

/// Whether `JAVA_TO_RUST_STRING_WRITES_CHECK` is set: where the writes of `String` arrays are
/// taken first (`Writes::First`), lowering without them checks that one can run then too, or
/// that none was lowered.
pub(crate) fn check_string_array_writes() -> bool {
    std::env::var_os("JAVA_TO_RUST_STRING_WRITES_CHECK").is_some()
}

/// `translate_program`, with these units and reach (`load`) and the writes of `String` arrays as
/// `writes` says.
fn translate_reached(
    program: &Program,
    units: Vec<Value>,
    mut reach: Reach,
    writes: Writes,
) -> Result<Translated, ProgramError> {
    use crate::timings::phase;
    reach.string_array_writes = writes != Writes::Without;
    // The program's files are the first units (`load`).
    let program_units = source_files(program)?.len();
    let (lowered, jdk_temps, without) = {
        let trees: Vec<&Value> = units.iter().map(|u| &u["tree"]).collect();
        let lowered =
            phase("lower", || lower::tolerant_lower_program(&trees, program_units, &reach, &program.main));
        // Whether a rejected write can run without the writes, for the check.
        let without = (writes == Writes::First && check_string_array_writes()).then(|| {
            reach.string_array_writes = false;
            let lowered = lower::tolerant_lower_program(&trees, program_units, &reach, &program.main);
            reach.string_array_writes = true;
            rejected_write_runs(lowered)
        });
        // What the JDK's messages of a `NullPointerException` need of its trees.
        (lowered, javac::jdk_temps(&trees[program_units..]), without)
    };
    let is_write =
        |error: &Error| writes == Writes::Without && error.message.starts_with(lower::STRING_ARRAY_WRITE);
    let wrote = lowered.wrote_string_arrays;
    // javac's trees are not needed once the program is lowered: only the file and the classes
    // of each unit, to say where an error is.
    let kept: Vec<Unit> = units.iter().map(Unit::new).collect();
    drop(units);
    let units = kept;
    let file = |unit: Option<usize>| unit.map(|u| units[u].file.clone());
    let mut class = match lowered.class {
        Ok(class) => class,
        // Where lowering with the writes ended, the translation is as without them.
        Err(_) if writes == Writes::First && wrote => {
            return Ok(Translated::Again(Box::new(reach), Writes::Without));
        }
        Err((_, error)) if is_write(&error) => return Ok(Translated::Again(Box::new(reach), Writes::With)),
        Err((unit, error)) => return Err(ProgramError { file: file(unit), error }),
    };
    stamp_units(&mut class, &lowered.units);
    print_notes(&class, &units.iter().map(|u| u.file.clone()).collect::<Vec<_>>());
    let watched = &lowered.string_array_writes;
    let mut missing = Vec::new();
    let live = || narrow(&mut class, lowered.rejected, None, &|_| false, watched, Some(&mut missing));
    let narrowed = phase("live methods", live);
    if !missing.is_empty() {
        return Ok(Translated::Flow(missing));
    }
    let (running, unresolved, writes_run) = narrowed.expect("nothing stops the walk");
    if let Some(without) = without {
        let with = wrote && writes_run;
        assert_eq!(
            with, without,
            "JAVA_TO_RUST_STRING_WRITES_CHECK: a write of a `String[]` can run with the writes ({with}) and \
             without them ({without})"
        );
    }
    if writes == Writes::First && wrote && !writes_run {
        return Ok(Translated::Again(Box::new(reach), Writes::Without));
    }
    let unresolved = unresolved.into_iter().map(|error| lower::Rejected {
        unit: None,
        anchor: None,
        error,
        ty: 0,
        method: None,
    });
    if let Some(rejected) = running.into_iter().chain(unresolved).next() {
        if is_write(&rejected.error) {
            return Ok(Translated::Again(Box::new(reach), Writes::With));
        }
        let file = file(rejected.unit).or_else(|| locate(&units, &class, rejected.error.pos));
        return Err(ProgramError { file, error: rejected.error });
    }
    drop(reach);
    phase("string identity", || analysis::string_identity(&mut class))
        .map_err(|mut errors| errors.remove(0))
        .map_err(|error| ProgramError { file: locate(&units, &class, error.pos), error })?;
    phase("whole objects", || analysis::whole_objects(&mut class))
        .map_err(|error| ProgramError { file: locate(&units, &class, error.pos), error })?;
    let located = |error: Error| ProgramError { file: locate(&units, &class, error.pos), error };
    let mut facts = phase("analyze", || analysis::analyze(&class)).map_err(located)?;
    // A `catch` reads the message of a `NullPointerException`: it names the class file's code.
    if facts.npe_message_read.is_some() {
        let reached: Vec<String> = units.iter().map(|u| u.file.clone()).collect();
        let mut names = phase("class files", || class_file_names(program, &reached))?;
        names.add_jdk_temps(jdk_temps);
        phase("messages", || {
            analysis::npe_messages(&class, &mut facts, &names, program.approximate_messages)
        })
        .map_err(|mut errors| located(errors.remove(0)))?;
    }
    phase("codegen", || codegen::generate_modules(&class, &facts)).map_err(located).map(Translated::Done)
}

/// Whether a write of a `String` array that `lowered` (without the writes) rejected can run: the
/// error that ends lowering, or a piece of code that it rejected and that can run, as a census
/// finds it (`census::lower_and_narrow`).
fn rejected_write_runs(lowered: lower::Lowered) -> bool {
    let is_write = |error: &Error| error.message.starts_with(lower::STRING_ARRAY_WRITE);
    match lowered.class {
        Err((_, error)) => is_write(&error) || lowered.rejected.iter().any(|r| is_write(&r.error)),
        Ok(mut class) => {
            stamp_units(&mut class, &lowered.units);
            let narrowed = narrow(&mut class, lowered.rejected, None, &|_| false, &[], None);
            let (running, _, _) = narrowed.expect("nothing stops the walk");
            running.iter().any(|r| is_write(&r.error))
        }
    }
}

/// How the class files of the program's code name what the messages of a `NullPointerException`
/// show (`javac::program_names`), with the files of the units that the translation `reached`.
fn class_file_names(program: &Program, reached: &[String]) -> Result<javac::BytecodeNames, ProgramError> {
    let modules = modules::modules(&program.sources)
        .into_iter()
        .filter_map(|m| Some((m.name, m.file.parent()?.to_path_buf())))
        .collect::<Vec<_>>();
    let roots = javac::NameRoots {
        libraries: &program.libraries,
        named_locals: program.named_locals,
        keep_going: program.keep_going,
        reached,
    };
    javac::program_names(&source_files(program)?, &modules, roots)
        .map_err(|error| ProgramError { file: None, error })
}

/// The pieces of code that lowering rejected (`rejected`, stubs in `class`) that can run, and
/// the calls that lowering left unresolved that can run; the methods that cannot run get a body
/// that never runs (`analysis::live_methods::prune`). The reflection table first loses the
/// members that no lookup can find (`analysis::reflection_table`), by the facts that find the
/// methods that can run. When those are not known, every rejection can run and nothing changes
/// (and the unresolved calls are not looked for). `found` are the methods that can run when
/// lowering found them already (`Lowered::live`), before `stamp_units`: the positions of their
/// unresolved calls have no unit. Where they come with their facts (`LiveMethods::facts`), the
/// table is cut by those, and the methods that can run are found again unless it lost nothing.
/// With `missing`, the classes that the facts find a lookup gets that the table lacks
/// (`analysis::reflection_table::missing`) are put there, and where there are any the result is
/// `None`, as the program is to be lowered again. `None`, with `class` as it was, when the
/// methods that can run are looked for here and a rejection that `stops` can run: the walk
/// stops there (`live_methods_unless`). Also whether any of the pieces of code `watched` (each
/// as `lower::Rejected` names its code) can run.
fn narrow(
    class: &mut crate::ast::Class,
    rejected: Vec<lower::Rejected>,
    found: Option<analysis::live_methods::LiveMethods>,
    stops: &dyn Fn(&lower::Rejected) -> bool,
    watched: &[(usize, Option<usize>)],
    missing: Option<&mut Vec<(crate::ast::TableKind, String)>>,
) -> Option<(Vec<lower::Rejected>, Vec<Error>, bool)> {
    let live = match found {
        // Without facts to cut the reflection table by (`LiveMethods::facts`), those found are
        // the methods that can run.
        Some(live) if live.facts.is_none() => Ok(live),
        found => {
            let code: Vec<(usize, Option<usize>)> =
                rejected.iter().filter(|r| stops(r)).map(|r| (r.ty, r.method)).collect();
            analysis::live_methods::live_methods_of_pruned(class, &code, missing, found).transpose()?
        }
    };
    let live = match live {
        Ok(live) => live,
        Err(unknown) => {
            if crate::timings::enabled() {
                eprintln!("timing: live methods: not known for {unknown}, {} rejections", rejected.len());
            }
            return Some((rejected, Vec::new(), !watched.is_empty()));
        }
    };
    let distinct = |rejected: &[lower::Rejected]| {
        rejected.iter().map(|r| r.error.message.as_str()).collect::<std::collections::HashSet<_>>().len()
    };
    let (total, distinct_total) = (rejected.len(), distinct(&rejected));
    let running: Vec<lower::Rejected> = rejected.into_iter().filter(|r| live.runs(r.ty, r.method)).collect();
    if crate::timings::enabled() {
        let (count, kinds) = (running.len(), distinct(&running));
        eprintln!(
            "timing: live methods: {} of {}; {total} rejections ({distinct_total} distinct), {count} of them \
             can run ({kinds} distinct), and {} unresolved calls",
            live.methods.len(),
            class.methods.len(),
            live.unresolved.len()
        );
    }
    class.thread_statics = live.thread_statics.as_ref().map(|t| t.iter().copied().collect());
    let mut lists: std::collections::HashMap<Vec<usize>, std::sync::Arc<[usize]>> = Default::default();
    class.receivers = crate::timings::phase("live methods: receivers", || {
        live.receivers
            .iter()
            .map(|(&id, classes)| {
                let list: Vec<usize> = classes.iter().collect();
                (id, lists.entry(list).or_insert_with_key(|list| list.as_slice().into()).clone())
            })
            .collect()
    });
    let watched_runs = watched.iter().any(|&(ty, method)| live.runs(ty, method));
    crate::timings::phase("live methods: prune", || analysis::live_methods::prune(class, &live));
    Some((running, live.unresolved, watched_runs))
}

/// The units of the program and of the library classes that its reachable code uses, with what
/// that code reaches. javac attributes the program's files, and a library's unit only when
/// reachability names one of its classes: each round reaches from `main` over the units so far
/// and asks javac for those of the classes it named that none of them declares
/// (`Reach::missing`), until it gets no more. What a round reaches only grows with the units, so
/// the last round reaches what a walk over every unit that javac can load would; the units are a
/// few dozen where javac, left to itself, attributes thousands (see `javac::Session`). The
/// program's files come first, in the order given, and then the library's in the order they
/// were asked for, so that the first unsupported construct reported is near the program's code.
///
/// Each round walks only what its units add (`reach::Rounds`), over the units with their records
/// complete (`javac::Records`). Which members a record gets depends on every unit, so a new unit
/// can add members to a record of a unit walked before: the walks take up the unit with them as
/// they go on (`reach::Rounds::replace`). Were a member to go, the round would walk all the units
/// again.
///
/// With `Program::keep_going`, javac goes on past compile errors.
///
/// Without `Reach::string_array_writes`, the load also gives what each round asked javac for,
/// when every round reached what it would have with them
/// (`reach::Rounds::same_with_string_array_writes`). The rounds with them would then ask for the
/// same classes, get the same units in the same order and reach the same, so the program is
/// lowered with them over these units and reach, or a load with them takes those instead of
/// walking again (`Reaching::Replay`).
///
/// A load of a program that an earlier load of it (`earlier`, with other classes for the lookups
/// of the reflection table, or with the writes) asked javac for units, takes the units that javac
/// gave that load for as long as it asks for the same (`Earlier`). The load gives what javac gave
/// it, round by round (`Transcript`), for a later one.
fn load(program: &Program, reaching: Reaching, earlier: Option<Earlier>) -> Result<Load, ProgramError> {
    match reaching {
        Reaching::Walk(string_array_writes) => load_in_rounds(program, string_array_writes, earlier),
        Reaching::Replay(asked, mut reach, string_array_writes) => {
            reach.string_array_writes = string_array_writes;
            Ok((replay(program, &asked)?, *reach, None, None))
        }
    }
}

/// What `load` gives: the units, what they reach, what each round asked javac for when the
/// rounds would ask for the same with the writes of `String` arrays, and what javac gave the load
/// when it asked javac.
type Load = (Vec<Value>, Reach, Option<Asked>, Option<Transcript>);

/// How `load` gets the units of a program and what they reach.
enum Reaching {
    /// By asking javac for units in rounds as a walk from `main` needs them, with
    /// `Reach::string_array_writes` or without.
    Walk(bool),
    /// From a load without `Reach::string_array_writes` whose every round reached what it would
    /// have with them, with them or without: javac is asked for what that load asked for in each
    /// round (`replay`), and what they reach is what that load reached.
    Replay(Asked, Box<Reach>, bool),
}

/// What each round of a load asked javac for: the classes, and the proxy classes that
/// reachability had found by then (`reach::ProxyClass`), whose sources it writes for javac; with
/// how many units javac gave for them.
#[derive(Clone)]
struct Asked(Vec<(Vec<String>, Vec<reach::ProxyClass>, usize)>);

/// What javac gave a load (`load_in_rounds`), for a later load of the same program
/// (`Earlier`): how many units it gave for the program's files and in each round, with what the
/// round asked for, and the units whose records the load completed, as javac gave them.
struct Transcript {
    /// The units of the program's files, and the bytes of their trees.
    first: (usize, usize),
    rounds: Vec<Round>,
    /// The units whose records were completed, by index, as javac gave them.
    raw: Vec<(usize, Value)>,
}

/// A round of a load: the classes and the proxy classes (`reach::ProxyClass`) it asked javac for,
/// and how many units, of how many bytes of trees, javac gave for them.
struct Round {
    classes: Vec<String>,
    made: Vec<reach::ProxyClass>,
    units: usize,
    bytes: usize,
}

/// The units that javac gave a load, as it gave them, for a later load of the same program: those
/// of the program's files, and those of each round, with what the round asked for. javac gives
/// the same units for the same questions asked in the same order (as `replay` takes it), so a
/// round that asks for what the earlier load's round asked for takes the units given then.
struct Earlier {
    first: (Vec<Value>, usize),
    rounds: std::collections::VecDeque<(Round, Vec<Value>)>,
}

impl Earlier {
    /// The units of a load (`units`, as `load` gives them, with their records completed) as javac
    /// gave them, by its transcript.
    fn new(transcript: Transcript, mut units: Vec<Value>) -> Earlier {
        for (index, raw) in transcript.raw {
            units[index] = raw;
        }
        let mut units = units.into_iter();
        let first = (units.by_ref().take(transcript.first.0).collect(), transcript.first.1);
        let rounds = transcript
            .rounds
            .into_iter()
            .map(|round| {
                let given = units.by_ref().take(round.units).collect();
                (round, given)
            })
            .collect();
        Earlier { first, rounds }
    }
}

/// Where a load gets its units: from javac, or from what javac gave an earlier load of the same
/// program (`Earlier`), for as long as the load asks for what that load asked for. javac is
/// started only once the load asks for something else, and is first asked for what the load
/// asked for before, round by round, so that it gives what it would have given the load from its
/// start.
struct Units<'s> {
    /// Starts javac on the program's files.
    start: &'s dyn Fn() -> Result<(javac::Session, Vec<Value>), ProgramError>,
    /// What a compile error of javac's is, as an error of the program.
    explain: &'s dyn Fn((Option<String>, Error)) -> ProgramError,
    session: Option<javac::Session>,
    earlier: Option<Earlier>,
    /// What each round so far asked for, as `javac::Session::load` takes it, and how many units
    /// came for it.
    sent: Vec<(Vec<String>, usize)>,
    /// How many units came for the program's files.
    first: usize,
    /// The bytes of the trees received so far.
    received: usize,
    /// How many rounds took the units of the earlier load, when there was one.
    taken: Option<usize>,
}

impl<'s> Units<'s> {
    /// The units of the program's files.
    fn start(
        start: &'s dyn Fn() -> Result<(javac::Session, Vec<Value>), ProgramError>,
        explain: &'s dyn Fn((Option<String>, Error)) -> ProgramError,
        earlier: Option<Earlier>,
    ) -> Result<(Units<'s>, Vec<Value>), ProgramError> {
        let taken = earlier.as_ref().map(|_| 0);
        let mut units =
            Units { start, explain, session: None, earlier, sent: Vec::new(), first: 0, received: 0, taken };
        let first = match units.earlier.as_mut() {
            Some(earlier) => {
                let (first, bytes) = std::mem::take(&mut earlier.first);
                units.received = bytes;
                first
            }
            None => {
                let (session, first) = start()?;
                units.received = session.received;
                units.session = Some(session);
                first
            }
        };
        units.first = first.len();
        Ok((units, first))
    }

    /// The units that javac gives for the classes `classes` and the proxy classes `made`, whose
    /// files `line` names after the classes, as `javac::Session::load` takes them; with the bytes
    /// of their trees.
    fn load(
        &mut self,
        classes: &[String],
        made: &[reach::ProxyClass],
        line: Vec<String>,
    ) -> Result<(Vec<Value>, usize), ProgramError> {
        let same = |(round, _): &(Round, Vec<Value>)| round.classes == classes && round.made == made;
        let (units, bytes) = match self.earlier.as_mut().filter(|e| e.rounds.front().is_some_and(same)) {
            Some(earlier) => {
                let (round, units) = earlier.rounds.pop_front().expect("a round to take");
                *self.taken.as_mut().expect("an earlier load") += 1;
                (units, round.bytes)
            }
            None => {
                self.earlier = None;
                if self.session.is_none() {
                    self.session = Some(self.started()?);
                }
                let session = self.session.as_mut().expect("javac is started");
                let before = session.received;
                let units = session.load(&line).map_err(self.explain)?;
                (units, session.received - before)
            }
        };
        self.sent.push((line, units.len()));
        self.received += bytes;
        Ok((units, bytes))
    }

    /// javac, started once the load asks for what the earlier load did not: asked for what each
    /// round so far asked for, for which it gives the units that came for them.
    fn started(&self) -> Result<javac::Session, ProgramError> {
        let (mut session, first) = (self.start)()?;
        let mut given = vec![(first.len(), self.first)];
        for (line, before) in &self.sent {
            given.push((session.load(line).map_err(self.explain)?.len(), *before));
        }
        match given.into_iter().find(|(now, before)| now != before) {
            Some((now, before)) => Err(fail(format!(
                "javac gave {now} units where it gave {before} for the same classes before"
            ))),
            None => Ok(session),
        }
    }
}

/// `load` by walking in rounds (`Reaching::Walk`).
fn load_in_rounds(
    program: &Program,
    string_array_writes: bool,
    earlier: Option<Earlier>,
) -> Result<Load, ProgramError> {
    let files = source_files(program)?;
    let modules = modules::modules(program.sources.iter().chain(&program.libraries));
    let javac_error = |(file, error)| modules::explain(ProgramError { file, error }, &modules);
    let (mut javac, mut reaching) = (std::time::Duration::ZERO, std::time::Duration::ZERO);
    let start = std::time::Instant::now();
    // Where the proxy classes that reachability finds are written (`reach::ProxyClass`).
    let proxies = ProxySources::new(program).map_err(fail)?;
    let start_javac = || {
        javac::Session::start(
            &files,
            &sourcepath(program),
            modules::kept(&modules),
            patches(program),
            program.keep_going,
        )
        .map_err(javac_error)
    };
    let (mut source, mut new) = Units::start(&start_javac, &javac_error, earlier)?;
    let mut transcript =
        Transcript { first: (new.len(), source.received), rounds: Vec::new(), raw: Vec::new() };
    javac += start.elapsed();
    // The units as javac gave them, and the copies of those whose records were completed.
    let (given, completed) = (Store::default(), Store::default());
    let mut records = javac::Records::default();
    // The tree that the rounds walk of each unit, and the units that completing their records
    // changes, by index, with their records and what completing them wrote.
    let mut trees: Vec<&Value> = Vec::new();
    let mut changing: Vec<(usize, &Value, Vec<&Value>, javac::Completion)> = Vec::new();
    let locale_data = reach::locale_data_classes(&program.libraries);
    let providers = modules::service_providers(program.sources.iter().chain(&program.libraries));
    let reflect_config = reach::reflect_config(program.sources.iter().chain(&program.libraries));
    let new_rounds = || {
        let rounds = reach::Rounds::new(&program.main)
            .asking()
            .with_locale_data(locale_data.clone())
            .with_class_roots(program.sources.iter().chain(&program.libraries).cloned().collect())
            .with_providers(providers.clone())
            .with_reflect_config(reflect_config.clone())
            .with_flow_classes(program.flow_classes.clone());
        if string_array_writes { rounds.with_string_array_writes() } else { rounds }
    };
    let mut rounds = new_rounds();
    // The classes javac has been asked for (`reach::Rounds::asked`).
    let mut asked_so_far: BTreeSet<String> = BTreeSet::new();
    let (mut round, mut for_records, mut stale, mut in_place) = (0, 0, 0, 0);
    let check = std::env::var_os("JAVA_TO_RUST_REACH_CHECK").is_some();
    let mut asked_in_rounds = (!string_array_writes).then(Vec::new);
    loop {
        round += 1;
        let start = std::time::Instant::now();
        let reach_before = reaching;
        let units = given.push(std::mem::take(&mut new));
        let changes = records.add(units);
        let complete = |unit: &Value| {
            let mut copy = unit.clone();
            records.complete(&mut copy);
            copy
        };
        // The units walked before whose records get other members with this round's units, and
        // whether those only add to the members they had, which the walks take up as they go on
        // (`reach::Rounds::replace`); otherwise every unit is walked again.
        let mut changed: Vec<(usize, Value)> = Vec::new();
        let mut adds_only = true;
        for (index, unit, of, completed) in &mut changing {
            let now = records.completion(of);
            if now != *completed {
                adds_only &= now.only_adds_to(completed);
                changed.push((*index, complete(unit)));
                *completed = now;
            }
        }
        let walked: Vec<usize> = changed.iter().map(|&(index, _)| index).collect();
        let restart = !adds_only;
        let old = trees.len();
        for (unit, &changes) in units.iter().zip(&changes) {
            if changes {
                changed.push((trees.len(), complete(unit)));
                let of = javac::Records::records_of(unit);
                let completed = records.completion(&of);
                changing.push((trees.len(), unit, of, completed));
            }
            trees.push(&unit["tree"]);
        }
        let (indices, copies): (Vec<usize>, Vec<Value>) = changed.into_iter().unzip();
        for (index, copy) in indices.into_iter().zip(completed.push(copies)) {
            trees[index] = &copy["tree"];
        }
        if restart {
            (for_records, stale) = (for_records + 1, stale + rounds.restarts());
            rounds = new_rounds();
            rounds.asked(&asked_so_far.iter().cloned().collect::<Vec<_>>());
            rounds.add(&trees).map_err(fail)?;
        } else {
            in_place += usize::from(!walked.is_empty());
            for &index in &walked {
                rounds.replace(index, trees[index]);
            }
            rounds.add(&trees[old..]).map_err(fail)?;
        }
        if check {
            let mut scratch = new_rounds();
            scratch.asked(&asked_so_far.iter().cloned().collect::<Vec<_>>());
            scratch.add(&trees).map_err(fail)?;
            check_round(round, &rounds, scratch).map_err(fail)?;
        }
        if asked_in_rounds.is_some() && !rounds.same_with_string_array_writes() {
            asked_in_rounds = None;
        }
        reaching += start.elapsed();
        let round_reach = (reaching - reach_before).as_secs_f64();
        // The proxy classes that javac is to add, by their files, rather than by name.
        let made = rounds.proxy_classes();
        let mut asked = rounds.missing();
        // The classes asked for the first time, by name or as proxy classes.
        let newly: Vec<String> = asked
            .iter()
            .cloned()
            .chain(made.iter().map(|m| m.class.clone()))
            .filter(|class| asked_so_far.insert(class.clone()))
            .collect();
        asked.retain(|class| !made.iter().any(|m| m.class == *class));
        let classes = asked.clone();
        let mut line = asked;
        line.extend(proxies.write(&made).map_err(fail)?.iter().map(|file| format!("+{}", file.display())));
        let start = std::time::Instant::now();
        let bytes;
        (new, bytes) = source.load(&classes, &made, line)?;
        javac += start.elapsed();
        rounds.asked(&newly);
        if let Some(asked) = &mut asked_in_rounds {
            asked.push((classes.clone(), made.clone(), new.len()));
        }
        transcript.rounds.push(Round { classes, made, units: new.len(), bytes });
        if crate::timings::enabled() {
            // Each round as it ends, for a program whose loading takes long.
            let received = source.received as f64 / 1e6;
            let again = match walked.len() {
                _ if restart => ", from the start again".to_string(),
                0 => String::new(),
                n => format!(", {n} units with new members in their records"),
            };
            eprintln!(
                "timing: round {round}: {} units, {} more asked for, {received:.0} MB of trees, reach \
                 {round_reach:.3} s{again}{}",
                trees.len(),
                new.len(),
                crate::timings::memory()
            );
        }
        // The methods that waited for the classes that javac has none of are reached in
        // another round, as external ones (`reach::Rounds::asking`).
        if new.is_empty() && newly.is_empty() {
            break;
        }
    }
    stale += rounds.restarts();
    let mut reach = rounds.finish();
    // The chains asked for are written (`reach::Rounds::finish`); nothing else is.
    if std::env::var_os("JAVA_TO_RUST_REACH_WHY").is_some() {
        std::process::exit(0);
    }
    let roots = program.sources.iter().chain(&program.libraries);
    reach.modules = modules::contents(roots, Some(std::path::Path::new(OVERLAY)));
    reach.jdk_resources = reach::read_jdk_resources(&reach.jdk_resource_names());
    reach.added_exports = program.add_exports.clone();
    let changing: Vec<usize> = changing.iter().map(|&(index, ..)| index).collect();
    let mut units = given.into_values();
    for index in changing {
        transcript.raw.push((index, units[index].clone()));
        records.complete(&mut units[index]);
    }
    let file_name = |u: &Value| u["file"].as_str().map(|f| f.rsplit('/').next().unwrap_or(f).to_string());
    reach.unit_files = units.iter().map(|u| file_name(u).unwrap_or_default()).collect();
    if crate::timings::enabled() {
        let (javac, reaching) = (javac.as_secs_f64(), reaching.as_secs_f64());
        let (classes, methods) = (reach.classes.len(), reach.methods.len());
        let memory = crate::timings::memory();
        let received = source.received as f64 / 1e6;
        let taken = match source.taken {
            Some(taken) => format!(" ({taken} of {round} rounds as an earlier load got them)"),
            None => String::new(),
        };
        eprintln!(
            "timing: javac: {javac:.3} s, {} units{taken}, {received:.0} MB of trees{memory}",
            units.len()
        );
        eprintln!(
            "timing: reach: {reaching:.3} s, {round} rounds ({} walks from the start again: {for_records} for records, \
             {stale} stale; {in_place} rounds added members to records walked before), {classes} classes and \
             {methods} methods",
            for_records + stale
        );
    }
    // What was reached is written, for comparing two reachabilities; nothing else is.
    if let Some(path) = std::env::var_os("JAVA_TO_RUST_REACH_DUMP") {
        std::fs::write(&path, reach.dump())
            .map_err(|e| fail(format!("{}: {e}", Path::new(&path).display())))?;
        std::process::exit(0);
    }
    Ok((units, reach, asked_in_rounds.map(Asked), Some(transcript)))
}

/// With `JAVA_TO_RUST_REACH_CHECK`, that a round reached what a walk from scratch over the same
/// units, with the same classes asked for, reaches (`scratch`): the rounds only add to what they
/// reached, so each must reach what one walk would. When the variable names a path, the two
/// reaches are written next to it (`.rounds`, `.scratch`); with `JAVA_TO_RUST_REACH_WHY`, the
/// chains of the walk from scratch are written.
fn check_round(round: usize, rounds: &reach::Rounds, scratch: reach::Rounds) -> Result<(), String> {
    let (incremental, from_scratch) = (rounds.dump(), scratch.dump());
    if incremental == from_scratch {
        return Ok(());
    }
    if let Some(prefix) = std::env::var("JAVA_TO_RUST_REACH_CHECK").ok().filter(|p| !p.is_empty()) {
        let _ = std::fs::write(format!("{prefix}.rounds"), &incremental);
        let _ = std::fs::write(format!("{prefix}.scratch"), &from_scratch);
    }
    scratch.finish();
    let only = |a: &str, b: &str| {
        let b: BTreeSet<&str> = b.lines().collect();
        a.lines().filter(|l| !b.contains(l)).take(20).collect::<Vec<_>>().join("\n")
    };
    Err(format!(
        "round {round} reached other than a walk from scratch;\nonly in the round:\n{}\nonly from scratch:\n{}",
        only(&incremental, &from_scratch),
        only(&from_scratch, &incremental)
    ))
}

/// The units of `load_in_rounds`, from what a load without `Reach::string_array_writes` asked
/// javac for in each round (`Reaching::Replay`): javac is asked for the same classes,
/// and adds the same proxy classes, in the same rounds, and so gives the same units in the same
/// order, whose records are completed as that load completes them.
fn replay(program: &Program, asked: &Asked) -> Result<Vec<Value>, ProgramError> {
    let files = source_files(program)?;
    let modules = modules::modules(program.sources.iter().chain(&program.libraries));
    let javac_error = |(file, error)| modules::explain(ProgramError { file, error }, &modules);
    let start = std::time::Instant::now();
    let proxies = ProxySources::new(program).map_err(fail)?;
    let (mut session, mut new) = javac::Session::start(
        &files,
        &sourcepath(program),
        modules::kept(&modules),
        patches(program),
        program.keep_going,
    )
    .map_err(javac_error)?;
    let mut records = javac::Records::default();
    let (mut units, mut changing) = (Vec::new(), Vec::new());
    for (classes, made, given) in &asked.0 {
        let changes = records.add(&new);
        changing.extend(
            changes.iter().enumerate().filter(|(_, changes)| **changes).map(|(i, _)| units.len() + i),
        );
        units.append(&mut new);
        let mut asked = classes.clone();
        asked.extend(proxies.write(made).map_err(fail)?.iter().map(|file| format!("+{}", file.display())));
        new = session.load(&asked).map_err(javac_error)?;
        if new.len() != *given {
            let message =
                format!("javac gave {} units where it gave {given} for the same classes before", new.len());
            return Err(fail(message));
        }
    }
    for index in changing {
        records.complete(&mut units[index]);
    }
    if crate::timings::enabled() {
        let javac = start.elapsed().as_secs_f64();
        let (memory, received) = (crate::timings::memory(), session.received as f64 / 1e6);
        eprintln!("timing: javac: {javac:.3} s, {} units, {received:.0} MB of trees{memory}", units.len());
        eprintln!(
            "timing: reach: the reach of the load without the writes of `String[]`s, over the units of its {} rounds",
            asked.0.len()
        );
    }
    Ok(units)
}

/// The directory that the proxy classes of a translation with libraries are written to as
/// reachability finds them (`reach::ProxyClass`), for javac to add to the program by their files
/// (`javac::Session::load`): one of its own under the system's temporary directory, removed
/// when the translation has loaded its units.
struct ProxySources {
    root: Option<PathBuf>,
}

impl ProxySources {
    fn new(program: &Program) -> Result<ProxySources, String> {
        if program.libraries.is_empty() {
            return Ok(ProxySources { root: None });
        }
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("java-to-rust-proxies-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|e| format!("cannot make {}: {e}", root.display()))?;
        Ok(ProxySources { root: Some(root) })
    }

    /// Writes the sources of the classes that are not written yet, and returns their files.
    fn write(&self, classes: &[reach::ProxyClass]) -> Result<Vec<PathBuf>, String> {
        let Some(root) = &self.root else { return Ok(Vec::new()) };
        let mut written = Vec::new();
        for made in classes {
            let file = root.join(format!("{}.java", made.class.replace('.', "/")));
            if file.exists() {
                continue;
            }
            let wrote = std::fs::create_dir_all(file.parent().expect("a file is in the directory"))
                .and_then(|()| std::fs::write(&file, &made.source));
            wrote.map_err(|e| format!("cannot write {}: {e}", file.display()))?;
            written.push(file);
        }
        Ok(written)
    }
}

impl Drop for ProxySources {
    fn drop(&mut self) {
        if let Some(root) = &self.root {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

/// Values that stay where they are while more are added, so that the rounds of `load` can keep
/// references to the units of earlier rounds: a list of batches, each added behind the last.
#[derive(Default)]
struct Store {
    first: std::cell::OnceCell<Box<Batch>>,
}

struct Batch {
    values: Vec<Value>,
    next: std::cell::OnceCell<Box<Batch>>,
}

impl Store {
    /// Adds a batch of values, and returns them where they now stay.
    fn push(&self, values: Vec<Value>) -> &[Value] {
        let mut last = &self.first;
        while let Some(batch) = last.get() {
            last = &batch.next;
        }
        &last.get_or_init(|| Box::new(Batch { values, next: std::cell::OnceCell::new() })).values
    }

    /// Every value, in the order added.
    fn into_values(self) -> Vec<Value> {
        let mut values = Vec::new();
        let mut next = self.first.into_inner();
        while let Some(batch) = next {
            let Batch { values: batch, next: rest } = *batch;
            values.extend(batch);
            next = rest.into_inner();
        }
        values
    }
}

/// What is kept of a unit once the program is lowered: its file and the qualified names of its
/// top-level classes.
struct Unit {
    file: String,
    classes: Vec<String>,
}

impl Unit {
    fn new(unit: &Value) -> Unit {
        let decls = unit["tree"]["typeDecls"].as_array().into_iter().flatten();
        Unit {
            file: unit["file"].as_str().unwrap_or_default().to_string(),
            classes: decls.map(class_name).collect(),
        }
    }
}

/// Prints the assumptions of the translation (`ast::Note`) to standard error, each at its file
/// (`files`, by unit), line and column.
pub(crate) fn print_notes(class: &crate::ast::Class, files: &[String]) {
    for note in &class.notes {
        let file = note.pos.unit.and_then(|u| files.get(u as usize)).map_or("", String::as_str);
        eprintln!("note: {file}:{}:{}: {}", note.pos.line, note.pos.column, note.message);
    }
}

/// Stamps on the positions of the code of each class the unit it is in (`Pos::unit`): those of
/// the methods, of the statements and expressions in them (those of switch expressions too) and
/// of the fields' initializers. `units` is the unit of each type, `None` for the types that
/// lowering provides, whose code gets `PROVIDED_UNIT`. It can stamp code again: what it stamps
/// depends on the classes alone.
pub(crate) fn stamp_units(class: &mut crate::ast::Class, units: &[Option<usize>]) {
    use crate::ast::{ExprKind, PROVIDED_UNIT, rewrite_expr, rewrite_stmt};
    let unit_of = |t: usize| {
        Some(
            units
                .get(t)
                .copied()
                .flatten()
                .map_or(PROVIDED_UNIT, |u| u32::try_from(u).unwrap_or(PROVIDED_UNIT)),
        )
    };
    for note in &mut class.notes {
        note.pos.unit = note.method.and_then(|m| unit_of(class.methods[m].class));
    }
    for method in &mut class.methods {
        let unit = unit_of(method.class);
        method.pos.unit = unit;
        for s in &mut method.body {
            stamp_statements(s, unit);
            rewrite_stmt(s, &mut |x| {
                x.pos.unit = unit;
                if let ExprKind::Switch { cases, .. } = &mut x.kind {
                    cases.iter_mut().flat_map(|c| &mut c.body).for_each(|s| stamp_statements(s, unit));
                }
            });
        }
    }
    for (t, decl) in class.types.iter_mut().enumerate() {
        let unit = unit_of(t);
        for field in decl.fields.iter_mut().chain(&mut decl.statics) {
            field.pos.unit = unit;
            if let Some(init) = &mut field.init {
                rewrite_expr(init, &mut |x| x.pos.unit = unit);
            }
        }
    }
}

/// Stamps `unit` on the positions of statement `s` and of the statements in it, not those of
/// their expressions: a local's declaration, a `return`, a `catch` clause.
fn stamp_statements(s: &mut crate::ast::Stmt, unit: Option<u32>) {
    use crate::ast::Stmt;
    let each = |stmts: &mut [Stmt]| stmts.iter_mut().for_each(|s| stamp_statements(s, unit));
    match s {
        Stmt::Local { pos, .. } | Stmt::Return(_, pos) => pos.unit = unit,
        Stmt::Block(stmts) => each(stmts),
        Stmt::If { then, otherwise, .. } => {
            stamp_statements(then, unit);
            if let Some(o) = otherwise {
                stamp_statements(o, unit);
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::ForEach { body, .. }
        | Stmt::Labeled { body, .. } => stamp_statements(body, unit),
        Stmt::For { init, body, .. } => {
            each(init);
            stamp_statements(body, unit);
        }
        Stmt::Switch { cases, .. } => cases.iter_mut().for_each(|c| each(&mut c.body)),
        Stmt::Try { body, catches, finally } => {
            each(body);
            for catch in catches {
                catch.pos.unit = unit;
                each(&mut catch.body);
            }
            if let Some(finally) = finally {
                each(finally);
            }
        }
        Stmt::Expr(_) | Stmt::Break | Stmt::Continue | Stmt::BreakLabel(_) | Stmt::ContinueLabel(_) => {}
        Stmt::Yield(_) | Stmt::Throw(_) => {}
    }
}

/// The file of the code at `pos` that an analysis or the generator rejected: the unit stamped on
/// the position (`stamp_units`; none in the code that lowering provides), or for a position
/// without one, the unit of the classes whose methods have code there, when they agree on one (a
/// line and column of one file may be that of another file's code too).
fn locate(units: &[Unit], class: &crate::ast::Class, pos: Pos) -> Option<String> {
    use crate::ast::{visit_expr, visit_stmt};
    if pos.unit.is_some() {
        return pos.source_unit().and_then(|unit| units.get(unit)).map(|u| u.file.clone());
    }
    let mut files = std::collections::BTreeSet::new();
    for method in &class.methods {
        let mut here = method.pos == pos;
        let decl = &class.types[method.class];
        let fields = decl.fields.iter().chain(&decl.statics).filter_map(|f| f.init.as_ref());
        fields.for_each(|e| visit_expr(e, &mut |x| here |= x.pos == pos));
        for s in &method.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut |x| here |= x.pos == pos), &mut |_| {});
        }
        if !here {
            continue;
        }
        // The unit that declares the class, or the class that it is nested in; for the class of
        // a lambda, a local or an anonymous class, the class whose code has it.
        let name = match &decl.origin {
            crate::ast::Origin::Lambda { binary, .. }
            | crate::ast::Origin::Local { binary, .. }
            | crate::ast::Origin::Anonymous { binary, .. } => binary.split('$').next().unwrap_or_default(),
            _ => decl.name.as_str(),
        };
        let unit = units.iter().find(|u| {
            u.classes.iter().any(|top| {
                name == top.as_str()
                    || name.strip_prefix(top.as_str()).is_some_and(|r| r.starts_with(['.', '$']))
            })
        });
        files.insert(unit.map(|u| u.file.as_str())?);
    }
    let mut files = files.into_iter();
    match (files.next(), files.next()) {
        (Some(file), None) => Some(file.to_string()),
        _ => None,
    }
}

/// The survey of what the code reachable from the program's `main` uses that the translator does
/// not support yet, as a text report and as JSON (see `survey`).
pub fn survey_program(program: &Program) -> Result<Survey, ProgramError> {
    let files = source_files(program)?;
    let tree = javac_tree(program, &files)?;
    let units: Vec<&Value> = tree["units"].as_array().into_iter().flatten().collect();
    let trees: Vec<&Value> = units.iter().map(|u| &u["tree"]).collect();
    let mut rounds = reach::Rounds::new(&program.main)
        .with_locale_data(reach::locale_data_classes(&program.libraries))
        .with_class_roots(program.sources.iter().chain(&program.libraries).cloned().collect())
        .with_providers(modules::service_providers(program.sources.iter().chain(&program.libraries)))
        .with_reflect_config(reach::reflect_config(program.sources.iter().chain(&program.libraries)));
    rounds.add(&trees).map_err(fail)?;
    let reach = rounds.finish();
    Ok(survey::survey(&units, &reach, program, &files))
}

/// javac's trees of the program, with those of every library unit that javac loads for it.
fn javac_tree(program: &Program, files: &[PathBuf]) -> Result<Value, ProgramError> {
    let modules = modules::modules(program.sources.iter().chain(&program.libraries));
    javac::program_tree(files, &sourcepath(program), modules::kept(&modules), patches(program))
        .map_err(|(file, error)| modules::explain(ProgramError { file, error }, &modules))
}

/// The roots that javac loads classes from: the program's, then with libraries the translator's
/// own classes that replace or add to the JDK's (`OVERLAY`), then the libraries'.
fn sourcepath(program: &Program) -> Vec<PathBuf> {
    let overlay = (!program.libraries.is_empty()).then(|| PathBuf::from(OVERLAY));
    program.sources.iter().cloned().chain(overlay).chain(program.libraries.iter().cloned()).collect()
}

/// The translator's own classes for a translation with the JDK's sources, which the JDK does not
/// have, ahead of the JDK's on the source path: the objects that `System.in`, `System.out` and
/// `System.err` are (see docs/natives-and-input.md), and the classes that calls of the JDK are
/// redirected to (`reach::REDIRECTED_CALLS`). See docs/jdk-overlays.md.
const OVERLAY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/java/overlay");

/// The translator's patches of the JDK's classes, which javac applies to the JDK's files as it
/// reads them (`java/JdkPatch.java`), with libraries.
fn patches(program: &Program) -> Option<&'static Path> {
    (!program.libraries.is_empty()).then(|| Path::new(PATCHES))
}

/// See `patches`, and docs/jdk-overlays.md.
pub const PATCHES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/java/patches");

/// The program's own files: the given files and every `.java` file under the source roots, sorted
/// and without duplicates. A root's `module-info.java` is not one: javac compiles every root as
/// part of one module (see `modules`).
fn source_files(program: &Program) -> Result<Vec<PathBuf>, ProgramError> {
    let mut files = program.files.clone();
    for root in &program.sources {
        java_files(root, &mut files).map_err(|e| fail(format!("{}: {e}", root.display())))?;
        files.retain(|f| f != &root.join("module-info.java"));
    }
    files.sort();
    files.dedup();
    if files.is_empty() {
        return Err(fail("no Java source files given".into()));
    }
    Ok(files)
}

/// Adds every `.java` file under `dir`, in a stable order.
fn java_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries: Vec<PathBuf> =
        std::fs::read_dir(dir)?.map(|e| e.map(|e| e.path())).collect::<Result<_, _>>()?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            java_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "java") {
            out.push(path);
        }
    }
    Ok(())
}
