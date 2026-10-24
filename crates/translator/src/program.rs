//! Whole programs: a Java program spread over many files and packages, translated into one Rust
//! crate in which every package is a module (`com.example.util` becomes `com::example::util`),
//! written as one file or as a file per module (`output::Layout`). javac compiles all of it in
//! one run, so that names across files resolve, and only what is reachable from `main` is
//! lowered and translated.

mod census;
mod modules;
mod reach;
mod survey;

pub use census::{Census, census_program};
pub use modules::{Contents as ModuleContents, loader_name};
pub use reach::{
    ANNOTATION_VALUES, CLASS_KINDS, FORMAT_IN_LOCALE, FieldUpdate, GENERIC_TYPES, IN_MODULE, LOCALE_LANGUAGE,
    Lookup, LookupKind, MODULE_CALLS, NATIVE_ACCESS_CALLS, NATIVE_ACCESS_MODULES, PRINT_FORMATTED,
    REGEX_CALLS, Reach, STRING_STREAMS, TRANSLATED_LANG_ACCESS, TRANSLATED_LANG_ACCESS_CLASS,
    TRANSLATED_MODULES, TRANSLATED_NESTED, TRANSLATED_PROPERTIES, TRANSLATED_SEGMENTS, UNSET_PROPERTIES,
    caller_query, charset_coder, charset_coding, class_bound, class_for_name, class_name, component_array,
    computed_for_name, created_by_name, direct_stream_call, field_offset_named, is_ensure_initialized,
    literal_class, live_branch, lookup, module_call, module_for_name, provided, recreated_exception,
    redirected_call, redirected_field, release_query, segment_access, standard_stream_field, stream_of_call,
    unsafe_field_access, unsafe_field_update, unset_property_ifs, untranslated, writes_string_array,
};
pub use survey::Survey;

use crate::ast::{Error, Pos};
use crate::output::ModuleTree;
use crate::value::Value;
use crate::{analysis, codegen, javac, lower};
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
    /// For a census only: its translation is written, and each piece of rejected code in it ends
    /// the program with the rejection's message, where a census otherwise leaves a rejected
    /// initializer out (`lower::census_lower_program`, `census::Stubs`).
    pub ends_rejected: bool,
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
pub fn translate_program_modules(program: &Program) -> Result<ModuleTree, ProgramError> {
    // An `Arrays.sort` or `Arrays.fill` of a `String[]` that is an `Object[]` calls the JDK's
    // method for an `Object[]`, which only that program reaches (`Reach::string_array_writes`).
    match translate_reached(program, false) {
        Err(e) if e.error.message.starts_with(lower::STRING_ARRAY_WRITE) => translate_reached(program, true),
        result => result,
    }
}

/// `translate_program`, with `Reach::string_array_writes` or without.
fn translate_reached(program: &Program, string_array_writes: bool) -> Result<ModuleTree, ProgramError> {
    use crate::timings::phase;
    let (units, reach) = load(program, string_array_writes, program.keep_going)?;
    // The program's files are the first units (`load`).
    let program_units = source_files(program)?.len();
    let (lowered, jdk_temps) = {
        let trees: Vec<&Value> = units.iter().map(|u| &u["tree"]).collect();
        let lowered =
            phase("lower", || lower::tolerant_lower_program(&trees, program_units, &reach, &program.main));
        // What the JDK's messages of a `NullPointerException` need of its trees.
        (lowered, javac::jdk_temps(&trees[program_units..]))
    };
    // javac's trees are not needed once the program is lowered: only the file and the classes
    // of each unit, to say where an error is.
    let kept: Vec<Unit> = units.iter().map(Unit::new).collect();
    drop((units, reach));
    let units = kept;
    let file = |unit: Option<usize>| unit.map(|u| units[u].file.clone());
    let mut class = lowered.class.map_err(|(unit, error)| ProgramError { file: file(unit), error })?;
    stamp_units(&mut class, &lowered.units);
    print_notes(&class, &units.iter().map(|u| u.file.clone()).collect::<Vec<_>>());
    if let Err(rejected) = phase("live methods", || keep_live(&mut class, lowered.rejected)) {
        let file = file(rejected.unit).or_else(|| locate(&units, &class, rejected.error.pos));
        return Err(ProgramError { file, error: rejected.error });
    }
    phase("string identity", || analysis::string_identity(&mut class))
        .map_err(|mut errors| errors.remove(0))
        .map_err(|error| ProgramError { file: locate(&units, &class, error.pos), error })?;
    phase("whole objects", || analysis::whole_objects(&mut class))
        .map_err(|error| ProgramError { file: locate(&units, &class, error.pos), error })?;
    let located = |error: Error| ProgramError { file: locate(&units, &class, error.pos), error };
    let mut facts = phase("analyze", || analysis::analyze(&class)).map_err(located)?;
    // A `catch` reads the message of a `NullPointerException`: it names the class file's code.
    if facts.npe_message_read.is_some() {
        let mut names = phase("class files", || class_file_names(program))?;
        names.add_jdk_temps(jdk_temps);
        phase("messages", || analysis::npe_messages(&class, &mut facts, &names))
            .map_err(|mut errors| located(errors.remove(0)))?;
    }
    phase("codegen", || codegen::generate_modules(&class, &facts)).map_err(located)
}

