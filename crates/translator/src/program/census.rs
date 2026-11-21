//! The census of what the translator rejects in a program: the real pipeline (lowering, the
//! analyses and the generator) runs over everything reachable from `main`, but does not stop at
//! the first error. `--census` prints it, and `--census-json` writes it as JSON.
//!
//! Lowering records an error in a method, a constructor or a lambda and lowers a stub in its place,
//! with its signature and a body that never runs; an error in a field initializer or an
//! initializer block leaves that initializer out (`lower::census_lower_program`). An error in a
//! class's declaration (extending or implementing a class that is not translated, or code of a
//! nested class that the planning of its captured variables rejects) rejects the class: it
//! extends and implements none of those, and each of its methods, constructors, field
//! initializers and initializer blocks is rejected with that error and stubbed. As in a
//! translation, only what can run counts: the methods that cannot run (`analysis::live_methods`)
//! get a body that never runs, and what lowering rejected in them is left out. An error of an
//! analysis or of the generator, which only has a line and column, stubs the methods whose code
//! is at that position, in any file (or drops the field initializer there), and the analyses and
//! the generator run again. The errors that they would find in the code alone (the checks of
//! the analyses that read no facts, a class converted to an interface that the runtime's classes
//! do not implement) are found before the analyses first run (`known_before_analyses`), and so
//! cost no round of their own. The generator and the analysis of exceptions report every error
//! they find, which are stubbed together; the other analyses report their first. A run of the
//! analyses goes on past a check that rejects code, to the checks and the messages after it
//! (`analysis::analyze_collecting`), so that the errors of all of them are stubbed in one round,
//! and the census is the one that a round per check finds (`round_by_round`). A panic while the
//! generator writes a method is an error of the method, found with the others. So every error
//! recorded is one that a translation of the program would report once the code before it
//! translated. Where a `catch` reads the message of a `NullPointerException`, the messages are
//! found as a translation finds them (`analysis::npe_messages`), and each dereference whose
//! message is not certain is an error of the analyses, which stubs its code. The writes of
//! `String` arrays as `Object` arrays are taken as a translation takes them
//! (`program::translate_program_modules`, `Reach::string_array_writes`). So the translation that
//! a census writes is the one that a translation would write, but for the stubs. An error that
//! no stub can take away ends the census there, with its place: the pieces it did not get to are
//! counted as they stand. An error that lowering met outside any unit's code is placed in the
//! unit that alone has code at its line and column.
//!
//! The pieces of code are those of the survey (`survey::scan`): the reachable methods,
//! constructors, field initializers and initializer blocks of the program's and its libraries'
//! classes, with the local and anonymous classes and lambdas they declare. An error belongs to
//! the piece of the method or lambda in which lowering met it, unless it is in an initializer
//! that a constructor runs; otherwise, to the piece at its position.
//!
//! Code that translates but knowingly behaves otherwise than Java's is listed apart from the
//! rejections: the pieces that throw `NoClassDefFoundError` for a missing class, as Java would
//! (`Piece::missing_class`), the calls of a handle's method that lowering does not follow,
//! which throw `UnsupportedOperationException` where Java runs them (`thrown_unsupported`), the
//! lookups of a charset by a name known only at run time, which throw it for a charset of the
//! JDK that the translation does not have (`charset_names`), and a security manager's
//! `getClassContext`, which throws it where Java gives the classes on the call stack
//! (`class_contexts`).

use super::checkpoint;
use super::survey::{self, Part, Piece};
use super::{
    Earlier, Program, ProgramError, Reach, Reaching, Transcript, Writes, class_file_names, load, source_files,
};
use crate::ast::{
    Builtin, Class, Error, Expr, ExprId, ExprKind, Method, PROVIDED_UNIT, Pos, Stmt, TableKind, Type,
    visit_expr, visit_stmt, visit_stmt_exprs,
};
use crate::output::ModuleTree;
use crate::value::{Value, json};
use crate::{analysis, codegen, javac, lower};
use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write;

/// Up to this many example places per message.
const EXAMPLES: usize = 3;

thread_local! {
    /// Whether the census on this thread ends each run of the analyses at the first check that
    /// rejects code (`round_by_round`).
    static ROUND_BY_ROUND: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Whether the census on this thread finds the messages of every method anew in each round
    /// (`messages_found_anew`).
    static MESSAGES_ANEW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// How many times the censuses on this thread ran the analyses (`rounds_of_analyses`).
    static ANALYSIS_ROUNDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// How many times the censuses on this thread ran the generator (`runs_of_generator`).
    static GENERATOR_RUNS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Whether the census on this thread runs the generator again for the stubs of its errors
    /// (`generator_again`).
    static GENERATOR_AGAIN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Runs `f` with each census on this thread running the generator again, in a round of its own,
/// for the stubs of the errors that a run of it found, rather than going on with them in the same
/// run (`codegen::generate_all_stubbing`): for a test that the census is the same either way.
pub fn generator_again<T>(f: impl FnOnce() -> T) -> T {
    let before = GENERATOR_AGAIN.replace(true);
    let out = f();
    GENERATOR_AGAIN.set(before);
    out
}

/// Runs `f`, with how many times the censuses it takes on this thread ran the generator: for a
/// test that a census writes the stubs of the generator's errors in the same run.
pub fn runs_of_generator<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let before = GENERATOR_RUNS.replace(0);
    let out = f();
    (out, GENERATOR_RUNS.replace(before))
}

/// Runs `f`, with how many times the censuses it takes on this thread ran the analyses: for a
/// test that the errors found before the analyses cost no round of their own.
pub fn rounds_of_analyses<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let before = ANALYSIS_ROUNDS.replace(0);
    let out = f();
    (out, ANALYSIS_ROUNDS.replace(before))
}

/// Runs `f` with each census on this thread finding the messages of `NullPointerException`s of
/// every method anew in each round, rather than keeping those of the methods that no stub changed
/// (`analysis::KeptMessages`): for a test that the census is the same either way.
pub fn messages_found_anew<T>(f: impl FnOnce() -> T) -> T {
    let before = MESSAGES_ANEW.replace(true);
    let out = f();
    MESSAGES_ANEW.set(before);
    out
}

/// Runs `f` with each census on this thread taken one round per check of the analyses that
/// rejects code, each round ending at that check, rather than with the errors of every check and
/// of the messages in one round (`analysis::analyze_collecting`): for a test that the census is
/// the same either way.
pub fn round_by_round<T>(f: impl FnOnce() -> T) -> T {
    let before = ROUND_BY_ROUND.replace(true);
    let out = f();
    ROUND_BY_ROUND.set(before);
    out
}

/// The census: the text report, and the same census as JSON; and the modules that the generator
/// wrote last, with every rejected piece of code stubbed, if it got that far.
pub struct Census {
    pub report: String,
    pub json: serde_json::Value,
    pub modules: Option<ModuleTree>,
}

/// An error that the census recorded and went on past: the unit of the code it is in (`None`
/// in the code that lowering provides, such as the standard exceptions), the position of the
/// method or lambda it is in, and the error.
#[derive(serde::Serialize, serde::Deserialize)]
struct Rejection {
    unit: Option<usize>,
    anchor: Option<Pos>,
    error: Error,
}

/// The census of the code reachable from the program's `main`, with or without the writes of
/// `String` arrays as `Object` arrays (`Reach::string_array_writes`), as a translation decides
/// (`super::translate_program_modules`).
pub fn census_program(program: &Program) -> Result<Census, ProgramError> {
    census_after(program, None)
}

/// `census_program`, whose load takes the units that javac gave an earlier load of the program
/// (`super::Earlier`) where it asks for the same: one of a census taken again with classes that
/// its reflection table missed.
fn census_after(program: &Program, earlier: Option<Earlier>) -> Result<Census, ProgramError> {
    if let Some(lowered) = checkpoint::load(program) {
        let hook = std::panic::take_hook();
        lower::quiet_panics();
        return Ok(analysed(program, lowered, hook));
    }
    let (units, reach, asked, transcript) = load(program, Reaching::Walk(false), earlier)?;
    // When a walk with the writes would reach the same, the census is first taken with them.
    let same = asked.is_some();
    let mut writes = if same { Writes::First } else { Writes::Without };
    let mut loaded = (units, reach, transcript);
    loop {
        match census_reached(program, loaded, writes, same)? {
            Taken::Census(census) => return Ok(census),
            Taken::Flow(missing, earlier) => {
                return census_after(&super::with_flow_classes(program, missing), earlier);
            }
            Taken::Again(again, next) => (loaded, writes) = (*again, next),
            Taken::Reload(earlier, next) => {
                let (units, reach, _, transcript) = load(program, Reaching::Walk(true), earlier)?;
                (loaded, writes) = ((units, reach, transcript), next);
            }
        }
    }
}

/// The units of a load and what they reach (`super::load`), with what javac gave the load.
type Loaded = (Vec<Value>, Reach, Option<Transcript>);

