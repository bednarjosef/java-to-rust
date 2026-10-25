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
//! the generator run again. The generator and the analysis of exceptions report every error they
//! find, which are stubbed together; the other analyses report their first. A panic while the
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

use super::survey::{self, Part, Piece};
use super::{Program, ProgramError, Reach, Reaching, Writes, class_file_names, load, source_files};
use crate::ast::{Builtin, Class, Error, Expr, ExprKind, Method, Pos, Stmt, Type, visit_expr, visit_stmt};
use crate::output::ModuleTree;
use crate::value::{Value, json};
use crate::{analysis, codegen, javac, lower};
use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write;

/// Up to this many example places per message.
const EXAMPLES: usize = 3;

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
struct Rejection {
    unit: Option<usize>,
    anchor: Option<Pos>,
    error: Error,
}

/// The census of the code reachable from the program's `main`, with or without the writes of
/// `String` arrays as `Object` arrays (`Reach::string_array_writes`), as a translation decides
/// (`super::translate_program_modules`).
pub fn census_program(program: &Program) -> Result<Census, ProgramError> {
    let (units, reach, asked) = load(program, Reaching::Walk(false))?;
    // When a walk with the writes would reach the same, the census is first taken with them.
    let same = asked.is_some();
    let mut writes = if same { Writes::First } else { Writes::Without };
    let mut loaded = (units, reach);
    loop {
        match census_reached(program, loaded, writes, same)? {
            Taken::Census(census) => return Ok(census),
            Taken::Again(Some(again), next) => (loaded, writes) = (*again, next),
            Taken::Again(None, next) => {
                let (units, reach, _) = load(program, Reaching::Walk(true))?;
                (loaded, writes) = ((units, reach), next);
            }
        }
    }
}

/// What `census_reached` gives: the census, or the census to take instead, over these units and
/// reach or, with none, over a load with the writes of `String` arrays.
enum Taken {
    Census(Census),
    Again(Option<Box<(Vec<Value>, Reach)>>, Writes),
}