/// How the class files of the program's code name what the messages of a `NullPointerException`
/// show (`javac::program_names`).
fn class_file_names(program: &Program) -> Result<javac::BytecodeNames, ProgramError> {
    let modules = modules::modules(&program.sources)
        .into_iter()
        .filter_map(|m| Some((m.name, m.file.parent()?.to_path_buf())))
        .collect::<Vec<_>>();
    javac::program_names(&source_files(program)?, &modules)
        .map_err(|error| ProgramError { file: None, error })
}

/// Keeps the methods of `class` that can run (`analysis::live_methods`) and gives the others a
/// body that never runs; the first piece of code that lowering rejected and that can run, or
/// else the first call that lowering left unresolved and that can run, if there is one.
fn keep_live(class: &mut crate::ast::Class, rejected: Vec<lower::Rejected>) -> Result<(), lower::Rejected> {
    let (running, unresolved) = narrow(class, rejected, None);
    let unresolved = unresolved.into_iter().map(|error| lower::Rejected {
        unit: None,
        anchor: None,
        error,
        ty: 0,
        method: None,
    });
    running.into_iter().chain(unresolved).next().map_or(Ok(()), Err)
}

/// The pieces of code that lowering rejected (`rejected`, stubs in `class`) that can run, and
/// the calls that lowering left unresolved that can run; the methods that cannot run get a body
/// that never runs (`analysis::live_methods::prune`). When the methods that can run are not
/// known, every rejection can run and nothing changes (and the unresolved calls are not looked
/// for). `found` are the methods that can run when lowering found them already (`Lowered::live`),
/// before `stamp_units`: the positions of their unresolved calls have no unit.
fn narrow(
    class: &mut crate::ast::Class,
    rejected: Vec<lower::Rejected>,
    found: Option<analysis::live_methods::LiveMethods>,
) -> (Vec<lower::Rejected>, Vec<Error>) {
    let live = match found.map_or_else(|| analysis::live_methods::live_methods(class), Ok) {
        Ok(live) => live,
        Err(unknown) => {
            if crate::timings::enabled() {
                eprintln!("timing: live methods: not known for {unknown}, {} rejections", rejected.len());
            }
            return (rejected, Vec::new());
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
    class.thread_statics = live.thread_statics.clone();
    class.receivers =
        live.receivers.iter().map(|(&id, classes)| (id, classes.iter().copied().collect())).collect();
    analysis::live_methods::prune(class, &live);
    (running, live.unresolved)
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
/// complete (`javac::Records`). Which members a record gets depends on every unit, so when a new
/// unit changes those of a unit walked before, the round walks all of them again.
///
/// With `keep_going`, javac goes on past compile errors (`Program::keep_going`).
fn load(
    program: &Program,
    string_array_writes: bool,
    keep_going: bool,
) -> Result<(Vec<Value>, Reach), ProgramError> {
    let files = source_files(program)?;
    let modules = modules::modules(program.sources.iter().chain(&program.libraries));
    let javac_error = |(file, error)| modules::explain(ProgramError { file, error }, &modules);
    let (mut javac, mut reaching) = (std::time::Duration::ZERO, std::time::Duration::ZERO);
    let start = std::time::Instant::now();
    let (mut session, mut new) = javac::Session::start(
        &files,
        &sourcepath(program),
        modules::kept(&modules),
        patches(program),
        keep_going,
    )
    .map_err(javac_error)?;
    javac += start.elapsed();
    // The units as javac gave them, and the copies of those whose records were completed.
    let (given, completed) = (Store::default(), Store::default());
    let mut records = javac::Records::default();
    // The tree that the rounds walk of each unit, and the units that completing their records
    // changes, by index, with their records as completed.
    let mut trees: Vec<&Value> = Vec::new();
    let mut changing: Vec<(usize, &Value, Vec<Value>)> = Vec::new();
    let locale_data = reach::locale_data_classes(&program.libraries);
    let providers = modules::service_providers(program.sources.iter().chain(&program.libraries));
    let new_rounds = || {
        let rounds = reach::Rounds::new(&program.main)
            .with_locale_data(locale_data.clone())
            .with_providers(providers.clone());
        if string_array_writes { rounds.with_string_array_writes() } else { rounds }
    };
    let mut rounds = new_rounds();
    let (mut round, mut walks) = (0, 0);
    loop {
        round += 1;
        let start = std::time::Instant::now();
        let units = given.push(std::mem::take(&mut new));
        let changes = records.add(units);
        let complete = |unit: &Value| {
            let mut copy = unit.clone();
            records.complete(&mut copy);
            copy
        };
        let mut changed: Vec<(usize, Value)> = Vec::new();
        for (index, unit, completed) in &mut changing {
            let now = records.completed_records(unit);
            if now != *completed {
                changed.push((*index, complete(unit)));
                *completed = now;
            }
        }
        let restart = !changed.is_empty();
        let old = trees.len();
        for (unit, &changes) in units.iter().zip(&changes) {
            if changes {
                changed.push((trees.len(), complete(unit)));
                changing.push((trees.len(), unit, records.completed_records(unit)));
            }
            trees.push(&unit["tree"]);
        }
        let (indices, copies): (Vec<usize>, Vec<Value>) = changed.into_iter().unzip();
        for (index, copy) in indices.into_iter().zip(completed.push(copies)) {
            trees[index] = &copy["tree"];
        }
        if restart {
            walks += 1 + rounds.restarts();
            rounds = new_rounds();
            rounds.add(&trees).map_err(fail)?;
        } else {
            rounds.add(&trees[old..]).map_err(fail)?;
        }
        reaching += start.elapsed();
        let start = std::time::Instant::now();
        new = session.load(&rounds.missing()).map_err(javac_error)?;
        javac += start.elapsed();
        if crate::timings::enabled() {
            // Each round as it ends, for a program whose loading takes long.
            let received = session.received as f64 / 1e6;
            eprintln!(
                "timing: round {round}: {} units, {} more asked for, {received:.0} MB of trees{}",
                trees.len(),
                new.len(),
                crate::timings::memory()
            );
        }
        if new.is_empty() {
            break;
        }
    }
    walks += rounds.restarts();
    let mut reach = rounds.finish();
    // The chains asked for are written (`reach::Rounds::finish`); nothing else is.
    if std::env::var_os("JAVA_TO_RUST_REACH_WHY").is_some() {
        std::process::exit(0);
    }
    if !program.libraries.is_empty() {
        let roots = program.sources.iter().chain(&program.libraries);
        reach.modules = modules::contents(roots, Some(std::path::Path::new(OVERLAY)));
    }
    let changing: Vec<usize> = changing.iter().map(|&(index, _, _)| index).collect();
    let mut units = given.into_values();
    for index in changing {
        records.complete(&mut units[index]);
    }
    if crate::timings::enabled() {
        let (javac, reaching) = (javac.as_secs_f64(), reaching.as_secs_f64());
        let (classes, methods) = (reach.classes.len(), reach.methods.len());
        let memory = crate::timings::memory();
        let received = session.received as f64 / 1e6;
        eprintln!("timing: javac: {javac:.3} s, {} units, {received:.0} MB of trees{memory}", units.len());
        eprintln!(
            "timing: reach: {reaching:.3} s, {round} rounds ({walks} walks from the start again), {classes} classes and {methods} methods"
        );
    }
    Ok((units, reach))
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

/// Stamps on the positions of the code of each class the unit it is in (`Pos::unit`): those of
/// the methods, of the expressions in them and of the fields' initializers. `units` is the unit
/// of each type, `None` for the types that lowering provides.
/// Prints the assumptions of the translation (`ast::Note`) to standard error, each at its file
/// (`files`, by unit), line and column.
pub(crate) fn print_notes(class: &crate::ast::Class, files: &[String]) {
    for note in &class.notes {
        let file = note.pos.unit.and_then(|u| files.get(u as usize)).map_or("", String::as_str);
        eprintln!("note: {file}:{}:{}: {}", note.pos.line, note.pos.column, note.message);
    }
}

pub(crate) fn stamp_units(class: &mut crate::ast::Class, units: &[Option<usize>]) {
    use crate::ast::{rewrite_expr, rewrite_stmt};
    let unit_of = |t: usize| units.get(t).copied().flatten().and_then(|u| u32::try_from(u).ok());
    for note in &mut class.notes {
        note.pos.unit = note.method.and_then(|m| unit_of(class.methods[m].class));
    }
    for method in &mut class.methods {
        let unit = unit_of(method.class);
        method.pos.unit = unit;
        method.body.iter_mut().for_each(|s| rewrite_stmt(s, &mut |x| x.pos.unit = unit));
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

/// The file of the code at `pos` that an analysis or the generator rejected: the unit stamped on
/// the position (`stamp_units`), or for a position without one, the unit of the classes whose
/// methods have code there, when they agree on one (a line and column of one file may be that of
/// another file's code too).
fn locate(units: &[Unit], class: &crate::ast::Class, pos: Pos) -> Option<String> {
    use crate::ast::{visit_expr, visit_stmt};
    if let Some(unit) = pos.unit {
        return units.get(unit as usize).map(|u| u.file.clone());
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
        .with_providers(modules::service_providers(program.sources.iter().chain(&program.libraries)));
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