/// What `census_reached` gives: the census, or the census to take instead.
enum Taken {
    Census(Census),
    /// Over these units and reach, with the writes of `String` arrays as `Writes` says.
    Again(Box<Loaded>, Writes),
    /// Over a load with the writes of `String` arrays, which takes the units that javac gave this
    /// one where it asks for the same.
    Reload(Option<Earlier>, Writes),
    /// Of the program with the classes missing from its reflection table
    /// (`super::with_flow_classes`), over a load that takes the units that javac gave this one
    /// where it asks for the same.
    Flow(Vec<(TableKind, String)>, Option<Earlier>),
}

/// `census_program` over these units and reach (`super::load`), with the writes of `String`
/// arrays as `writes` says; `same`: a walk with them reaches the same as without.
fn census_reached(
    program: &Program,
    loaded: Loaded,
    writes: Writes,
    same: bool,
) -> Result<Taken, ProgramError> {
    let files = source_files(program)?;
    let (units, mut reach, transcript) = loaded;
    reach.string_array_writes = writes != Writes::Without;
    // Panics are rejections in a census: they are reported with the rest, not printed.
    let hook = std::panic::take_hook();
    lower::quiet_panics();
    let trees: Vec<&Value> = units.iter().map(|u| &u["tree"]).collect();
    let narrowed = lower_and_narrow(program, &trees, files.len(), &reach, writes);
    if writes == Writes::First && super::check_string_array_writes() {
        // The census takes the writes when one can run with them; check that one can run
        // without them too, as the census then takes them, or that none was lowered.
        reach.string_array_writes = false;
        let without = lower_and_narrow(program, &trees, files.len(), &reach, Writes::Without).again.is_some();
        reach.string_array_writes = true;
        let with = narrowed.wrote && narrowed.again.is_none();
        assert_eq!(
            with, without,
            "JAVA_TO_RUST_STRING_WRITES_CHECK: a write of a `String[]` can run with the writes ({with}) and \
             without them ({without})"
        );
    }
    let Narrowed { class, mut rejections, mut fatal, again, units: lowered_units, missing, .. } = narrowed;
    if !missing.is_empty() {
        std::panic::set_hook(hook);
        drop((class, trees, reach));
        return Ok(Taken::Flow(missing, transcript.map(|t| Earlier::new(t, units))));
    }
    if let Some(next) = again {
        std::panic::set_hook(hook);
        drop((class, trees));
        if !same {
            drop(reach);
            return Ok(Taken::Reload(transcript.map(|t| Earlier::new(t, units)), next));
        }
        if crate::timings::enabled() {
            eprintln!(
                "timing: javac and reach: the units and reach of the census without the writes of `String[]`s"
            );
        }
        return Ok(Taken::Again(Box::new((units, reach, transcript)), next));
    }
    // What the census needs of javac's trees is taken from them now, so that they are not held
    // while the program is analysed: the pieces of code, the files, and the units of the
    // rejections of lowering that have none. Those of the analyses and the generator get theirs
    // from the stubs (`Stubs::stub`).
    locate(&trees, rejections.iter_mut().filter(|r| r.anchor.is_none()).chain(&mut fatal));
    let scan = survey::scan(&units.iter().collect::<Vec<_>>(), &reach, program, &files);
    // What the JDK's messages of a `NullPointerException` need of its trees.
    let jdk_temps = javac::jdk_temps(&trees[files.len()..]);
    let unit_files = survey::unit_files(&units.iter().collect::<Vec<_>>(), program, &files);
    drop(trees);
    drop((units, reach, transcript));
    let lowered =
        Lowered { class, units: lowered_units, files: unit_files, rejections, fatal, scan, jdk_temps };
    if lowered.class.is_some() {
        checkpoint::save(program, &lowered);
    }
    Ok(Taken::Census(analysed(program, lowered, hook)))
}

/// What the census has of the program once it is lowered and narrowed, which the analyses and
/// the generator take from there (and a checkpoint keeps: `checkpoint`).
#[derive(serde::Serialize, serde::Deserialize)]
struct Lowered {
    class: Option<Class>,
    /// The unit of each type of the program.
    units: Vec<Option<usize>>,
    /// The file of each unit.
    files: Vec<String>,
    rejections: Vec<Rejection>,
    fatal: Option<Rejection>,
    scan: survey::Census,
    #[serde(with = "checkpoint::jdk_temps")]
    jdk_temps: javac::JdkTemps,
}

/// The census of the lowered program: its analyses and generator, past every rejection.
fn analysed(
    program: &Program,
    lowered: Lowered,
    hook: Box<dyn Fn(&std::panic::PanicHookInfo) + Sync + Send>,
) -> Census {
    use crate::timings::phase;
    let Lowered { mut class, units, files, mut rejections, mut fatal, scan, jdk_temps } = lowered;
    // Each error of the analyses and the generator is at the position of code with its unit, so
    // that the census stubs the code of that unit alone at its line and column (`Stubs::stub`).
    // A checkpoint keeps the program as lowering left it.
    if let Some(class) = &mut class {
        super::stamp_units(class, &units);
        super::print_notes(class, &files);
    }
    let mut modules = None;
    let (mut unfollowed, mut charset_names, mut approximated, mut class_contexts, mut offsets) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    if let Some(mut class) = class {
        fatal = phase("analyze", || {
            std::thread::scope(|scope| {
                // The class files' names, which the messages need once the analyses are done, compile
                // (javac, a process of its own) while the analyses run.
                let early = scope.spawn(|| class_file_names(program, &files));
                let names = Names {
                    program,
                    files: &files,
                    jdk_temps: RefCell::new(jdk_temps),
                    early: RefCell::new(Some(early)),
                    found: RefCell::new(None),
                    approximated: RefCell::new(Vec::new()),
                    kept: RefCell::new(match MESSAGES_ANEW.get() {
                        true => analysis::KeptMessages::default(),
                        false => analysis::KeptMessages::new(),
                    }),
                };
                let fatal = analyse(
                    &mut class,
                    &units,
                    &files,
                    &mut rejections,
                    &mut modules,
                    &names,
                    program.ends_rejected,
                );
                approximated = approximated_messages(names.approximated.into_inner());
                fatal
            })
        });
        unfollowed = thrown_unsupported(&class, lower::UNFOLLOWED_MESSAGE);
        charset_names = computed_charset_names(&class);
        class_contexts = thrown_unsupported(&class, CLASS_CONTEXT_MESSAGE);
        offsets = thrown_unsupported(&class, lower::UNRESOLVED_OFFSET);
    }
    std::panic::set_hook(hook);
    let listed = [unfollowed, charset_names, approximated, class_contexts, offsets];
    let tally = Tally::new(&scan.pieces, &files, rejections, fatal, listed);
    let (report, json) = (tally.report(&scan, &program.main), (&tally.json(&scan, &program.main)).into());
    Census { report, json, modules }
}

/// The program lowered for a census and narrowed to the code that can run (`super::narrow`).
struct Narrowed {
    /// The program, unless lowering ended with `fatal` or the census is taken again.
    class: Option<Class>,
    rejections: Vec<Rejection>,
    fatal: Option<Rejection>,
    /// The unit of each type of the program.
    units: Vec<Option<usize>>,
    /// How the census is taken instead, when it is: with the writes of `String` arrays, where one
    /// that lowering rejected without them can run; without them, where lowering with them over
    /// the same reach (`Writes::First`) lowered one and none can run.
    again: Option<Writes>,
    /// Whether lowering lowered a write of a `String` array as a call of the JDK's method.
    wrote: bool,
    /// The classes that a lookup gets that the reflection table lacks (`super::narrow`), with
    /// which the census is taken again.
    missing: Vec<(TableKind, String)>,
}