/// `census_program` over these units and reach (`super::load`), with the writes of `String`
/// arrays as `writes` says; `same`: a walk with them reaches the same as without.
fn census_reached(
    program: &Program,
    loaded: (Vec<Value>, Reach),
    writes: Writes,
    same: bool,
) -> Result<Taken, ProgramError> {
    use crate::timings::phase;
    let files = source_files(program)?;
    let (units, mut reach) = loaded;
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
    let Narrowed { class, mut rejections, mut fatal, again, units: lowered_units, .. } = narrowed;
    if let Some(next) = again {
        std::panic::set_hook(hook);
        drop((class, trees));
        let again = same.then(|| Box::new((units, reach)));
        if again.is_some() && crate::timings::enabled() {
            eprintln!(
                "timing: javac and reach: the units and reach of the census without the writes of `String[]`s"
            );
        }
        return Ok(Taken::Again(again, next));
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
    if let Some(class) = &class {
        super::print_notes(class, &unit_files);
    }
    drop(trees);
    drop((units, reach));
    let mut modules = None;
    if let Some(mut class) = class {
        fatal = phase("analyze", || {
            let names = Names { program, jdk_temps: RefCell::new(jdk_temps), found: RefCell::new(None) };
            analyse(
                &mut class,
                &lowered_units,
                &unit_files,
                &mut rejections,
                &mut modules,
                &names,
                program.ends_rejected,
            )
        });
    }
    std::panic::set_hook(hook);
    let tally = Tally::new(&scan.pieces, &unit_files, rejections, fatal);
    let (report, json) = (tally.report(&scan, &program.main), (&tally.json(&scan, &program.main)).into());
    Ok(Taken::Census(Census { report, json, modules }))
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
    let (class, fatal) = match lowered.class {
        Err((unit, error)) => {
            rejections.extend(lowered.rejected.into_iter().map(rejection));
            (None, Some(Rejection { unit, anchor: None, error }))
        }
        // As a translation does, only what can run counts (the unresolved calls are found again
        // below, where they are stubbed).
        Ok(mut class) => {
            super::stamp_units(&mut class, &lowered.units);
            // The census is taken again with the writes of `String` arrays (`census_program`)
            // once one of them can run: looking further, or analysing this program, would
            // change nothing.
            let stops = |r: &lower::Rejected| rejects_writes(&r.error.message);
            let watched = &lowered.string_array_writes;
            let narrow = || super::narrow(&mut class, lowered.rejected, lowered.live, &stops, watched);
            match phase("live methods", narrow) {
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
    Narrowed { class, rejections, fatal, units: lowered.units, again, wrote }
}

/// How the class files name what the messages of a `NullPointerException` show, found the first
/// time a `catch` of the program reads one, as a translation finds them.
struct Names<'a> {
    program: &'a Program,
    /// The temps of the JDK's code (`javac::jdk_temps`), until they are added to `found`.
    jdk_temps: RefCell<javac::JdkTemps>,
    found: RefCell<Option<javac::BytecodeNames>>,
}

impl Names<'_> {
    /// The messages of `facts`'s dereferences (`analysis::npe_messages`), where a `catch` reads
    /// one; each that is not certain is an error.
    fn messages(&self, class: &Class, facts: &mut analysis::Facts) -> Result<(), Vec<Error>> {
        if facts.npe_message_read.is_none() {
            return Ok(());
        }
        let mut found = self.found.borrow_mut();
        if found.is_none() {
            let names = crate::timings::phase("census: class files", || class_file_names(self.program));
            let mut names = names.map_err(|e| vec![e.error])?;
            names.add_jdk_temps(self.jdk_temps.take());
            if crate::timings::enabled() {
                eprintln!("timing: census: class files: {}", names.sizes());
            }
            *found = Some(names);
        }
        let names = found.as_ref().expect("found above");
        crate::timings::phase("census: messages", || analysis::npe_messages(class, facts, names))
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
    let mut stubs = Stubs { next_id: next_id(class) + 1, stubbed: HashSet::new(), ends_rejected };
    // `whole_objects` changes the program only when it passes: an error of it that no stub takes
    // away is recorded, and the census goes on without it. An error of a check of the analyses
    // that only rejects (as of a field's type) that no stub takes away is recorded too, and the
    // analyses go on past it (`analysis::analyze_tolerating`). Any other error of the analyses or
    // the generator that no stub takes away ends it. The generator reports every error it finds,
    // which are stubbed together.
    let tolerated: std::cell::RefCell<Vec<Error>> = Default::default();
    // The facts that the analyses find before their first check, kept while no stub changes the
    // program: a pass that only tolerates more errors finds them again the same.
    let represented: std::cell::RefCell<Option<analysis::Facts>> = Default::default();
    let passes: [Pass; 4] = [
        &|class| crate::timings::phase("census: string identity", || analysis::string_identity(class)),
        &|class| crate::timings::phase("census: whole objects", || analysis::whole_objects_all(class)),
        &unresolved_calls,
        &|class| {
            let tolerated = tolerated.borrow();
            let mut facts = crate::timings::phase("census: analyses", || {
                let found =
                    represented.borrow_mut().get_or_insert_with(|| analysis::represented(class)).clone();
                analysis::analyze_from(class, &tolerated, found)
            })?;
            // Each message that is not certain stubs its code, as a translation rejects it.
            names.messages(class, &mut facts)?;
            codegen::catch_panics(true);
            let tree = crate::timings::phase("census: generator", || codegen::generate_all(class, &facts));
            codegen::catch_panics(false);
            tree.map(|tree| *generated.borrow_mut() = Some(tree))
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
            if crate::timings::enabled() {
                let first = errors.first().map_or("", |e| e.message.as_str());
                eprintln!(
                    "timing: census: pass {index} rejects {} pieces of code, first: {first}",
                    errors.len()
                );
            }
            let mut stuck = Vec::new();
            let mut stubbed = false;
            for error in errors {
                let rejection = stubs.stub(class, units, error);
                if crate::timings::enabled() {
                    let owner = rejection.anchor.and_then(|at| {
                        let in_unit = |m: &Method| units.get(m.class).copied().flatten() == rejection.unit;
                        class.methods.iter().find(|m| m.pos == at && in_unit(m))
                    });
                    let owner = owner.map_or("nothing".to_string(), |m| {
                        format!("{}.{}", class.types[m.class].name, m.name)
                    });
                    eprintln!(
                        "timing: census: stubbed {owner} at {}: {}",
                        place(files, &rejection),
                        rejection.error.message
                    );
                }
                if rejection.anchor.is_some() {
                    stubbed = true;
                    rejections.push(rejection);
                } else {
                    stuck.push(rejection);
                }
            }
            // An error that no stub took away comes back, unless the stubs took away its cause.
            if stubbed {
                represented.take();
                continue;
            }
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
}

impl Stubs {
    /// Stubs the methods that have code at the position of `error`, and drops the field
    /// initializers there; or else makes the abstract methods there concrete stubs. The rejection
    /// has the position of the first method stubbed as its anchor, and none when there was
    /// nothing to stub.
    fn stub(&mut self, class: &mut Class, units: &[Option<usize>], error: Error) -> Rejection {
        let pos = error.pos;
        // An error at a position with its unit is in that unit's code alone (`Pos::unit`).
        let known = pos.unit.map(|u| u as usize);
        let mut rejection = Rejection { unit: known, anchor: None, error };
        // The code that lowering provides has no position of its own.
        if pos == Pos::default() {
            return rejection;
        }
        let in_unit = |t: usize| known.is_none() || units.get(t).copied().flatten() == known;
        // The names of the classes, which the stubs' messages name the code by.
        let names: Vec<String> = match self.ends_rejected {
            true => class.types.iter().map(|d| d.name.clone()).collect(),
            false => vec![String::new(); class.types.len()],
        };
        let at = |e: &Expr| {
            let mut here = false;
            visit_expr(e, &mut |x| here |= x.pos.same_place(pos));
            here
        };
        let mut abstracts = Vec::new();
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
                let message =
                    stub_message(&format!("{}.{}", names[method.class], method.name), &rejection.error);
                self.never(method, message);
                if rejection.anchor.is_none() {
                    rejection.unit = units.get(method.class).copied().flatten();
                    rejection.anchor = Some(method.pos);
                }
            }
        }
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
                    field.init = self.ends_rejected.then(|| {
                        let message = stub_message(&names[t], &rejection.error);
                        self.expr(Builtin::NoObject { class: t }, Some(message), field.pos, field.ty)
                    });
                    if rejection.anchor.is_none() {
                        rejection.unit = units.get(t).copied().flatten();
                        rejection.anchor = Some(field.pos);
                    }
                }
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
                let message =
                    stub_message(&format!("{}.{}", names[method.class], method.name), &rejection.error);
                method.is_abstract = false;
                self.never(method, message);
                class.hierarchy.make_concrete(index);
                rejection.unit = units.get(method.class).copied().flatten();
                rejection.anchor = Some(method.pos);
            }
        }
        rejection
    }

    /// Makes the body of `method` one that never runs, or where the translation is written, one
    /// that ends the program with `message`.
    fn never(&mut self, method: &mut Method, message: String) {
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

/// Gives the rejections that have a position but no unit (an error that lowering met outside the
/// code of a class, or one that no stub took away) the unit with code at that position, when one
/// unit alone has code there.
fn locate<'a>(trees: &[&Value], rejections: impl Iterator<Item = &'a mut Rejection>) {
    let mut rejections: Vec<&mut Rejection> =
        rejections.filter(|r| r.unit.is_none() && r.error.pos != Pos::default()).collect();
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
}

impl Tally {
    fn new(pieces: &[Piece], files: &[String], rejections: Vec<Rejection>, fatal: Option<Rejection>) -> Self {
        let mut tally = Tally {
            errors: vec![BTreeSet::new(); pieces.len()],
            elsewhere: BTreeSet::new(),
            fatal: fatal.map(|f| (f.error.message.clone(), place(files, &f))),
        };
        for rejection in rejections {
            let entry = (rejection.error.message.clone(), place(files, &rejection));
            match attribute(pieces, &rejection) {
                Some(piece) => tally.errors[piece].insert(entry),
                None => tally.elsewhere.insert(entry),
            };
        }
        tally
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
        json!({
            "main": main,
            "complete": self.fatal.is_none(),
            "fatal": self.fatal.as_ref().map(|(message, place)| json!({"message": message, "place": place})),
            "totals": totals,
            "rejections": rejections,
            "missingClasses": missing_classes,
            "pieces": pieces.iter().zip(&self.errors).map(|(p, errors)| json!({
                "source": p.part.name(),
                "class": p.class,
                "kind": p.kind,
                "name": p.name,
                "location": p.location,
                "missingClass": p.missing_class,
                "errors": errors.iter().map(|(message, place)| json!({"message": message, "place": place}))
                    .collect::<Vec<_>>(),
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