/// Lowers the program for a census, with the writes of `String` arrays as `writes` says (as
/// `reach` has them), and narrows it to the code that can run.
fn lower_and_narrow(
    program: &Program,
    trees: &[&Value],
    files: usize,
    reach: &Reach,
    writes: Writes,
) -> Narrowed {
    use crate::timings::phase;
    // The program's files are the first units (`load`).
    let lower = || lower::census_lower_program(trees, files, reach, &program.main, program.ends_rejected);
    let lowered = phase("lower", || std::panic::catch_unwind(lower));
    // To time lowering alone, on a program whose census takes long (`docs/performance.md`).
    if std::env::var_os("JAVA_TO_RUST_STOP_AFTER_LOWER").is_some() {
        std::process::exit(0);
    }
    let lowered = lowered.unwrap_or_else(|panic| {
        let message = format!("the translator panicked: {}", lower::panic_message(&*panic));
        lower::Lowered {
            class: Err((None, Error { pos: Pos::default(), message })),
            rejected: Vec::new(),
            units: Vec::new(),
            live: None,
            string_array_writes: Vec::new(),
            wrote_string_arrays: false,
        }
    });
    let rejection = |r: lower::Rejected| Rejection { unit: r.unit, anchor: r.anchor, error: r.error };
    let rejects_writes =
        |message: &str| writes == Writes::Without && message.starts_with(lower::STRING_ARRAY_WRITE);
    let wrote = lowered.wrote_string_arrays;
    let mut rejections = Vec::new();
    // Whether a write that can run takes the census again.
    let mut again = None;
    let mut missing = Vec::new();
    let (class, fatal) = match lowered.class {
        Err((unit, error)) => {
            rejections.extend(lowered.rejected.into_iter().map(rejection));
            (None, Some(Rejection { unit, anchor: None, error }))
        }
        // As a translation does, only what can run counts (the unresolved calls are found again
        // below, where they are stubbed).
        Ok(mut class) => {
            // The census is taken again with the writes of `String` arrays (`census_program`)
            // once one of them can run: looking further, or analysing this program, would
            // change nothing.
            let stops = |r: &lower::Rejected| rejects_writes(&r.error.message);
            let watched = &lowered.string_array_writes;
            let narrow = || {
                super::narrow(&mut class, lowered.rejected, lowered.live, &stops, watched, Some(&mut missing))
            };
            match phase("live methods", narrow) {
                _ if !missing.is_empty() => (None, None),
                // With the writes, over the reach without them, none of those lowered can run.
                Some((_, _, false)) if writes == Writes::First && wrote => {
                    again = Some(Writes::Without);
                    (None, None)
                }
                Some((running, _, _)) if !running.iter().any(stops) => {
                    rejections.extend(running.into_iter().map(rejection));
                    (Some(class), None)
                }
                // The walk stopped at a write, or one can run.
                _ => {
                    again = Some(Writes::With);
                    (None, None)
                }
            }
        }
    };
    if again.is_none() && rejections.iter().chain(&fatal).any(|r| rejects_writes(&r.error.message)) {
        again = Some(Writes::With);
    }
    Narrowed { class, rejections, fatal, units: lowered.units, again, wrote, missing }
}

/// How the class files name what the messages of a `NullPointerException` show, found the first
/// time a `catch` of the program reads one, as a translation finds them.
struct Names<'a> {
    program: &'a Program,
    /// The file of each unit, relative to its root.
    files: &'a [String],
    /// The temps of the JDK's code (`javac::jdk_temps`), until they are added to `found`.
    jdk_temps: RefCell<javac::JdkTemps>,
    /// The class files' names (`class_file_names`) being found on a thread of their own since the
    /// analyses started, until `found` takes them.
    early: RefCell<Option<std::thread::ScopedJoinHandle<'a, Result<javac::BytecodeNames, ProgramError>>>>,
    /// The names, or why the class files could not be compiled for them, once looked for.
    found: RefCell<Option<Found>>,
    /// The dereferences whose message the last pass approximated (`Facts::npe_approximated`).
    approximated: RefCell<Vec<(Pos, Option<Pos>)>>,
    /// The messages of each method's code, kept from pass to pass but for the methods whose code
    /// a stub changed (`Stubs::changed`).
    kept: RefCell<analysis::KeptMessages>,
}

/// The names of the class files, or, where they cannot be compiled, none but those of the JDK's
/// code, with why.
struct Found {
    names: javac::BytecodeNames,
    without: Option<String>,
}

impl Names<'_> {
    /// The messages of `facts`'s dereferences (`analysis::npe_messages`), where a `catch` reads
    /// one; each that is not certain is an error, or with `Program::approximate_messages` is
    /// approximated (`approximated`).
    fn messages(&self, class: &Class, facts: &mut analysis::Facts) -> Result<(), Vec<Error>> {
        self.approximated.borrow_mut().clear();
        if facts.npe_message_read.is_none() {
            return Ok(());
        }
        let mut found = self.found.borrow_mut();
        if found.is_none() {
            let names = crate::timings::phase("census: class files", || match self.early.take() {
                Some(early) => early.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
                None => class_file_names(self.program, self.files),
            });
            // Without the class files (the program does not compile, as decompiled code may
            // not), the messages are found with the names of the JDK's code alone, which do not
            // come from them: each that needs a name of the program's class files is not
            // certain, and its code is rejected there, as a translation rejects an uncertain
            // one, and the census goes on.
            let (names, without) = match names.map_err(|e| e.error) {
                Ok(names) => (Ok(names), None),
                Err(error) => {
                    let why = error.message.lines().next().unwrap_or_default().to_string();
                    (javac::source_names("").map_err(|e| e.message), Some(why))
                }
            };
            let mut names = names.unwrap_or_else(|e| panic!("the names of no code: {e}"));
            names.add_jdk_temps(self.jdk_temps.take());
            if crate::timings::enabled() {
                eprintln!("timing: census: class files: {}", names.sizes());
            }
            *found = Some(Found { names, without });
        }
        let Found { names, without } = found.as_ref().expect("found above");
        let approximate = self.program.approximate_messages;
        let found = crate::timings::phase("census: messages", || {
            analysis::npe_messages_kept(class, facts, names, approximate, &mut self.kept.borrow_mut())
        });
        *self.approximated.borrow_mut() = std::mem::take(&mut facts.npe_approximated);
        found.map_err(|errors| match without {
            // A message of the program's own code, which names no method of the JDK's.
            Some(why) => errors
                .into_iter()
                .map(|e| match e.message.contains(" in `") {
                    true => e,
                    false => Error {
                        pos: e.pos,
                        message: format!(
                            "the message of a `NullPointerException` without the class files of the program ({why})"
                        ),
                    },
                })
                .collect(),
            None => errors,
        })
    }
}

/// An analysis or the generator, run for its errors.
type Pass<'a> = &'a dyn Fn(&mut Class) -> Result<(), Vec<Error>>;

/// Runs the analyses and the generator over the lowered program until they pass, stubbing the
/// code of each error; the error that ends it early, if one does.
fn analyse(
    class: &mut Class,
    units: &[Option<usize>],
    files: &[String],
    rejections: &mut Vec<Rejection>,
    modules: &mut Option<ModuleTree>,
    names: &Names,
    ends_rejected: bool,
) -> Option<Rejection> {
    let generated: std::cell::RefCell<Option<ModuleTree>> = Default::default();
    let stubs = RefCell::new(Stubs {
        next_id: next_id(class) + 1,
        stubbed: HashSet::new(),
        ends_rejected,
        owner: None,
        changed: Vec::new(),
        reshaped: false,
        replaced: HashSet::new(),
    });
    let rejections = RefCell::new(rejections);
    // The errors of a pass stubbed (`Stubs::stub`), each recorded: whether a stub took one away,
    // those that no stub took away, and the methods whose code the stubs changed.
    let stub_errors = |class: &mut Class, index: usize, errors: Vec<Error>| {
        if crate::timings::enabled() {
            let first = errors.first().map_or("", |e| e.message.as_str());
            eprintln!("timing: census: pass {index} rejects {} pieces of code, first: {first}", errors.len());
        }
        let mut stubs = stubs.borrow_mut();
        let mut stuck = Vec::new();
        let mut stubbed = false;
        stubs.reshaped = false;
        for error in errors {
            let rejection = stubs.stub(class, units, error);
            if crate::timings::enabled() {
                let owner = rejection.anchor.and(stubs.owner).map(|m| &class.methods[m]);
                let owner = owner
                    .map_or("nothing".to_string(), |m| format!("{}.{}", class.types[m.class].name, m.name));
                eprintln!(
                    "timing: census: stubbed {owner} at {}: {}",
                    place(files, &rejection),
                    rejection.error.message
                );
            }
            if rejection.anchor.is_some() {
                stubbed = true;
                rejections.borrow_mut().push(rejection);
            } else {
                stuck.push(rejection);
            }
        }
        // The messages are found again for the code the stubs changed alone.
        let changed = std::mem::take(&mut stubs.changed);
        for &method in &changed {
            names.kept.borrow_mut().forget(method);
        }
        (stubbed, stuck, changed)
    };
    // `whole_objects` changes the program only when it passes: an error of it that no stub takes
    // away is recorded, and the census goes on without it. An error of a check of the analyses
    // that only rejects (as of a field's type) that no stub takes away is recorded too, and the
    // analyses go on past it (`analysis::analyze_tolerating`). Any other error of the analyses or
    // the generator that no stub takes away ends it. The generator reports every error it finds,
    // which are stubbed together.
    let tolerated: std::cell::RefCell<Vec<Error>> = Default::default();
    // The facts that depend on the program alone, kept while no stub changes the program: a pass
    // that only tolerates more errors finds them again the same.
    let represented: std::cell::RefCell<Option<analysis::Represented>> = Default::default();
    // A stub ends the program where its code would run, so it only takes behaviour away: the
    // facts of the analyses still hold for the program after it, which may do less than they
    // say but nothing more. A round whose errors the stubs took away by giving methods such a
    // body (`Stubs::reshaped` not set) keeps its facts, and the next round runs the messages and
    // the generator alone on them; any other change, or an error that no stub took away, runs
    // the analyses again.
    let kept: std::cell::RefCell<Option<analysis::Facts>> = Default::default();
    let keep = std::cell::Cell::new(false);
    // The errors of a round that the generator's census hook stubbed already, when it could not
    // go on with the stubs in the same run (`generate`): whether a stub took one away, and those
    // no stub took away.
    let processed: RefCell<Option<(bool, Vec<Rejection>)>> = Default::default();
    // The generator's hook (`codegen::generate_all_stubbing`): the errors of the analyses and the
    // messages (`before`) and those of the generator's first round, stubbed; where the stubs
    // replaced only method bodies and took every error away, the facts are kept and the messages
    // found again for the changed code, as a round with kept facts would do next, and the
    // generator goes on with the stubs. The methods the stubs changed, with the errors of the
    // messages.
    let in_place = |class: &mut Class, facts: &mut analysis::Facts, before: Vec<Error>, found: Vec<Error>| {
        let mut errors = before;
        errors.extend(found);
        if errors.is_empty() {
            return Some((HashSet::new(), Vec::new()));
        }
        let (stubbed, stuck, changed) = stub_errors(class, 3, errors);
        if stubbed {
            represented.take();
        }
        if stubbed && stuck.is_empty() && !stubs.borrow().reshaped {
            forget_replaced(facts, &std::mem::take(&mut stubs.borrow_mut().replaced));
            let errors = names.messages(class, facts).err().unwrap_or_default();
            return Some((changed.into_iter().collect(), errors));
        }
        *processed.borrow_mut() = Some((stubbed, stuck));
        None
    };
    let passes: [Pass; 4] = [
        &|class| crate::timings::phase("census: string identity", || analysis::string_identity(class)),
        &|class| crate::timings::phase("census: whole objects", || analysis::whole_objects_all(class)),
        &|class| {
            let made = crate::timings::phase("census: closure factories", || {
                analysis::inline_closure_factories(class)
            });
            // The stubs' expressions take ids after those of the lambdas that replaced calls.
            if let Some(next) = made {
                let mut stubs = stubs.borrow_mut();
                stubs.next_id = stubs.next_id.max(next);
            }
            unresolved_calls(class)?;
            crate::timings::phase("census: checks of the code", || known_before_analyses(class))
        },
        &|class| {
            let reused = if keep.take() { kept.take() } else { None };
            kept.take();
            if let Some(mut facts) = reused {
                forget_replaced(&mut facts, &std::mem::take(&mut stubs.borrow_mut().replaced));
                let errors = names.messages(class, &mut facts).err().unwrap_or_default();
                return generate(class, facts, errors, &generated, &kept, &in_place);
            }
            ANALYSIS_ROUNDS.set(ANALYSIS_ROUNDS.get() + 1);
            stubs.borrow_mut().replaced.clear();
            let tolerated = tolerated.borrow();
            // The errors of every check, and those of the messages after them, are found in one
            // run (`analysis::analyze_collecting`); a panic after an error ends the run there,
            // with the errors found before it, as the analyses that stop at it end.
            let collected = RefCell::new(Vec::new());
            let after_errors = |panic: Box<dyn std::any::Any + Send>| -> Vec<Error> {
                let errors = collected.take();
                if errors.is_empty() {
                    std::panic::resume_unwind(panic);
                }
                represented.take();
                errors
            };
            let analysed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::timings::phase("census: analyses", || match ROUND_BY_ROUND.get() {
                    true => analysis::analyze_from(class, &tolerated, &mut represented.borrow_mut()),
                    false => analysis::analyze_collecting(
                        class,
                        &tolerated,
                        &mut represented.borrow_mut(),
                        &collected,
                    ),
                })
            }));
            let mut analysed = match analysed {
                Ok(analysed) => analysed?,
                Err(panic) => return Err(after_errors(panic)),
            };
            // Each message that is not certain stubs its code, as a translation rejects it. The
            // objects on the stack, which the messages do not read, are taken once the messages
            // pass.
            let messages = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                names.messages(class, &mut analysed.facts)
            }));
            let messages = match messages {
                Ok(messages) => messages,
                Err(panic) => return Err(after_errors(panic)),
            };
            // The generator runs past the errors of the analyses and the messages too, as they
            // run past each other's, so that one round stubs the errors of all three; its tree
            // is kept only from a round without errors.
            let mut errors = collected.take();
            match messages {
                Err(found) if ROUND_BY_ROUND.get() => return Err(found),
                Err(found) => errors.extend(found),
                Ok(()) => {}
            }
            generate(class, analysed.with_stack_objects(), errors, &generated, &kept, &in_place)
        },
    ];
    for (index, pass) in passes.into_iter().enumerate() {
        loop {
            // A stub can lead the translator where a translation never goes. A panic while the
            // generator writes a method is an error of the method, which is stubbed with the
            // generator's other errors (`codegen::catch_panics`); any other panic ends the census
            // as an error does, with what it found so far.
            let errors = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pass(class))) {
                Ok(Ok(())) => break,
                Ok(Err(errors)) => errors,
                Err(panic) => {
                    let message = format!("the translator panicked: {}", lower::panic_message(&*panic));
                    vec![Error { pos: codegen::panicked_in().unwrap_or_default(), message }]
                }
            };
            let (stubbed, stuck) = match processed.take() {
                Some(done) => done,
                None => {
                    let (stubbed, stuck, _) = stub_errors(class, index, errors);
                    (stubbed, stuck)
                }
            };
            // An error that no stub took away comes back, unless the stubs took away its cause.
            if stubbed {
                represented.take();
                keep.set(stuck.is_empty() && !stubs.borrow().reshaped);
                continue;
            }
            let mut rejections = rejections.borrow_mut();
            if index < passes.len() - 1 {
                rejections.push(stuck.into_iter().next().expect("a pass that fails has an error"));
                break;
            }
            // The analyses go on past an error they met before only if it is one they tolerate.
            let same = |a: &Error, b: &Error| a.pos == b.pos && a.message == b.message;
            let new: Vec<Rejection> = stuck
                .iter()
                .filter(|r| !tolerated.borrow().iter().any(|t| same(t, &r.error)))
                .map(copy)
                .collect();
            if new.is_empty() {
                // It ends the census: it is that error, no longer one recorded and gone past.
                let first = stuck.into_iter().next()?;
                if let Some(at) = rejections.iter().rposition(|r| same(&r.error, &first.error)) {
                    rejections.remove(at);
                }
                return Some(first);
            }
            for rejection in new {
                tolerated
                    .borrow_mut()
                    .push(Error { pos: rejection.error.pos, message: rejection.error.message.clone() });
                rejections.push(rejection);
            }
        }
    }
    *modules = generated.take();
    None
}

/// The census's hook of the generator (`generate`): it stubs the errors before the generator and
/// those of its first round, and gives back the methods it changed with the errors of the
/// messages after, or `None` where the generator does not go on with the stubs.
type InPlace<'a> = dyn Fn(&mut Class, &mut analysis::Facts, Vec<Error>, Vec<Error>) -> Option<(HashSet<usize>, Vec<Error>)>
    + 'a;

/// The generator on the facts of a round of the analyses (`analyse`), after the `errors` of the
/// analyses and the messages: the tree when there are none, or else the errors of all three, with
/// the facts kept for the next round. Unless the census goes round by round, the errors of the
/// analyses and the messages and those of the generator's first round are stubbed by `in_place`
/// while the generator runs, and where the stubs replaced only method bodies, the messages of the
/// changed code are found again and the generator goes on with the stubs
/// (`codegen::generate_all_stubbing`): the errors are then those of the program with the stubs.
fn generate(
    class: &mut Class,
    mut facts: analysis::Facts,
    errors: Vec<Error>,
    generated: &std::cell::RefCell<Option<ModuleTree>>,
    kept: &std::cell::RefCell<Option<analysis::Facts>>,
    in_place: &InPlace,
) -> Result<(), Vec<Error>> {
    GENERATOR_RUNS.set(GENERATOR_RUNS.get() + 1);
    // The errors before the generator, until the hook takes them; then those of the messages
    // with the stubs, where the generator went on with them.
    let before = RefCell::new(Some(errors));
    let after = RefCell::new(None);
    let mut hook = |class: &mut Class, facts: &mut analysis::Facts, found: Vec<Error>| {
        let before = before.borrow_mut().take().unwrap_or_default();
        let (changed, messages) = in_place(class, facts, before, found)?;
        *after.borrow_mut() = Some(messages);
        Some(changed)
    };
    codegen::catch_panics(true);
    let tree = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::timings::phase("census: generator", || match ROUND_BY_ROUND.get() || GENERATOR_AGAIN.get() {
            true => codegen::generate_all(class, &facts),
            false => codegen::generate_all_stubbing(class, &mut facts, &mut hook),
        })
    }));
    codegen::catch_panics(false);
    // The hook takes the errors before the generator unless it did not run.
    let mut errors = before.take().or(after.take()).unwrap_or_default();
    // A panic outside the methods, after errors, may come of the code they reject: the round ends
    // with those errors.
    let tree = match tree {
        Ok(tree) => tree,
        Err(_) if !errors.is_empty() => return Err(errors),
        Err(panic) => std::panic::resume_unwind(panic),
    };
    match tree {
        Ok(tree) if errors.is_empty() => {
            *generated.borrow_mut() = Some(tree);
            return Ok(());
        }
        Ok(_) => {}
        Err(found) => errors.extend(found),
    }
    if !ROUND_BY_ROUND.get() {
        *kept.borrow_mut() = Some(facts);
    }
    Err(errors)
}

/// Takes out of facts kept from before stubs what they say of the code the stubs `replaced`, which
/// is not in the program any more: its dereferences, which the generator checks are written
/// (`Facts::null_checked`) and the messages check have a message (`Facts::npe_message_needed`).
fn forget_replaced(facts: &mut analysis::Facts, replaced: &HashSet<ExprId>) {
    if replaced.is_empty() {
        return;
    }
    facts.null_checked.retain(|id, _| !replaced.contains(id));
    facts.npe_message_needed.retain(|id| !replaced.contains(id));
}

/// The name of a class as the stubs' messages name the code by it: none unless the translation
/// is written (`Program::ends_rejected`).
fn named(name: &str, ends_rejected: bool) -> &str {
    if ends_rejected { name } else { "" }
}

/// A copy of a rejection.
fn copy(r: &Rejection) -> Rejection {
    Rejection {
        unit: r.unit,
        anchor: r.anchor,
        error: Error { pos: r.error.pos, message: r.error.message.clone() },
    }
}

/// The stubs that the census puts in place of code that an analysis or the generator rejected.
struct Stubs {
    /// The id of the next expression the stubs create.
    next_id: usize,
    /// The methods that are stubs already.
    stubbed: HashSet<usize>,
    /// The translation is written (`Program::ends_rejected`): a rejected field initializer becomes
    /// a stub that ends the program, rather than being left out.
    ends_rejected: bool,
    /// The method that the last rejection stubbed first, for `--timings`.
    owner: Option<usize>,
    /// The methods whose code the stubs changed since `changed` was last taken: those stubbed,
    /// the abstract ones made concrete, and those of a class whose field initializers a stub
    /// dropped, which its first constructor runs.
    changed: Vec<usize>,
    /// Whether a stub changed more than the body of a method since this was last cleared: it
    /// made an abstract method concrete, or changed a field's initializer.
    reshaped: bool,
    /// The expressions of the method bodies that the stubs replaced since the analyses last ran,
    /// which facts kept from that run still name (`forget_replaced`).
    replaced: HashSet<ExprId>,
}

impl Stubs {
    /// Stubs the methods that have code at the position of `error`, and drops the field
    /// initializers there; or else makes the abstract methods there concrete stubs. The rejection
    /// has the position of the first method stubbed as its anchor, and none when there was
    /// nothing to stub.
    fn stub(&mut self, class: &mut Class, units: &[Option<usize>], error: Error) -> Rejection {
        let pos = error.pos;
        // An error at a position with its unit is in that unit's code alone (`Pos::unit`), or in
        // the code that lowering provides alone (`PROVIDED_UNIT`).
        let known = pos.unit;
        let mut rejection = Rejection { unit: pos.source_unit(), anchor: None, error };
        self.owner = None;
        // The code that lowering provides has no position of its own.
        if pos == Pos::default() {
            return rejection;
        }
        let in_unit = |t: usize| {
            let unit = units.get(t).copied().flatten();
            known.is_none_or(|k| unit.map_or(PROVIDED_UNIT, |u| u as u32) == k)
        };
        let ends_rejected = self.ends_rejected;
        let at = |e: &Expr| {
            let mut here = false;
            visit_expr(e, &mut |x| here |= x.pos.same_place(pos));
            here
        };
        let mut abstracts = Vec::new();
        let types = &class.types;
        for (index, method) in class.methods.iter_mut().enumerate() {
            if !in_unit(method.class) {
                continue;
            }
            let here = std::cell::Cell::new(method.pos.same_place(pos));
            for s in &method.body {
                let mut statement = |s: &Stmt| {
                    if let Stmt::Local { pos: at, .. } | Stmt::Return(_, at) = s {
                        here.set(here.get() || *at == pos);
                    }
                };
                visit_stmt(s, &mut |e| here.set(here.get() || at(e)), &mut statement);
            }
            if !here.get() {
                continue;
            }
            // An error that no stub takes away is placed at the method it is in all the same.
            if rejection.unit.is_none() {
                rejection.unit = units.get(method.class).copied().flatten();
            }
            if method.is_abstract {
                abstracts.push(index);
            } else if self.stubbed.insert(index) {
                let message = stub_message(
                    &format!("{}.{}", named(&types[method.class].name, ends_rejected), method.name),
                    &rejection.error,
                );
                self.never(method, message);
                self.changed.push(index);
                if rejection.anchor.is_none() {
                    rejection.unit = units.get(method.class).copied().flatten();
                    rejection.anchor = Some(method.pos);
                    self.owner = Some(index);
                }
            }
        }
        let mut initialized = Vec::new();
        for (t, decl) in class.types.iter_mut().enumerate() {
            if !in_unit(t) {
                continue;
            }
            for field in decl.fields.iter_mut().chain(&mut decl.statics) {
                // An error at a field's declaration (its type) is placed in the field's unit.
                if field.pos == pos && rejection.unit.is_none() {
                    rejection.unit = units.get(t).copied().flatten();
                }
                // A constant's value is a literal that nothing rejects, though its position may
                // be that of an error in another file.
                if !field.constant && field.init.as_ref().is_some_and(|i| at(i) && !is_stub(i)) {
                    self.reshaped = true;
                    field.init = self.ends_rejected.then(|| {
                        let message = stub_message(named(&decl.name, ends_rejected), &rejection.error);
                        self.expr(Builtin::NoObject { class: t }, Some(message), field.pos, field.ty)
                    });
                    initialized.push(t);
                    if rejection.anchor.is_none() {
                        rejection.unit = units.get(t).copied().flatten();
                        rejection.anchor = Some(field.pos);
                    }
                }
            }
        }
        for (index, method) in class.methods.iter().enumerate() {
            if initialized.contains(&method.class) {
                self.changed.push(index);
            }
        }
        // An abstract method that an analysis rejects (one that an enum does not implement, say,
        // as its implementation was rejected) gets a body that never runs.
        if rejection.anchor.is_none() {
            for index in abstracts {
                if !self.stubbed.insert(index) {
                    continue;
                }
                let method = &mut class.methods[index];
                let message = stub_message(
                    &format!("{}.{}", named(&class.types[method.class].name, ends_rejected), method.name),
                    &rejection.error,
                );
                method.is_abstract = false;
                self.reshaped = true;
                self.never(method, message);
                self.changed.push(index);
                class.hierarchy.make_concrete(index);
                rejection.unit = units.get(method.class).copied().flatten();
                rejection.anchor = Some(method.pos);
                self.owner = Some(index);
            }
        }
        rejection
    }

    /// Makes the body of `method` one that never runs, or where the translation is written, one
    /// that ends the program with `message`.
    fn never(&mut self, method: &mut Method, message: String) {
        for s in &method.body {
            visit_stmt(s, &mut |e| visit_expr(e, &mut |x| _ = self.replaced.insert(x.id)), &mut |_| {});
        }
        let message = self.ends_rejected.then_some(message);
        let never = self.expr(Builtin::NoObject { class: method.class }, message, method.pos, Type::Void);
        method.body = vec![Stmt::Expr(never)];
    }

    /// A new expression of `builtin` with a string argument `message`, if there is one.
    fn expr(&mut self, builtin: Builtin, message: Option<String>, pos: Pos, ty: Type) -> Expr {
        let mut args = Vec::new();
        if let Some(message) = message {
            args.push(Expr { kind: ExprKind::Str(message), pos, ty: Type::String, id: self.next_id });
            self.next_id += 1;
        }
        let expr = Expr { kind: ExprKind::Builtin(builtin, args), pos, ty, id: self.next_id };
        self.next_id += 1;
        expr
    }
}

/// What a census's stub says where it runs: the code it stands for, named by `name` (its class,
/// or its method), and the rejection with its line.
fn stub_message(name: &str, error: &Error) -> String {
    format!("{name}: {} (line {})", error.message, error.pos.line)
}

/// Whether `e` is a census's stub (`Stubs::expr`).
fn is_stub(e: &Expr) -> bool {
    matches!(e.kind, ExprKind::Builtin(Builtin::NoObject { .. }, _))
}

/// The calls that lowering left without the method they call, which the analyses take to be
/// resolved: a fault of the translator, which would panic on them.
fn unresolved_calls(class: &mut Class) -> Result<(), Vec<Error>> {
    let mut errors = Vec::new();
    let mut see = |e: &Expr| {
        visit_expr(e, &mut |x| {
            if let ExprKind::Call { name, resolved: None, .. }
            | ExprKind::MethodCall { name, resolved: None, .. } = &x.kind
            {
                let message = format!("a call of `{name}` that lowering left unresolved");
                errors.push(Error { pos: x.pos, message });
            }
        })
    };
    for method in &class.methods {
        method.body.iter().for_each(|s| visit_stmt(s, &mut see, &mut |_| {}));
    }
    for decl in &class.types {
        decl.fields.iter().chain(&decl.statics).filter_map(|f| f.init.as_ref()).for_each(&mut see);
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// The errors that the analyses and the generator would find in the code alone, without their
/// facts, found before the analyses so that the code they reject is stubbed before the analyses
/// first run, rather than in a round of the analyses and the generator of its own: the checks of
/// the analyses that read the code alone (`analysis::code_checks`), and a class converted to an
/// interface of `java.lang.Class` that the runtime's classes do not implement, which the generator
/// rejects (`codegen::dispatch::class_cast`).
fn known_before_analyses(class: &mut Class) -> Result<(), Vec<Error>> {
    let mut errors = analysis::code_checks(class);
    let mut see = |e: &Expr| {
        visit_expr(e, &mut |x| {
            if let ExprKind::Cast(Type::Class(name, _), value) = &x.kind
                && value.ty == Type::JavaClass
                && crate::ast::class_only_interface(name)
            {
                let message = format!("a `java.lang.Class` as a `{name}` not supported yet");
                errors.push(Error { pos: value.pos, message });
            }
        })
    };
    for method in &class.methods {
        method.body.iter().for_each(|s| visit_stmt(s, &mut see, &mut |_| {}));
    }
    for decl in &class.types {
        decl.fields.iter().chain(&decl.statics).filter_map(|f| f.init.as_ref()).for_each(&mut see);
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// What the `UnsupportedOperationException` of a security manager's `getClassContext` says, which
/// the translator's patch of `SecurityManager` throws (`java/patches/java/lang/SecurityManager.java`).
const CLASS_CONTEXT_MESSAGE: &str = "SecurityManager.getClassContext: the classes on the call stack, which a translated program does not have";

/// The places where the program, as the census leaves it, throws an
/// `UnsupportedOperationException` whose message contains `marker`: code that translates, but
/// behaves otherwise than Java's, each with the message it throws, in the method it is in. A call
/// of a handle's method that lowering does not follow (`lower::handle_values`) throws one at the
/// call, with `lower::UNFOLLOWED_MESSAGE`; a security manager's `getClassContext` throws one with
/// `CLASS_CONTEXT_MESSAGE`; an `Unsafe` access at an offset that lowering does not resolve
/// (`lower::handles`) throws one with `lower::UNRESOLVED_OFFSET`, at the access or, where the
/// offset names no field, in the translator's access that it calls with that message.
fn thrown_unsupported(class: &Class, marker: &str) -> Vec<Rejection> {
    let Some(exception) = class.types.iter().position(|t| t.name == lower::UNFOLLOWED) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut see = |x: &Expr, anchor: Option<Pos>| {
        let message = match &x.kind {
            ExprKind::New { class, args, .. } if *class == exception => match args.as_slice() {
                [Expr { kind: ExprKind::Str(message), .. }] => Some(message),
                _ => None,
            },
            ExprKind::Call { args, .. } => args.iter().find_map(|a| match &a.kind {
                ExprKind::Str(message) => Some(message),
                _ => None,
            }),
            _ => None,
        };
        if let Some(message) = message.filter(|m| m.contains(marker)) {
            let unit = x.pos.source_unit();
            let error = Error { pos: x.pos, message: message.clone() };
            found.push(Rejection { unit, anchor, error });
        }
    };
    for method in &class.methods {
        for stmt in &method.body {
            visit_stmt_exprs(stmt, &mut |e| see(e, Some(method.pos)));
        }
    }
    for decl in &class.types {
        for init in decl.fields.iter().chain(&decl.statics).filter_map(|f| f.init.as_ref()) {
            visit_expr(init, &mut |e| see(e, None));
        }
    }
    found
}

/// The lookups of a charset by a name that the program computes (`lower::charset_lookups`), which
/// throw `UnsupportedOperationException` where the name is that of a charset of the JDK that the
/// translation has neither in the runtime nor as a table (`jdk_charsets::missing`), where Java
/// finds it: each at the call, in the method it is in.
fn computed_charset_names(class: &Class) -> Vec<Rejection> {
    let mut found = Vec::new();
    lower::charset_lookups(class, &mut |lookup| {
        if !matches!(lookup.name.kind, ExprKind::Str(_)) {
            let pos = lookup.call.pos;
            let message = format!("`{}` of a charset name known only at run time", lookup.what);
            let unit = pos.source_unit();
            found.push(Rejection { unit, anchor: lookup.method, error: Error { pos, message } });
        }
    });
    found
}

/// The dereferences whose `NullPointerException` message is approximated
/// (`Program::approximate_messages`), each in the method it is in.
fn approximated_messages(places: Vec<(Pos, Option<Pos>)>) -> Vec<Rejection> {
    places
        .into_iter()
        .map(|(pos, anchor)| {
            let message = "the message of the `NullPointerException` of this dereference".to_string();
            Rejection { unit: pos.source_unit(), anchor, error: Error { pos, message } }
        })
        .collect()
}

/// Gives the rejections that have a position but no unit (an error that lowering met outside the
/// code of a class, or one that no stub took away) the unit with code at that position, when one
/// unit alone has code there; not those in the code that lowering provides.
fn locate<'a>(trees: &[&Value], rejections: impl Iterator<Item = &'a mut Rejection>) {
    let mut rejections: Vec<&mut Rejection> =
        rejections.filter(|r| r.unit.is_none() && r.error.pos != Pos::default() && !provided(r)).collect();
    if rejections.is_empty() {
        return;
    }
    let wanted: HashSet<(usize, usize)> =
        rejections.iter().map(|r| (r.error.pos.line, r.error.pos.column)).collect();
    let mut found: BTreeMap<(usize, usize), BTreeSet<usize>> = BTreeMap::new();
    fn visit(
        node: &Value,
        unit: usize,
        wanted: &HashSet<(usize, usize)>,
        found: &mut BTreeMap<(usize, usize), BTreeSet<usize>>,
    ) {
        match node {
            Value::Object(fields) => {
                if let (Some(line), Some(column)) =
                    (fields.get("line").and_then(Value::as_u64), fields.get("column").and_then(Value::as_u64))
                {
                    let pos = (line as usize, column as usize);
                    if wanted.contains(&pos) {
                        found.entry(pos).or_default().insert(unit);
                    }
                }
                fields.values().for_each(|v| visit(v, unit, wanted, found));
            }
            Value::Array(items) => items.iter().for_each(|v| visit(v, unit, wanted, found)),
            _ => {}
        }
    }
    for (unit, tree) in trees.iter().enumerate() {
        visit(tree, unit, &wanted, &mut found);
    }
    for rejection in &mut rejections {
        if let Some(units) = found.get(&(rejection.error.pos.line, rejection.error.pos.column))
            && units.len() == 1
        {
            rejection.unit = units.first().copied();
        }
    }
}

/// Whether `rejection` is in the code that lowering provides (`PROVIDED_UNIT`).
fn provided(rejection: &Rejection) -> bool {
    rejection.error.pos.unit == Some(PROVIDED_UNIT)
}

/// The largest id of an expression of the program.
fn next_id(class: &Class) -> usize {
    let mut max = 0;
    let mut see = |e: &Expr| visit_expr(e, &mut |x| max = max.max(x.id));
    for method in &class.methods {
        method.body.iter().for_each(|s| visit_stmt(s, &mut see, &mut |_| {}));
    }
    for decl in &class.types {
        decl.fields.iter().chain(&decl.statics).filter_map(|f| f.init.as_ref()).for_each(&mut see);
        decl.enum_constants.iter().flatten().flat_map(|c| &c.values).for_each(&mut see);
    }
    class.raised.iter().for_each(&mut see);
    max
}

/// The rejections attributed to the pieces of code.
struct Tally {
    /// For each piece of the scan, the errors in it: message and place.
    errors: Vec<BTreeSet<(String, String)>>,
    /// The errors in no piece: in the code that lowering provides, or outside every piece.
    elsewhere: BTreeSet<(String, String)>,
    /// The error that ended the census early, with its place.
    fatal: Option<(String, String)>,
    /// Code that translates but throws `UnsupportedOperationException` where Java goes on, each
    /// of a `Listed` kind: the piece each is in, if any, its message and its place.
    listed: [BTreeSet<(Option<usize>, String, String)>; 5],
}

/// The kinds of code that translate but throw `UnsupportedOperationException` where Java goes
/// on, which the census lists apart from the rejections: the index of each in `Tally::listed`.
#[derive(Clone, Copy)]
enum Listed {
    /// The calls of a handle's method that lowering does not follow (`thrown_unsupported`).
    Handles = 0,
    /// The lookups of a charset by a name known only at run time (`computed_charset_names`).
    CharsetNames = 1,
    /// The `NullPointerException` messages that are approximated (`approximated_messages`).
    Messages = 2,
    /// A security manager's `getClassContext` (`thrown_unsupported`).
    ClassContexts = 3,
    /// The accesses of `Unsafe` at an offset that lowering does not resolve (`thrown_unsupported`).
    UnsafeOffsets = 4,
}

impl Listed {
    const ALL: [Listed; 5] = [
        Listed::Handles,
        Listed::CharsetNames,
        Listed::Messages,
        Listed::ClassContexts,
        Listed::UnsafeOffsets,
    ];

    /// What the report says of the pieces with code of this kind.
    fn heading(self) -> &'static str {
        match self {
            Listed::Handles => {
                "pieces that translate into code that throws UnsupportedOperationException for a handle \
                 call that the translation does not follow, where Java runs it"
            }
            Listed::CharsetNames => {
                "pieces that translate into code that throws UnsupportedOperationException where a \
                 charset name known only at run time names a charset of the JDK that the translation \
                 does not have, where Java finds it"
            }
            Listed::Messages => {
                "pieces that translate into code whose NullPointerException, which a catch may read, has \
                 an approximated message (--approximate-messages): the JVM's words for what failed \
                 without where the null came from, or none"
            }
            Listed::ClassContexts => {
                "pieces that translate into code that throws UnsupportedOperationException for the \
                 classes on the call stack, which a translated program does not have, where Java gives them"
            }
            Listed::UnsafeOffsets => {
                "pieces that translate into code that throws UnsupportedOperationException for an Unsafe \
                 access at an offset that the translation does not resolve, unless it is the number of a \
                 field of the reflection table, where Java reads or writes the memory there"
            }
        }
    }

    /// How the report starts each of its rows.
    fn row(self) -> &'static str {
        match self {
            Listed::Handles | Listed::ClassContexts => "throws UnsupportedOperationException:",
            Listed::CharsetNames | Listed::UnsafeOffsets => "may throw UnsupportedOperationException:",
            Listed::Messages => "approximates",
        }
    }
}

impl Tally {
    fn new(
        pieces: &[Piece],
        files: &[String],
        rejections: Vec<Rejection>,
        fatal: Option<Rejection>,
        listed: [Vec<Rejection>; 5],
    ) -> Self {
        let mut tally = Tally {
            errors: vec![BTreeSet::new(); pieces.len()],
            elsewhere: BTreeSet::new(),
            fatal: fatal.map(|f| (f.error.message.clone(), place(files, &f))),
            listed: std::array::from_fn(|_| BTreeSet::new()),
        };
        for rejection in rejections {
            let entry = (rejection.error.message.clone(), place(files, &rejection));
            match attribute(pieces, &rejection) {
                Some(piece) => tally.errors[piece].insert(entry),
                None => tally.elsewhere.insert(entry),
            };
        }
        for (set, calls) in tally.listed.iter_mut().zip(listed) {
            for call in calls {
                let piece = attribute(pieces, &call);
                set.insert((piece, call.error.message.clone(), place(files, &call)));
            }
        }
        tally
    }

    /// The code of kind `kind` (`listed`) in the pieces of one part that translate, or with no
    /// part in no piece: each message, the number of pieces, and the places of all of its calls,
    /// the messages of the most pieces first.
    fn listed(&self, kind: Listed, pieces: &[Piece], part: Option<Part>) -> Vec<(&str, usize, Vec<&str>)> {
        let mut rows: BTreeMap<&str, (BTreeSet<usize>, Vec<&str>)> = BTreeMap::new();
        for (piece, message, place) in &self.listed[kind as usize] {
            let here = match piece {
                Some(p) => Some(pieces[*p].part) == part && self.errors[*p].is_empty(),
                None => part.is_none(),
            };
            if here {
                let row = rows.entry(message).or_default();
                row.0.extend(*piece);
                row.1.push(place);
            }
        }
        // By file, line and column.
        let key = |place: &&str| {
            let mut parts = place.rsplitn(3, ':');
            let (column, line) =
                (parts.next().and_then(|c| c.parse().ok()), parts.next().and_then(|l| l.parse().ok()));
            (parts.next().unwrap_or_default().to_string(), line.unwrap_or(0usize), column.unwrap_or(0usize))
        };
        let mut rows: Vec<_> = rows
            .into_iter()
            .map(|(message, (pieces, mut places))| {
                places.sort_by_cached_key(key);
                (message, pieces.len(), places)
            })
            .collect();
        rows.sort_by_key(|&(message, n, _)| (Reverse(n), message));
        rows
    }

    /// The pieces of one part: how many there are, and how many translate.
    fn counts(&self, pieces: &[Piece], part: Part) -> (usize, usize) {
        let mine = || pieces.iter().zip(&self.errors).filter(|(p, _)| p.part == part);
        (mine().count(), mine().filter(|(_, e)| e.is_empty()).count())
    }

    /// The rejections of one part's pieces, by normalized message: the pieces each blocks, its
    /// messages with their numbers of pieces, and up to `EXAMPLES` places.
    fn messages<'a>(&'a self, pieces: &[Piece], part: Part) -> Vec<(String, MessageRow<'a>)> {
        let mut rows: BTreeMap<String, MessageRow> = BTreeMap::new();
        for (piece, errors) in pieces.iter().zip(&self.errors).filter(|(p, _)| p.part == part) {
            // A piece counts once for each message it has, normalized or not.
            let (mut seen, mut seen_exact) = (HashSet::new(), HashSet::new());
            for (message, place) in errors {
                let normalized = normalize(message);
                let row = rows.entry(normalized.clone()).or_default();
                if seen.insert(normalized) {
                    row.pieces += 1;
                }
                if seen_exact.insert(message) {
                    *row.exact.entry(message).or_default() += 1;
                }
                if row.examples.len() < EXAMPLES && !row.examples.contains(&place.as_str()) {
                    row.examples.push(place);
                }
                row.classes.insert(piece.class.clone());
            }
        }
        let mut rows: Vec<_> = rows.into_iter().collect();
        rows.sort_by(|a, b| (Reverse(a.1.pieces), &a.0).cmp(&(Reverse(b.1.pieces), &b.0)));
        rows
    }

    /// The pieces of one part that translate into code that throws `NoClassDefFoundError` where
    /// Java would, for a class that is missing (`Piece::missing_class`): each class, the number of
    /// pieces, and up to `EXAMPLES` of their places, the classes of the most pieces first.
    fn missing_classes<'a>(&self, pieces: &'a [Piece], part: Part) -> Vec<(&'a str, usize, Vec<&'a str>)> {
        let mut rows: BTreeMap<&str, (usize, Vec<&str>)> = BTreeMap::new();
        for (piece, _) in pieces.iter().zip(&self.errors).filter(|(p, e)| p.part == part && e.is_empty()) {
            if let Some(class) = &piece.missing_class {
                let row = rows.entry(class).or_default();
                row.0 += 1;
                if row.1.len() < EXAMPLES {
                    row.1.push(&piece.location);
                }
            }
        }
        let mut rows: Vec<_> = rows.into_iter().map(|(class, (n, examples))| (class, n, examples)).collect();
        rows.sort_by_key(|&(class, n, _)| (Reverse(n), class));
        rows
    }

    fn report(&self, scan: &survey::Census, main: &str) -> String {
        let pieces = &scan.pieces;
        let mut out = String::new();
        _ = writeln!(
            out,
            "Census of the code reachable from {main}: each piece of code (method, constructor, field \
             initializer, initializer block) lowered, analysed and generated, past every rejection."
        );
        if let Some((message, place)) = &self.fatal {
            _ = writeln!(out, "Incomplete: the census stopped at {place}: {message}");
        }
        let mut together = (0, 0);
        for part in Part::ALL {
            let (all, translate) = self.counts(pieces, part);
            together = (together.0 + all, together.1 + translate);
            _ = writeln!(
                out,
                "  {}: {all} pieces; {translate} ({}) translate, {} rejected.",
                part.title(),
                percent(translate, all),
                all - translate
            );
        }
        let (all, translate) = together;
        _ = writeln!(out, "  Together: {all} pieces; {translate} ({}) translate.", percent(translate, all));
        for part in Part::ALL {
            _ = writeln!(out, "\n{} code: rejections by pieces of code they block", part.title());
            for (message, row) in self.messages(pieces, part) {
                _ = writeln!(out, "  {:>6}  {message}", row.pieces);
                if row.exact.len() > 1 {
                    let mut exact: Vec<_> = row.exact.iter().collect();
                    exact.sort_by_key(|(m, n)| (Reverse(**n), **m));
                    for (m, n) in exact.into_iter().take(EXAMPLES) {
                        _ = writeln!(out, "          {n:>6}  {m}");
                    }
                }
                _ = writeln!(out, "          at {}", row.examples.join(", "));
            }
            let missing = self.missing_classes(pieces, part);
            if !missing.is_empty() {
                _ = writeln!(
                    out,
                    "\n{} code: pieces that translate, where a class is missing, into code that throws",
                    part.title()
                );
                for (class, n, examples) in missing {
                    _ = writeln!(
                        out,
                        "  throws NoClassDefFoundError like Java: missing class {class}, {n} piece{}",
                        if n == 1 { "" } else { "s" }
                    );
                    _ = writeln!(out, "          at {}", examples.join(", "));
                }
            }
            for kind in Listed::ALL {
                self.report_listed(&mut out, kind, pieces, Some(part));
            }
        }
        for kind in Listed::ALL {
            self.report_listed(&mut out, kind, pieces, None);
        }
        if !self.elsewhere.is_empty() {
            _ = writeln!(out, "\nRejected outside the pieces of code:");
            for (message, place) in &self.elsewhere {
                _ = writeln!(out, "  {place}: {message}");
            }
        }
        let missing: Vec<_> = scan.natives.iter().filter(|n| !n.3).collect();
        _ = writeln!(
            out,
            "\nNative methods reached: {} ({} in the runtime); missing from the runtime:",
            scan.natives.len(),
            scan.natives.len() - missing.len()
        );
        let mut missing: Vec<_> = missing.into_iter().map(|n| (n.1.as_str(), n.2.as_str())).collect();
        missing.sort();
        for (native, node) in missing {
            _ = writeln!(out, "  {native}");
            let chain = scan.chain(node);
            if !chain.is_empty() {
                _ = writeln!(out, "      via {}", chain.join(" -> "));
            }
        }
        out
    }

    /// The code of kind `kind` in the pieces of `part` (in no piece: `None`), for the report.
    fn report_listed(&self, out: &mut String, kind: Listed, pieces: &[Piece], part: Option<Part>) {
        let rows = self.listed(kind, pieces, part);
        if rows.is_empty() {
            return;
        }
        let whose = part.map_or("Code in no piece".to_string(), |part| format!("{} code", part.title()));
        _ = writeln!(out, "\n{whose}: {}", kind.heading());
        for (message, n, places) in rows {
            _ = writeln!(out, "  {} {message}, {n} piece{}", kind.row(), if n == 1 { "" } else { "s" });
            _ = writeln!(out, "          at {}", places.join(", "));
        }
    }

    fn json(&self, scan: &survey::Census, main: &str) -> Value {
        let pieces = &scan.pieces;
        let totals: crate::value::Map = Part::ALL
            .iter()
            .map(|&part| {
                let (all, translate) = self.counts(pieces, part);
                (
                    part.name().to_string(),
                    json!({"pieces": all, "translate": translate, "rejected": all - translate}),
                )
            })
            .collect();
        let rejections: Vec<Value> = Part::ALL
            .iter()
            .flat_map(|&part| {
                self.messages(pieces, part).into_iter().map(move |(message, row)| {
                    json!({
                        "source": part.name(),
                        "message": message,
                        "pieces": row.pieces,
                        "classes": row.classes.len(),
                        "messages": row.exact,
                        "examples": row.examples,
                    })
                })
            })
            .collect();
        let missing_classes: Vec<Value> = Part::ALL
            .iter()
            .flat_map(|&part| {
                self.missing_classes(pieces, part).into_iter().map(move |(class, n, examples)| {
                    json!({"source": part.name(), "class": class, "pieces": n, "examples": examples})
                })
            })
            .collect();
        let listed = |kind: Listed| -> Vec<Value> {
            Part::ALL
                .iter()
                .map(|&part| Some(part))
                .chain([None])
                .flat_map(|part| {
                    self.listed(kind, pieces, part).into_iter().map(move |(message, n, places)| {
                        json!({
                            "source": part.map(Part::name),
                            "message": message,
                            "pieces": n,
                            "places": places,
                        })
                    })
                })
                .collect()
        };
        let by_piece: Vec<BTreeMap<usize, Vec<Value>>> = Listed::ALL
            .iter()
            .map(|&kind| {
                let mut by_piece: BTreeMap<usize, Vec<Value>> = BTreeMap::new();
                for (piece, message, place) in &self.listed[kind as usize] {
                    if let Some(piece) = piece {
                        by_piece.entry(*piece).or_default().push(json!({"message": message, "place": place}));
                    }
                }
                by_piece
            })
            .collect();
        let of_piece = |kind: Listed, i: usize| by_piece[kind as usize].get(&i).cloned().unwrap_or_default();
        json!({
            "main": main,
            "complete": self.fatal.is_none(),
            "fatal": self.fatal.as_ref().map(|(message, place)| json!({"message": message, "place": place})),
            "totals": totals,
            "rejections": rejections,
            "missingClasses": missing_classes,
            "unfollowedHandles": listed(Listed::Handles),
            "charsetNames": listed(Listed::CharsetNames),
            "approximatedMessages": listed(Listed::Messages),
            "classContexts": listed(Listed::ClassContexts),
            "unresolvedOffsets": listed(Listed::UnsafeOffsets),
            "pieces": pieces.iter().zip(&self.errors).enumerate().map(|(i, (p, errors))| json!({
                "source": p.part.name(),
                "class": p.class,
                "kind": p.kind,
                "name": p.name,
                "location": p.location,
                "missingClass": p.missing_class,
                "errors": errors.iter().map(|(message, place)| json!({"message": message, "place": place}))
                    .collect::<Vec<_>>(),
                "unfollowedHandles": of_piece(Listed::Handles, i),
                "charsetNames": of_piece(Listed::CharsetNames, i),
                "approximatedMessages": of_piece(Listed::Messages, i),
                "classContexts": of_piece(Listed::ClassContexts, i),
                "unresolvedOffsets": of_piece(Listed::UnsafeOffsets, i),
            })).collect::<Vec<_>>(),
            "elsewhere": self.elsewhere.iter().map(|(message, place)| json!({"message": message, "place": place}))
                .collect::<Vec<_>>(),
            "natives": scan.natives.iter().map(|(part, method, node, runtime)| json!({
                "source": part.name(),
                "method": method,
                "runtime": runtime,
                "chain": scan.chain(node),
            })).collect::<Vec<_>>(),
        })
    }
}

/// One normalized message's row in a part's table.
#[derive(Default)]
struct MessageRow<'a> {
    pieces: usize,
    classes: BTreeSet<String>,
    /// The messages that normalize to it, with the number of pieces each blocks.
    exact: BTreeMap<&'a str, usize>,
    examples: Vec<&'a str>,
}

/// The piece of code that a rejection belongs to: that of the method or lambda in which it was
/// met (its anchor), unless the error is in an initializer of the same class that a constructor
/// runs; without an anchor, the piece at the error's position.
fn attribute(pieces: &[Piece], rejection: &Rejection) -> Option<usize> {
    let unit = rejection.unit?;
    let key = |p: Pos| (p.line, p.column);
    let at = |pos: Pos| {
        pieces.iter().position(|p| p.unit == unit && key(p.span.0) <= key(pos) && key(pos) <= key(p.span.1))
    };
    let here = at(rejection.error.pos);
    match (here, rejection.anchor.and_then(at)) {
        (Some(here), Some(anchor))
            if here != anchor
                && !(pieces[here].class == pieces[anchor].class
                    && matches!(pieces[here].kind, "field initializer" | "instance block")) =>
        {
            Some(anchor)
        }
        (here, anchor) => here.or(anchor),
    }
}

/// `file:line:column` of a rejection, the file relative to its root; `line:column` in the code
/// that lowering provides.
fn place(files: &[String], rejection: &Rejection) -> String {
    let pos = rejection.error.pos;
    match rejection.unit {
        Some(unit) => format!("{}:{}:{}", files[unit], pos.line, pos.column),
        None => format!("{}:{}", pos.line, pos.column),
    }
}

/// A message with the names of classes, methods and types in backquotes left out, so that the
/// rejections of one construct are counted together: "type `java.io.File` not supported yet" is
/// "type `…` not supported yet". Operators and simple names (`==`, `Object`) stay.
pub fn normalize(message: &str) -> String {
    let mut out = String::new();
    for (i, part) in message.split('`').enumerate() {
        if i % 2 == 1 {
            let name = part.contains(['.', '<', '[', '(']);
            out.push('`');
            out.push_str(if name { "…" } else { part });
            out.push('`');
        } else {
            out.push_str(part);
        }
    }
    out
}

fn percent(part: usize, whole: usize) -> String {
    format!("{:.1}%", 100.0 * part as f64 / whole.max(1) as f64)
}

#[cfg(test)]
mod tests {
    /// The census finds the throw of `getClassContext` by its message, which the patch writes.
    #[test]
    fn the_patch_of_security_manager_throws_the_listed_message() {
        let patch = include_str!("../../java/patches/java/lang/SecurityManager.java");
        assert!(patch.contains(super::CLASS_CONTEXT_MESSAGE));
    }
}
