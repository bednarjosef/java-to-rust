//! Generates Rust source from the typed syntax tree. Every construct is translated so that it
//! behaves exactly like Java: `int` arithmetic wraps, division by zero throws, doubles print
//! like Java. Where the analyses prove a plainer form equivalent (an addition that cannot
//! overflow, a divisor that cannot be zero, a counting `for` loop, a string parameter that is
//! only read) that form is used instead.

use crate::analysis::Facts;
use crate::analysis::hierarchy::{self, Home};
use crate::ast::*;
use crate::output::ModuleTree;
use std::collections::{HashMap, HashSet};

mod boxing;
mod ceil_div;
mod channels;
mod char_sequence;
mod character;
mod charsets;
mod class_values;
mod classes;
mod closures;
mod code;
mod code_point;
mod covariant;
mod deflate;
mod dispatch;
mod enums;
mod exceptions;
mod expr;
mod field_inits;
mod field_updates;
mod file_channels;
mod file_copy;
mod files;
mod idiomatic;
mod imports;
mod inflate;
mod math;
mod names;
mod null;
mod nullable_enums;
mod object_clone;
mod parse_range;
mod primitives;
mod statics;
mod stmt;
mod strings;
mod switch;
mod tail;
mod text;
mod unsigned;

use code::*;
use dispatch::{Need, Needs};
use expr::Operand;
use idiomatic::{bool_test, literal_shift};
use names::*;
use null::is_null_test;
use primitives::*;
use switch::byte_string;
use switch::{Frame, Kind};

mod array_writes;
mod arraycopy;
mod arraycopy_classes;
mod arrays;
mod borrow_args;
mod java_arrays;
mod java_objects;
mod main_args;
mod modules;
mod natives;
mod objects;
mod owned;
mod patterns;
mod plain_arrays;
mod range_loops;
mod shared_strings;
mod sliced_loops;
mod stack_objects;
mod sync;
mod threads;
pub use threads::on_threads;

use arrays::name_part;
use boxing::boxed_locals;
use objects::*;

/// The program as the text of one file (`output::Layout::OneFile`).
pub fn generate(class: &Class, facts: &Facts) -> Result<String, Error> {
    generate_modules(class, facts).map(|tree| tree.one_file())
}

/// The program as modules, one per package, for a layout to write.
pub fn generate_modules(class: &Class, facts: &Facts) -> Result<ModuleTree, Error> {
    generate_all(class, facts).map_err(|mut errors| errors.remove(0))
}

/// The program, or what in it cannot be translated: every construct found, in the order the
/// program is written, or else every method with a call that throws inside a closure. A census
/// (`program/census.rs`) stubs them all at once.
pub fn generate_all(class: &Class, facts: &Facts) -> Result<ModuleTree, Vec<Error>> {
    SHARED_STRINGS.set(class.shared_strings);
    // The enums of class types get the items that the code uses, which writing the code finds:
    // the program is written again until it asks for no item that was not written. Only the
    // enum items read which items are wanted, so what the rest asks for, and its text, are the
    // same in every round: after the first, the rounds write only the enum items until those
    // ask for nothing new, and then all of them once more among the first round's other items
    // (`Kept`). What an enum item asks for depends on the item alone, so a round of enum items
    // writes those asked for anew (`Gen::only_enums`): the others ask for what they asked for
    // before. What does not depend on the wanted items is worked out once (`Setup`).
    let setup = Setup::new(class, facts);
    rounds(class, facts, &setup, Needs::new(), None, None, 1)
}

/// A census's hook of `generate_all_stubbing`.
pub type Stub<'a> = dyn FnMut(&mut Class, &mut Facts, Vec<Error>) -> Option<HashSet<usize>> + 'a;

/// `generate_all` for a census, which stubs what the first round rejects and has the generator
/// go on with the stubs in the same run. `stub` gets the errors of the first round, after the
/// first round's text of every module is written and checked, and stubs their code in `class`:
/// it gives back the methods whose bodies it replaced, or `None` where it changed more than
/// bodies, or did not take every error away, and the census goes on as with `generate_all`'s
/// errors (which it then has already). The modules that the stubs reach are written again
/// (`restubbed`); every other module keeps its first round's text and what writing it found
/// (`Found`), as a module's text depends on its own code alone, given what the generator works
/// out of the whole program before it writes (`Setup`) and the messages of the dereferences.
/// Where the stubs change those, other than which classes are live, the program is written again
/// from the start.
/// The errors given back are those of the program with the stubs, or the first round's when
/// `stub` gave back `None`. `stub` may change `facts` to those of the program with the stubs: take
/// out what they say of the code it replaced, and find the messages of the dereferences again.
pub fn generate_all_stubbing(
    class: &mut Class,
    facts: &mut Facts,
    stub: &mut Stub,
) -> Result<ModuleTree, Vec<Error>> {
    SHARED_STRINGS.set(class.shared_strings);
    let setup = Setup::new(class, facts);
    let start = std::time::Instant::now();
    let mut kept = None;
    let wanted = Needs::new();
    let (tree, needs, rejected, closures) = generate_with(class, facts, &setup, &wanted, None, &mut kept);
    round_timing(1, start, "", &needs, &wanted);
    let Some(mut kept) = kept else {
        // The program asks for no enum item and rejects nothing: it is done in one round.
        let tree = tree.expect("a round that keeps nothing is the last");
        return match stub(class, facts, Vec::new()) {
            None => Err(Vec::new()),
            Some(changed) if changed.is_empty() => finished(tree, closures),
            Some(_) => generate_all(class, facts),
        };
    };
    let first = rejected.iter().map(|e| Error { pos: e.pos, message: e.message.clone() }).collect();
    let messages = facts.npe_messages.clone();
    let Some(changed) = stub(class, facts, rejected) else { return Err(first) };
    if !changed.is_empty() {
        let start = std::time::Instant::now();
        // The messages of the dereferences may change where the stubs take away an error of them.
        let present = |messages: &crate::analysis::FastMap<ExprId, Option<String>>| {
            let present = messages.iter().filter(|(id, _)| facts.null_checked.contains_key(id));
            present.map(|(&id, message)| (id, message.clone())).collect::<HashMap<_, _>>()
        };
        if present(&messages) != present(&facts.npe_messages) {
            if crate::timings::enabled() {
                eprintln!("timing: codegen: the stubs change the messages of dereferences");
            }
            return generate_all(class, facts);
        }
        // Which classes are live may change (a stub takes away the only `new` of a class), as
        // nothing else of the setup does where the stubs replace bodies alone.
        let mut stubbed = Setup::new(class, facts);
        let live = std::mem::replace(&mut stubbed.live, setup.live.clone());
        if stubbed != setup {
            if crate::timings::enabled() {
                eprintln!(
                    "timing: codegen: the stubs change what the whole program is written with: {}",
                    stubbed.differences(&setup).join(", ")
                );
            }
            return generate_all(class, facts);
        }
        stubbed.live = live;
        let unlive: Vec<usize> =
            (0..class.types.len()).filter(|&t| stubbed.live[t] != setup.live[t]).collect();
        let setup = stubbed;
        let (needs, rejected, written) = restubbed(class, facts, &setup, &mut kept, &changed, &unlive);
        if crate::timings::enabled() {
            eprintln!(
                "timing: codegen: {written} of {} modules written again with stubs: {:.3} s",
                setup.plans.len(),
                start.elapsed().as_secs_f64()
            );
        }
        if !rejected.is_empty() {
            return Err(rejected);
        }
        let only_enums = anew(class, &setup, &wanted, &needs);
        return rounds(class, facts, &setup, needs, Some(only_enums), Some(kept), 2);
    }
    let only_enums = anew(class, &setup, &wanted, &needs);
    rounds(class, facts, &setup, needs, Some(only_enums), Some(kept), 2)
}

/// The rounds of `generate_all` from round `first`, with the enum items `wanted`, those of a round
/// of enum items alone (`only_enums`) and the first round's modules (`kept`), as earlier rounds
/// left them.
fn rounds(
    class: &Class,
    facts: &Facts,
    setup: &Setup,
    mut wanted: Needs,
    mut only_enums: Option<OnlyEnums>,
    mut kept: Option<Kept>,
    first: usize,
) -> Result<ModuleTree, Vec<Error>> {
    for round in first.. {
        let start = std::time::Instant::now();
        let last = only_enums.is_none() && kept.is_some();
        let (tree, needs, rejected, closures) =
            generate_with(class, facts, setup, &wanted, only_enums.as_ref(), &mut kept);
        let written = match &only_enums {
            Some(only) => format!(" (enum items of {} types)", only.types.len()),
            None if last => " (all enum items, the first round's other items kept)".to_string(),
            None => String::new(),
        };
        round_timing(round, start, &written, &needs, &wanted);
        if only_enums.is_some() {
            // What the enum items reject, or the program once they ask for nothing new, is
            // found by writing all of them with the same items.
            if !rejected.is_empty() || needs.is_subset(&wanted) {
                only_enums = None;
                continue;
            }
            only_enums = Some(anew(class, setup, &wanted, &needs));
            wanted.extend(needs);
            continue;
        }
        if !rejected.is_empty() {
            return Err(rejected);
        }
        if let Some(tree) = tree {
            return finished(tree, closures);
        }
        only_enums = Some(anew(class, setup, &wanted, &needs));
        wanted.extend(needs);
    }
    unreachable!("the rounds end")
}

/// The `--timings` line of a round of the generator.
fn round_timing(round: usize, start: std::time::Instant, written: &str, needs: &Needs, wanted: &Needs) {
    if crate::timings::enabled() {
        let new = needs.iter().filter(|n| !wanted.contains(n)).count();
        eprintln!(
            "timing: codegen: round {round}{written}: {:.3} s, {new} items asked for anew",
            start.elapsed().as_secs_f64()
        );
    }
}

/// The enum items that `needs` asks for and `wanted` lacks, and the types they are of.
fn anew(class: &Class, setup: &Setup, wanted: &Needs, needs: &Needs) -> OnlyEnums {
    let needs: Needs = needs.difference(wanted).copied().collect();
    let types = needs.iter().flat_map(|&need| dispatch::need_types(class, &setup.homes, need)).collect();
    OnlyEnums { needs, types }
}

/// The written program, unless some of its methods have a call that throws inside a closure: those
/// methods, and then any other code.
fn finished(tree: ModuleTree, closures: Vec<Error>) -> Result<ModuleTree, Vec<Error>> {
    if !closures.is_empty() {
        return Err(closures);
    }
    for module in &tree.modules {
        exceptions::check_closures(&module.items_text()).map_err(|e| vec![e])?;
    }
    Ok(tree)
}

/// The first round's modules (`kept`) after a census's stubs changed the bodies of the methods
/// `changed` and which of the classes `unlive` are live (`generate_all_stubbing`): those that hold
/// one of the methods or code that reads one, those that hold one of the classes or depend on the
/// variants of an enum that may hold one (`Found::variant_types`), and those that rejected code
/// in the first round, are written again as the first round wrote them; then the checks after the
/// writing run over all of them. What the program asks for, what it rejects, and how many modules
/// were written again.
fn restubbed(
    class: &Class,
    facts: &Facts,
    setup: &Setup,
    kept: &mut Kept,
    changed: &HashSet<usize>,
    unlive: &[usize],
) -> (Needs, Vec<Error>, usize) {
    let mut module_of_type = vec![usize::MAX; class.types.len()];
    let mut module_of_method = HashMap::new();
    for (index, plan) in setup.plans.iter().enumerate() {
        for &t in plan.types() {
            module_of_type[t] = index;
        }
        for &m in plan.methods() {
            module_of_method.insert(m, index);
        }
    }
    let mut again: std::collections::BTreeSet<usize> = kept.rejecting.iter().copied().collect();
    let add_method = |again: &mut std::collections::BTreeSet<usize>, m: usize| {
        let mut types = vec![class.methods[m].class];
        if let Home::Class(c) | Home::Fields(c) = setup.homes[m] {
            types.push(c);
        }
        again.extend(types.into_iter().map(|t| module_of_type[t]).filter(|&i| i != usize::MAX));
        again.extend(module_of_method.get(&m).copied());
    };
    let changed_classes: HashSet<usize> = changed.iter().map(|&m| class.methods[m].class).collect();
    let reads = |e: &Expr| {
        let mut found = false;
        visit_expr(e, &mut |x| {
            found |= match &x.kind {
                ExprKind::Call { resolved: Some(m), .. }
                | ExprKind::MethodCall { resolved: Some(m), .. }
                | ExprKind::ThisCall { resolved: Some(m), .. }
                | ExprKind::SuperInit { resolved: Some(m), .. } => changed.contains(m),
                ExprKind::New { class: c, ctor, .. } => {
                    changed_classes.contains(c) || ctor.is_some_and(|m| changed.contains(&m))
                }
                _ => false,
            }
        });
        found
    };
    for (m, method) in class.methods.iter().enumerate() {
        let mut found = changed.contains(&m);
        for s in &method.body {
            if !found {
                visit_stmt(s, &mut |e| found |= reads(e), &mut |_| {});
            }
        }
        if found {
            add_method(&mut again, m);
        }
    }
    for (t, decl) in class.types.iter().enumerate() {
        let initializers = decl.fields.iter().chain(&decl.statics).filter_map(|f| f.init.as_ref());
        if initializers.into_iter().any(&reads) && module_of_type[t] != usize::MAX {
            again.insert(module_of_type[t]);
        }
    }
    if !unlive.is_empty() {
        again.extend(unlive.iter().map(|&c| module_of_type[c]).filter(|&i| i != usize::MAX));
        let asked: HashSet<usize> =
            kept.found.iter().flat_map(|found| found.variant_types.iter().copied()).collect();
        let moved: HashSet<usize> = asked
            .into_iter()
            .filter(|&t| hierarchy::variants(class, facts, t).iter().any(|c| unlive.contains(c)))
            .collect();
        for (index, found) in kept.found.iter().enumerate() {
            if found.variant_types.iter().any(|t| moved.contains(t)) {
                again.insert(index);
            }
        }
    }
    let again: Vec<usize> = again.into_iter().collect();
    let _modules = modules::enter(class);
    let wanted = Needs::new();
    let round = Round { wanted: &wanted, impl_needs: HashMap::new(), only_enums: None };
    let written = threads::write_modules(class, facts, setup, &round, None, Some(&again));
    let mut rejected_at = Vec::new();
    for (&index, (module, found)) in again.iter().zip(written) {
        if !module.rejected.is_empty() {
            rejected_at.push(index);
        }
        kept.found[index] = found;
        kept.modules[index] = module;
    }
    kept.rejecting = rejected_at;
    let mut g = Gen::new(class, facts, setup, &round);
    g.add_found(Found::all(&kept.found));
    // What the modules reject, in the order they are written.
    g.rejected = kept.modules.iter_mut().flat_map(|module| std::mem::take(&mut module.rejected)).collect();
    g.check_dereferences();
    g.check_casts();
    g.check_catching();
    (g.needs.take(), std::mem::take(&mut g.rejected), again.len())
}

thread_local! {
    /// Whether the program being written has every string as a `java::JStr`
    /// (`Class::shared_strings`): `rust_type` names its strings so.
    static SHARED_STRINGS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether the program being written has every string as a `java::JStr`.
pub(crate) fn shared_strings() -> bool {
    SHARED_STRINGS.get()
}

thread_local! {
    /// The position of the method being written, which a census stubs when the generator panics.
    static WRITING: std::cell::Cell<Option<Pos>> = const { std::cell::Cell::new(None) };
    /// Whether a panic while writing a method is an error of that method (`catch_panics`).
    static CATCHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Makes a panic while the generator writes a method, from now on in this thread (until
/// `catch_panics(false)`), an error at the method that names the panic, and the generator goes on
/// with the other methods. A census does this, so that it finds every method that makes the
/// generator panic in one run of it.
pub fn catch_panics(on: bool) {
    CATCHING.with(|c| c.set(on));
}

/// The position of the method that the generator was writing when it panicked, if it was writing
/// one; it is forgotten.
pub fn panicked_in() -> Option<Pos> {
    WRITING.with(|w| w.take())
}

/// The enum items that a round of them alone writes (`generate_all`): those asked for anew, and
/// the types they are of (`dispatch::need_types`).
#[derive(Clone)]
struct OnlyEnums {
    needs: Needs,
    types: HashSet<usize>,
}

/// What the first round wrote apart from the enum items of class types, which no later round
/// writes otherwise: the last round writes the enum items among it (`generate_all`). Labels are
/// numbered per item, so an item's text depends on what is before it only through the blank
/// line that may start it (`Gen::module`).
struct Kept {
    modules: Vec<modules::Written>,
    /// What writing each module found.
    found: Vec<Found>,
    /// The modules that rejected code, by index.
    rejecting: Vec<usize>,
}

/// What writing modules finds besides their text (`threads::write_modules`): the enum items
/// that the code uses, and what the checks after the writing and the classes' constants read
/// (`Gen::written`, `Gen::dereferenced`, `Gen::class_values`, `Gen::class_details`,
/// `Gen::class_interfaces`), and the types whose enum's variants the code depends on
/// (`Gen::variant_types`).
#[derive(Clone, Default)]
struct Found {
    needs: Needs,
    written: HashSet<usize>,
    dereferenced: HashSet<ExprId>,
    class_values: std::collections::BTreeSet<usize>,
    class_details: bool,
    class_interfaces: bool,
    variant_types: HashSet<usize>,
}

impl Found {
    /// What the generators of `parts` found together.
    fn all(parts: &[Found]) -> Found {
        let mut all = Found::default();
        for part in parts {
            all.add(part.clone());
        }
        all
    }

    /// Adds what another generator found. Each part only grows as the program is written, so
    /// what generators of parts of it found together is what one of all of it finds.
    fn add(&mut self, more: Found) {
        self.needs.extend(more.needs);
        self.written.extend(more.written);
        self.dereferenced.extend(more.dereferenced);
        self.class_values.extend(more.class_values);
        self.class_details |= more.class_details;
        self.class_interfaces |= more.class_interfaces;
        self.variant_types.extend(more.variant_types);
    }
}

/// A list of classes shared between the users of a generator's caches.
type Classes = std::rc::Rc<[usize]>;

/// What the generator finds of the program before it writes it, the same in every round
/// (`generate_all`).
#[derive(PartialEq)]
struct Setup {
    live: Vec<bool>,
    homes: Vec<Home>,
    fn_names: Vec<String>,
    group_names: HashMap<usize, String>,
    struct_methods: HashMap<usize, Vec<usize>>,
    static_fn_names: HashSet<String>,
    /// `fn_names` as a set, for the temporaries that must not take one.
    fn_name_set: HashSet<String>,
    receiver_sets: HashMap<u64, std::sync::Arc<[usize]>>,
    class_tags: Vec<u16>,
    runs: Vec<bool>,
    super_targets: HashSet<usize>,
    /// The program's modules, in the order they are written.
    plans: Vec<modules::Plan>,
    /// `Gen::conversion_names` and `Gen::stack_forms`, which a generator finds from the rest.
    conversion_names: std::sync::Arc<[String]>,
    stack_forms: HashSet<usize>,
    /// `Gen::stack_sites` of each method that has any.
    stack_sites: HashMap<usize, Vec<(ExprId, usize)>>,
    /// The enums whose `valueOf` a `catch` or `finally` sees fail (`enums::checked_value_ofs`),
    /// found once rather than by each round's generators.
    checked_value_ofs: HashSet<&'static str>,
}

impl Setup {
    /// The names of the fields in which `self` and `other` differ, for the timings.
    fn differences(&self, other: &Setup) -> Vec<&'static str> {
        let fields = [
            ("live", self.live == other.live),
            ("homes", self.homes == other.homes),
            ("fn_names", self.fn_names == other.fn_names),
            ("group_names", self.group_names == other.group_names),
            ("struct_methods", self.struct_methods == other.struct_methods),
            ("static_fn_names", self.static_fn_names == other.static_fn_names),
            ("fn_name_set", self.fn_name_set == other.fn_name_set),
            ("receiver_sets", self.receiver_sets == other.receiver_sets),
            ("class_tags", self.class_tags == other.class_tags),
            ("runs", self.runs == other.runs),
            ("super_targets", self.super_targets == other.super_targets),
            ("plans", self.plans == other.plans),
            ("conversion_names", self.conversion_names == other.conversion_names),
            ("stack_forms", self.stack_forms == other.stack_forms),
            ("stack_sites", self.stack_sites == other.stack_sites),
            ("checked_value_ofs", self.checked_value_ofs == other.checked_value_ofs),
        ];
        fields.into_iter().filter(|(_, same)| !same).map(|(name, _)| name).collect()
    }

    fn new(class: &Class, facts: &Facts) -> Setup {
        // The names of methods are made in the program's modules, before any is written.
        let _modules = modules::enter(class);
        let homes: Vec<Home> = (0..class.methods.len()).map(|m| hierarchy::home(class, facts, m)).collect();
        let start = std::time::Instant::now();
        let (fn_names, group_names) = method_names(class, &homes);
        if crate::timings::enabled() {
            eprintln!("timing: codegen: method names: {:.3} s", start.elapsed().as_secs_f64());
        }
        // The methods that may be emitted in the `impl` of each class's struct (`emitted_in`),
        // in index order.
        let mut struct_methods: HashMap<usize, Vec<usize>> = HashMap::new();
        for (i, m) in class.methods.iter().enumerate() {
            let t = match homes[i] {
                _ if m.is_static => continue,
                _ if m.is_ctor => m.class,
                Home::Class(c) | Home::Fields(c) => c,
                _ => continue,
            };
            struct_methods.entry(t).or_default().push(i);
        }
        let static_fn_names = (0..fn_names.len())
            .filter(|&i| class.methods[i].is_static)
            .map(|i| fn_names[i].clone())
            .collect();
        let fn_name_set = fn_names.iter().cloned().collect();
        let mut setup = Setup {
            live: live_types(class, facts),
            homes,
            fn_names,
            fn_name_set,
            group_names,
            struct_methods,
            static_fn_names,
            receiver_sets: class.receivers.values().map(|s| (dispatch::receiver_key(s), s.clone())).collect(),
            class_tags: dispatch::class_numbers(class),
            runs: running_types(class, facts),
            super_targets: super_targets(class),
            plans: modules::plans(class),
            conversion_names: std::sync::Arc::from([]),
            stack_forms: HashSet::new(),
            stack_sites: HashMap::new(),
            checked_value_ofs: enums::checked_value_ofs(class, facts),
        };
        let wanted = Needs::new();
        let round = Round { wanted: &wanted, impl_needs: HashMap::new(), only_enums: None };
        let g = Gen::new(class, facts, &setup, &round);
        let sites: HashMap<usize, Vec<(ExprId, usize)>> = (0..class.methods.len())
            .map(|m| (m, g.find_stack_sites(m)))
            .filter(|(_, sites)| !sites.is_empty())
            .collect();
        let (conversion_names, stack_forms) = (g.conversion_names().into(), g.stack_forms(&sites));
        setup.conversion_names = conversion_names;
        setup.stack_forms = stack_forms;
        setup.stack_sites = sites;
        setup
    }
}

/// The program, with the enum items `wanted`, once it rejects nothing and asks for no other;
/// the enum items that it uses; and what in it cannot be translated, and the methods with a call
/// that throws inside a closure (see `exceptions::check_closures`). A round of enum items alone
/// (`only_enums`) writes no program. The first round keeps what it wrote in `kept` when it is
/// not the last, and a round after the rounds of enum items writes those among it.
fn generate_with(
    class: &Class,
    facts: &Facts,
    setup: &Setup,
    wanted: &Needs,
    only_enums: Option<&OnlyEnums>,
    kept: &mut Option<Kept>,
) -> (Option<ModuleTree>, Needs, Vec<Error>, Vec<Error>) {
    let _modules = modules::enter(class);
    let round = Round {
        wanted,
        impl_needs: dispatch::impl_needs(wanted, &setup.homes),
        only_enums: match (only_enums, &kept) {
            (Some(only), _) => Some(only.clone()),
            // The last round writes every enum item, which are all asked for anew.
            (None, Some(_)) => Some(OnlyEnums {
                needs: wanted.clone(),
                types: wanted
                    .iter()
                    .filter_map(|need| match *need {
                        Need::Enum(t) => Some(t),
                        _ => None,
                    })
                    .collect(),
            }),
            (None, None) => None,
        },
    };
    let mut g = Gen::new(class, facts, setup, &round);
    let header = match class.types.iter().any(|t| !t.package.is_empty()) {
        true => format!("// Translated from the Java program `{}` by java-to-rust.\n\n", class.name),
        false => format!("// Translated from the Java class `{}` by java-to-rust.\n\n", class.name),
    };
    // The functions of a module named after a struct (`objects::module_fn`), `Point__new`.
    let header = format!("{header}#![allow(non_snake_case)]\n\n");
    // With threads a static holds objects that must be `Send`, which Rust proves through every
    // field of every class the object reaches: javac's `Arc<Locked<..>>` chains go deeper than
    // the default limit of 128.
    let header = if g.facts.threaded {
        format!("{header}#![recursion_limit = \"{RECURSION_LIMIT}\"]\n\n")
    } else {
        header
    };
    if only_enums.is_none()
        && let Some(kept) = kept
    {
        g.add_found(Found::all(&kept.found));
    }
    let first = kept.as_ref().filter(|_| only_enums.is_none()).map(|kept| &kept.modules[..]);
    let (mut written, found): (Vec<modules::Written>, Vec<Found>) =
        threads::write_modules(class, facts, setup, &round, first, None).into_iter().unzip();
    g.add_found(Found::all(&found));
    let rejecting = (0..written.len()).filter(|&i| !written[i].rejected.is_empty()).collect();
    // What the modules reject, in the order they are written.
    g.rejected = written.iter_mut().flat_map(|module| std::mem::take(&mut module.rejected)).collect();
    let needs = g.needs.take();
    if only_enums.is_some() {
        return (None, needs, g.rejected, Vec::new());
    }
    g.check_dereferences();
    g.check_casts();
    g.check_catching();
    let done = g.rejected.is_empty() && needs.is_subset(wanted);
    let written = match kept.take() {
        Some(first) if done => modules::merge(first.modules, written),
        Some(first) => {
            *kept = Some(first);
            return (None, needs, g.rejected, Vec::new());
        }
        None if done => written,
        None => {
            *kept = Some(Kept { modules: written, found, rejecting });
            return (None, needs, g.rejected, Vec::new());
        }
    };
    let (tree, closures) = g.finish(header, written);
    (Some(tree), needs, g.rejected, closures)
}

/// What every module of a round is written with (`generate_with`).
struct Round<'r> {
    wanted: &'r Needs,
    /// The items of `wanted` in the `impl` of the enum of each type (`dispatch::impl_needs`).
    impl_needs: HashMap<usize, Vec<Need>>,
    only_enums: Option<OnlyEnums>,
}

impl<'a> Gen<'a> {
    /// A generator for a round that writes with `round`, before it writes anything.
    fn new(class: &'a Class, facts: &'a Facts, setup: &'a Setup, round: &'a Round<'a>) -> Gen<'a> {
        Gen {
            class,
            facts,
            fn_names: &setup.fn_names,
            fn_name_set: &setup.fn_name_set,
            static_fn_names: &setup.static_fn_names,
            variant_names: Default::default(),
            cell_names: Default::default(),
            object_hash_code: Default::default(),
            enum_bodies: Default::default(),
            reps: Default::default(),
            variant_lists: Default::default(),
            enum_variant_lists: Default::default(),
            passing_lists: Default::default(),
            cast_classes: Default::default(),
            enum_variant_sets: Default::default(),
            variant_types: Default::default(),
            checked_value_ofs: &setup.checked_value_ofs,
            group_names: &setup.group_names,
            receiver_sets: &setup.receiver_sets,
            conversion_names: setup.conversion_names.clone(),
            live: &setup.live,
            class_tags: &setup.class_tags,
            runs: &setup.runs,
            wanted: round.wanted,
            impl_needs: &round.impl_needs,
            needs: std::cell::RefCell::new(Needs::new()),
            homes: &setup.homes,
            struct_methods: &setup.struct_methods,
            super_targets: &setup.super_targets,
            this_struct: 0,
            method: 0,
            in_field_inits: false,
            borrowed: HashSet::new(),
            borrowed_optional: HashSet::new(),
            after_body: &[],
            borrowed_optional_objects: HashSet::new(),
            int_receiver: false,
            double_receiver: false,
            this_form: ThisForm::Shared,
            thin_this: false,
            lazy_text: false,
            formatting: false,
            chars: false,
            indexed: false,
            exhaustive: false,
            chain: false,
            field_locals: HashMap::new(),
            bound_params: None,
            traits: std::collections::BTreeSet::new(),
            ctor_result: None,
            printing: false,
            printing_format: false,
            guarded: false,
            steady: None,
            annotations: Vec::new(),
            locals: HashMap::new(),
            boxed: HashMap::new(),
            elements: HashMap::new(),
            sliced: HashMap::new(),
            sliced_accesses: HashSet::new(),
            out: String::new(),
            indent: 0,
            frames: Vec::new(),
            pending: None,
            labels: 0,
            ret: Type::Void,
            uses_runtime: false,
            throwing: false,
            attempts: Vec::new(),
            finallies: Vec::new(),
            finally_ends: Vec::new(),
            yield_slots: Vec::new(),
            optional_yield: false,
            attempt_count: 0,
            try_assigned: HashSet::new(),
            unset_locals: HashMap::new(),
            dereferenced: HashSet::new(),
            checked_cast: None,
            catching_now: HashSet::new(),
            discarded: None,
            written: HashSet::new(),
            rejected: Vec::new(),
            closures: Vec::new(),
            slots: HashMap::new(),
            slot_param: String::new(),
            on_stack: false,
            stack_forms: &setup.stack_forms,
            stack_sites: &setup.stack_sites,
            only_enums: round.only_enums.as_ref(),
            class_values: Default::default(),
            class_value_names: Vec::new(),
            class_details: false,
            class_interfaces: false,
            arms_method: None,
            arm_parts: String::new(),
        }
    }
}

impl Gen<'_> {
    /// Adds what other generators found (`Found`) to what this one has.
    fn add_found(&mut self, found: Found) {
        let mut all = self.take_found();
        all.add(found);
        *self.needs.get_mut() = all.needs;
        self.written = all.written;
        self.dereferenced = all.dereferenced;
        self.class_values = all.class_values;
        self.class_details = all.class_details;
        self.class_interfaces = all.class_interfaces;
        *self.variant_types.get_mut() = all.variant_types;
    }

    /// What this generator found, taken.
    fn take_found(&mut self) -> Found {
        Found {
            needs: self.needs.take(),
            written: std::mem::take(&mut self.written),
            dereferenced: std::mem::take(&mut self.dereferenced),
            class_values: std::mem::take(&mut self.class_values),
            class_details: self.class_details,
            class_interfaces: self.class_interfaces,
            variant_types: self.variant_types.take(),
        }
    }
}

/// How deep rustc follows a trait such as `Send` through the fields of nested types.
/// The classes without objects in a translation, which are no type of it, whose class literals
/// are the runtime's classes (`java::Class::RUNTIME`, `java::Class::SYSTEM`).
pub const OBJECTLESS_CLASSES: [&str; 2] = ["java.lang.Runtime", "java.lang.System"];

const RECURSION_LIMIT: u32 = 1024;

/// The crate root's allocator: the runtime's, which caches small blocks (runtime `allocator.rs`),
/// as Java programs make and drop many small objects.
const ALLOCATOR: &str = "#[global_allocator]\nstatic JAVA_RUNTIME_ALLOCATOR: java_runtime::Allocator = java_runtime::Allocator;\n";

/// How `this` is written in the code being generated.
#[derive(Clone, Copy, PartialEq)]
enum ThisForm {
    /// `this`, a reference to the shared object: `&Rc<RefCell<C>>`, or `&C` for an immutable class.
    Shared,
    /// `self`, in a method that borrows its object.
    SelfRef,
    /// `this`, the plain value that a constructor is building.
    Local,
    /// `self`, the value of a type's enum, in a method of the enum.
    Enum,
}

/// The innermost enclosing loop, which decides how `continue` is translated.
#[derive(Clone, Copy)]
enum Loop<'a> {
    /// Rust's `continue` does what Java's does.
    Plain,
    /// A `for` loop translated to `while`: `continue` must run the update first.
    For(&'a [Expr]),
    /// A `do`-`while` loop translated to `loop`: `continue` must test the condition first.
    DoWhile(&'a Expr),
}

/// A piece of a string concatenation or of printed text.
enum Part {
    /// Constant text.
    Lit(String),
    /// A value shown through its `Display`, such as a number.
    Arg(Code),
    /// A string, `char` or builder, which a concatenation takes as it is (a `java::Piece`), so
    /// that the halves of a pair that meet join.
    Text(Code),
}

struct Gen<'a> {
    class: &'a Class,
    facts: &'a Facts,
    /// Where each method is emitted (see `analysis::hierarchy`).
    homes: &'a [Home],
    /// The methods that may be emitted in the `impl` of each class's struct, in index order.
    struct_methods: &'a HashMap<usize, Vec<usize>>,
    /// The names of the static methods, which are free functions in the namespace of locals.
    static_fn_names: &'a HashSet<String>,
    /// The names of the variants of the enum of each type (see `dispatch::variant`), once made.
    variant_names: std::cell::RefCell<HashMap<usize, std::rc::Rc<HashMap<usize, String>>>>,
    /// The names of the cells of each type's statics, by field (see `objects::cell_names`), once
    /// made.
    cell_names: std::cell::RefCell<HashMap<usize, std::rc::Rc<HashMap<String, String>>>>,
    /// `Object`'s `hashCode`, if the program has it (`hash_index`), found once.
    object_hash_code: std::cell::OnceCell<Option<usize>>,
    /// The body that a call of a method on the enum of a type calls directly, by type and method
    /// (`dispatch::enum_body`), once found.
    enum_bodies: std::cell::RefCell<HashMap<(usize, usize), Option<usize>>>,
    /// How the values of each type are represented, and the variants of its enum, once found.
    reps: std::cell::RefCell<HashMap<usize, hierarchy::Rep>>,
    variant_lists: std::cell::RefCell<HashMap<usize, std::rc::Rc<Vec<usize>>>>,
    /// The variants of the enum of each type that have objects (`dispatch::enum_variants`), once
    /// found.
    enum_variant_lists: std::cell::RefCell<HashMap<usize, std::rc::Rc<[usize]>>>,
    /// The variants of the enum of each type whose objects are of another type
    /// (`dispatch::passing`), by the two, once found.
    passing_lists: std::cell::RefCell<HashMap<(usize, usize), Classes>>,
    /// The text of each class as a `ClassCastException` names it (`dispatch::cast_class`), once
    /// made.
    cast_classes: HashMap<usize, String>,
    /// `enum_variant_lists` as sets, for `dispatch::is_enum_variant`.
    enum_variant_sets: std::cell::RefCell<HashMap<usize, std::rc::Rc<HashSet<usize>>>>,
    /// The types whose `enum_variants` the code written asked for, which depend on which classes
    /// are live: a census that stubs code writes the modules that asked again where a stub
    /// changes that (`generate_all_stubbing`).
    variant_types: std::cell::RefCell<HashSet<usize>>,
    /// The enums whose `valueOf` a `catch` or `finally` sees fail (see `enums::checked_value_of`).
    checked_value_ofs: &'a HashSet<&'static str>,
    /// The name of each override group (by group id): the name of its dispatchers.
    group_names: &'a HashMap<usize, String>,
    /// The sets of classes in `Class::receivers`, by their key (`receiver_key`).
    receiver_sets: &'a HashMap<u64, std::sync::Arc<[usize]>>,
    /// The name of each type in the names of its conversions, unique in the program
    /// (`conversion_names`).
    conversion_names: std::sync::Arc<[String]>,
    /// Which classes have a struct (see `live_types`).
    live: &'a [bool],
    /// The number of each type in a `java::Thin` (see `dispatch::class_numbers`).
    class_tags: &'a [u16],
    /// Which classes' instance methods can run (see `running_types`).
    runs: &'a [bool],
    /// The enum items to write, which an earlier pass found used (see `generate`).
    wanted: &'a Needs,
    /// The items of `wanted` in the `impl` of the enum of each type (`dispatch::impl_needs`).
    impl_needs: &'a HashMap<usize, Vec<Need>>,
    /// The enum items that the code written so far uses.
    needs: std::cell::RefCell<Needs>,
    /// The methods that a `super.m(...)` call names, which run although a subclass overrides them.
    super_targets: &'a HashSet<usize>,
    /// The class (index into `Class::types`) whose struct `this` is in the code being written:
    /// the class of the `impl` that the method is emitted in.
    this_struct: usize,
    /// The index of the current method.
    method: usize,
    /// Whether the code being written is a class's field initializers, which run in its
    /// constructors (`field_inits`).
    in_field_inits: bool,
    /// The current method's string variables that are `&str`: borrowed parameters, and
    /// possibly-null strings inside a null check that narrowed them.
    borrowed: HashSet<String>,
    /// The current method's possibly-null string parameters that are borrowed as `Option<&str>`.
    borrowed_optional: HashSet<String>,
    /// The update expressions of the `for` loop whose body is being written, which follow its
    /// statements in their scope: a variable they assign is not narrowed there
    /// (`early_null_exit`).
    after_body: &'a [Expr],
    /// The current method's possibly-null object parameters that are borrowed as
    /// `&Option<Rc<..>>`: a read of one as a value clones it.
    borrowed_optional_objects: HashSet<String>,
    /// Whether the current method calls a method on an `i32` or `f64` value that is not a
    /// literal. Rust needs the type of such a value to be known, so variables initialized with a
    /// bare literal then get their type written (see `annotation`).
    int_receiver: bool,
    double_receiver: bool,
    /// How `this` is written in the code being generated.
    this_form: ThisForm,
    /// Whether the code being written is a function of the module of a type's enum, whose `this`
    /// is the enum's value itself (`this: Shape`, see `Gen::enum_fn`), rather than a method of the
    /// enum's `impl`, whose `self` borrows it.
    thin_this: bool,
    /// Whether the concatenation being written takes objects, enums and builders as they are, as
    /// `java::Piece`s whose text is made when the string is built, rather than converted to a
    /// string first (see `parts`).
    lazy_text: bool,
    /// Whether the code being written is an operand of a string concatenation.
    formatting: bool,
    /// Whether the switch being written matches its `char` selector as a Rust `char`.
    chars: bool,
    /// Whether the switch being written on a string matches the number of the case of its label
    /// (`java::string_case`), as it has many labels.
    indexed: bool,
    /// Whether the cases of the switch being written cover every value of its enum selector.
    exhaustive: bool,
    /// Whether the `?:` being written is the else branch of another, so that the chain is written
    /// as `if .. else if ..` on lines of its own.
    chain: bool,
    /// The locals that hold the fields that a constructor's initializers computed first, by
    /// Java field name: the later initializers read them there.
    field_locals: HashMap<String, String>,
    /// In a closure's body, the lambda's method whose parameters are bound to the converted
    /// arguments already (see `closures::inline_call`).
    bound_params: Option<usize>,
    /// The extension traits of closure types in other modules that the module being written
    /// calls default methods of, by path (see `closures::closure_trait`).
    traits: std::collections::BTreeSet<String>,
    /// In a constructor, what a `return` gives back: the object being built.
    ctor_result: Option<&'static str>,
    /// Whether the text being formatted is printed.
    printing: bool,
    /// Whether the next `String.format` is `printf`'s, which prints the text itself (see
    /// `java::print_format`).
    printing_format: bool,
    /// Whether the statement being emitted also borrows an object mutably (see `conflicts`).
    guarded: bool,
    /// What the statement being emitted does to the plain objects it names, where it is known
    /// (`strings::Steady`); `None` outside a statement, where no field is borrowed in place.
    steady: Option<strings::SteadyOf<'a>>,
    /// Placeholders for type annotations decided at the end of the method: (placeholder, type,
    /// annotation).
    annotations: Vec<(String, Type, String)>,
    fn_names: &'a [String],
    /// `fn_names` as a set.
    fn_name_set: &'a HashSet<String>,
    /// Java local variable names to Rust names, for the current method.
    locals: HashMap<String, String>,
    /// The boxed types of the current method's parameters and locals, by Java name.
    boxed: HashMap<String, Type>,
    /// Reads of elements that a for-each-style loop has bound to a variable, by expression.
    elements: HashMap<ExprId, String>,
    /// The plain arrays that the loop being written reaches through slices (see
    /// `sliced_loops.rs`): Java name to the slice's Rust name and how many rows it skips.
    sliced: HashMap<String, (String, usize)>,
    /// The accesses that the check before a sliced loop puts in bounds.
    sliced_accesses: HashSet<ExprId>,
    out: String,
    indent: usize,
    /// The statements that Java can break out of or continue, innermost last (see `switch.rs`).
    frames: Vec<Frame<'a>>,
    /// The Java label of the statement whose frame opens next.
    pending: Option<String>,
    /// How many Rust labels have been made for statements without a Java label in the item being
    /// written: each item of a module numbers its own (`item_start`).
    labels: usize,
    /// The return type of the current method.
    ret: Type,
    uses_runtime: bool,
    /// Whether the current method returns `Result<_, Throwable>` (see `exceptions`).
    throwing: bool,
    /// The `try` blocks around the code being written, innermost last.
    attempts: Vec<exceptions::Attempt>,
    /// The `try` statements with a `finally` whose body or `catch` clauses are being written,
    /// innermost last.
    finallies: Vec<exceptions::Finally>,
    /// The `finally` blocks being written, innermost last: the condition, on what ended their
    /// `try`, that holds when an exception did; `None` where only a completed `try` gets there.
    finally_ends: Vec<Option<String>>,
    /// The switch expressions (by frame index) that a `yield` leaves through a `finally`: their
    /// value waits in a `yielded{index}` local (see `switch.rs`).
    yield_slots: Vec<usize>,
    /// Whether the innermost switch expression may be null, so that each value it yields is an
    /// `Option`.
    optional_yield: bool,
    /// How many `try` blocks of the current method have a label.
    attempt_count: usize,
    /// The current method's locals declared without a value that a `try` block assigns (see
    /// `exceptions::try_assigned`).
    try_assigned: HashSet<String>,
    /// The locals of `try_assigned` declared so far without a value to start as, by their types:
    /// a `try` block that sets one carries its value out (see `exceptions::carried_out`).
    unset_locals: HashMap<String, Type>,
    /// The objects of `Facts::null_checked` whose checked dereference has been written, and the
    /// casts of `Facts::raising` whose checked form has; and everything in a `catch` clause that
    /// is not written, which needs neither.
    dereferenced: HashSet<ExprId>,
    /// The cast of `Facts::raising` that `class_cast` is writing, in the checked form.
    checked_cast: Option<ExprId>,
    /// The operations of `Facts::catching` being written inside `java::catching`.
    catching_now: HashSet<ExprId>,
    /// The expression of the statement being written, whose value is not used: a string it
    /// makes is not shared (`shared_made`).
    discarded: Option<ExprId>,
    /// The methods whose bodies have been written. A body that is not, as no call needs it or
    /// it never runs, dereferences nothing.
    written: HashSet<usize>,
    /// The constructs found that cannot be translated, of which `generate` reports the first.
    rejected: Vec<Error>,
    /// The methods written with a call that throws inside a closure, at their positions.
    closures: Vec<Error>,
    /// The slots of the method being written, by the `new` or call of a factory that makes its
    /// object in each: the slot's name and the object's class (see `stack_objects`).
    slots: HashMap<ExprId, (String, usize)>,
    /// The name of the slot that a function's form on the stack is given.
    slot_param: String,
    /// Whether the constructor or factory being written is its form on the stack, which makes
    /// its object in the slot it is given.
    on_stack: bool,
    /// The constructors and factories that get a form on the stack (see `stack_objects`).
    stack_forms: &'a HashSet<usize>,
    /// `Setup::stack_sites`.
    stack_sites: &'a HashMap<usize, Vec<(ExprId, usize)>>,
    /// The enum items written in a round of them alone, to find what they ask for, or all of
    /// them in the last round, which takes the other items from the first (`generate_all`).
    only_enums: Option<&'a OnlyEnums>,
    /// The classes of the program whose `java::Class` values the code names, each declared once
    /// as a static at the crate root (`class_value_constants`).
    class_values: std::collections::BTreeSet<usize>,
    /// The name of each class's constant, made when the first is named (`class_value_names`).
    class_value_names: Vec<String>,
    /// Whether the code asks a class for its modifiers or its nesting, which the constants then
    /// carry (`class_value`).
    class_details: bool,
    /// Whether the code asks a class for its interfaces (`ClassOp::InterfaceCount`), which the
    /// constants then carry (`class_value`).
    class_interfaces: bool,
    /// The method whose `match` on the class number `match_arms_or` writes, while one is set, so
    /// that a `match` over thousands of classes is cut into methods of its own.
    arms_method: Option<dispatch::ArmsMethod>,
    /// The methods those cuts make, which `flush_arm_parts` writes after the method.
    arm_parts: String,
}

impl<'a> Gen<'a> {
    fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        if self.guarded {
            self.out.push_str(&scoped_arguments(text));
        } else {
            self.out.push_str(text);
        }
        self.out.push('\n');
    }

    fn runtime(&mut self, path: &str) -> String {
        self.uses_runtime = true;
        format!("java::{path}")
    }

    /// The Rust type of a builder: a `java::SharedStringBuilder` where two threads may reach
    /// builders (`Facts::shared_builders`).
    fn builder_type(&mut self) -> String {
        self.runtime(if self.facts.shared_builders { "SharedStringBuilder" } else { "StringBuilder" })
    }

    fn local(&self, java: &str) -> String {
        self.locals.get(java).cloned().unwrap_or_else(|| snake_case(java))
    }

    /// `code` as the receiver of a method call on a value of Java type `ty`.
    fn recv(&mut self, code: &Code, ty: Type) -> String {
        if code.literal.is_none() {
            match ty {
                Type::Int => self.int_receiver = true,
                Type::Double => self.double_receiver = true,
                _ => {}
            }
        }
        code.receiver()
    }

    /// The type written for a variable initialized with an untyped expression: `: i64`, since an
    /// integer literal would default to `i32`; for `i32` and `f64`, which are the defaults, only
    /// when the method calls methods on such values.
    fn annotation(&mut self, ty: Type, text: String) -> String {
        if !matches!(ty, Type::Int | Type::Double) {
            return text;
        }
        let placeholder = format!("\u{1}{}\u{1}", self.annotations.len());
        self.annotations.push((placeholder.clone(), ty, text));
        placeholder
    }

    fn resolve_annotations(&mut self, from: usize) {
        let mut text = self.out.split_off(from);
        for (placeholder, ty, annotation) in self.annotations.drain(..) {
            let needed = if ty == Type::Int { self.int_receiver } else { self.double_receiver };
            text = text.replace(&placeholder, if needed { &annotation } else { "" });
        }
        self.out.push_str(&text);
    }

    fn no_overflow(&self, id: ExprId) -> bool {
        self.facts.no_overflow.contains(&id)
    }

    /// Sets up the state for writing method `index`: its locals and the forms of its parameters.
    fn enter_method(&mut self, index: usize) {
        let m = &self.class.methods[index];
        self.ret = m.ret;
        self.method = index;
        self.throwing = self.facts.throwing.contains(&index);
        self.attempts.clear();
        self.finallies.clear();
        self.finally_ends.clear();
        self.attempt_count = 0;
        self.try_assigned = exceptions::try_assigned(self.facts, &m.body);
        self.unset_locals.clear();
        let starts = |ty: Type| self.try_start(ty).is_some();
        let optional = |ty: Type, name: &str| self.optional_var(index, ty, name);
        if let Some(e) = exceptions::check_try_assigned(&m.body, &self.try_assigned, starts, optional) {
            self.rejected.push(e);
        }
        self.name_locals(m);
        self.int_receiver = false;
        self.double_receiver = false;
        self.borrowed = (0..m.params.len())
            .filter(|&i| {
                self.facts.borrowed_params.contains(&(index, i))
                    || self.facts.borrowed_objects.contains(&(index, i))
            })
            .map(|i| m.params[i].name.clone())
            .collect();
        self.borrowed_optional = (0..m.params.len())
            .filter(|&i| self.facts.borrowed_optional_params.contains(&(index, i)))
            .map(|i| m.params[i].name.clone())
            .collect();
        self.borrowed_optional_objects = (0..m.params.len())
            .filter(|&i| self.facts.borrowed_optional_objects.contains(&(index, i)))
            .map(|i| m.params[i].name.clone())
            .collect();
    }

    /// The parameters of method `index` with their Rust types. For the method's own body
    /// (`body`), a parameter that it assigns is `mut` and one that it ignores is `_x`.
    fn typed_params(&mut self, index: usize, body: bool) -> Vec<String> {
        let m = &self.class.methods[index];
        m.params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mutable = if body && assigns_any(&m.body, &p.name) { "mut " } else { "" };
                let ty = if self.borrowed_optional.contains(&p.name) {
                    "Option<&java::Wtf8>".to_string()
                } else if self.facts.borrowed_optional_objects.contains(&(index, i)) {
                    format!("&Option<{}>", self.ty_name(p.ty))
                } else if self.optional_var(index, p.ty, &p.name) {
                    format!("Option<{}>", self.ty_name(p.ty))
                } else if self.facts.borrowed_objects.contains(&(index, i)) {
                    format!("&{}", self.ty_name(p.ty))
                } else if self.borrowed.contains(&p.name) {
                    "&java::Wtf8".to_string()
                } else if let Some(&written) = self.facts.borrowed_arrays.get(&(index, i)) {
                    self.borrowed_ty(p.ty, written)
                } else {
                    self.ty_name(p.ty)
                };
                // A body that ignores a parameter (a default that overrides replace) names it `_x`.
                // A copy into the class of an array reads that array for its class only, beside
                // the copy (`Class::array_likes`).
                let mut used = !body;
                for s in &m.body {
                    visit_stmt(
                        s,
                        &mut |e| {
                            used |= mentions(e, &p.name);
                            if let Some(like) = self.class.array_likes.get(&e.id) {
                                visit_expr(like, &mut |x| used |= mentions(x, &p.name));
                            }
                        },
                        &mut |_| {},
                    );
                }
                let name = self.local(&p.name);
                let name = if used { name } else { format!("_{}", name.trim_start_matches("r#")) };
                format!("{mutable}{name}: {ty}")
            })
            .collect()
    }

    /// The return type of method `index` as it is written after its parameters: ` -> T`, or
    /// nothing.
    fn ret_text(&mut self, index: usize) -> String {
        let plain = self.plain_ret_text(index);
        if !self.facts.throwing.contains(&index) {
            return plain;
        }
        let ok = plain.strip_prefix(" -> ").unwrap_or("()").to_string();
        format!(" -> Result<{ok}, {}>", self.throwable_type())
    }

    /// `ret_text` of a method that does not throw.
    fn plain_ret_text(&mut self, index: usize) -> String {
        let m = &self.class.methods[index];
        let name = &self.class.types[m.class].name;
        if m.is_ctor && self.builds_plain(name) {
            format!(" -> {}", self.generic_struct(m.class))
        } else if m.is_ctor {
            let args = self.type_params(&self.class.types[m.class].type_params).1;
            format!(" -> {}", self.object_type_with(name, &args))
        } else if m.ret == Type::Void {
            String::new()
        } else if crate::analysis::nullable_type(m.ret) && self.facts.nullable_returns.contains(&index) {
            format!(" -> Option<{}>", self.ty_name(m.ret))
        } else if self.facts.owned_returns.contains(&index) {
            format!(" -> {}", self.plain_ty(m.ret))
        } else {
            format!(" -> {}", self.ty_name(m.ret))
        }
    }

    /// Whether method `index` is a function of its class's module: a static method, or the
    /// `init` of a class whose static initialization runs eagerly.
    fn emitted_free(&self, index: usize) -> bool {
        let m = &self.class.methods[index];
        if m.name == "<clinit>" { self.facts.eager_init.contains(&m.class) } else { m.is_static }
    }

    fn method(&mut self, index: usize) {
        self.method_form(index);
        if self.stack_forms.contains(&index) {
            self.on_stack = true;
            self.method_form(index);
            self.on_stack = false;
        }
    }

    /// Writes method `index`, or its form on the stack when `on_stack` is set.
    fn method_form(&mut self, index: usize) {
        if !CATCHING.with(|c| c.get()) {
            return self.write_method(index);
        }
        let (start, indent, outer) = (self.out.len(), self.indent, WRITING.with(|w| w.get()));
        let thin = self.thin_this;
        let written = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.write_method(index)));
        if let Err(panic) = written {
            self.out.truncate(start);
            self.indent = indent;
            self.thin_this = thin;
            // Its code is not written, so what it dereferences is not looked for.
            self.written.remove(&index);
            WRITING.with(|w| w.set(outer));
            let message = format!("the translator panicked: {}", crate::lower::panic_message(&*panic));
            self.rejected.push(Error { pos: self.class.methods[index].pos, message });
        }
    }

    fn write_method(&mut self, index: usize) {
        let m = &self.class.methods[index];
        let outer = WRITING.with(|w| w.replace(Some(m.pos)));
        let outer_slots = std::mem::take(&mut self.slots);
        self.enter_method(index);
        let start = self.out.len();
        let name = &self.class.types[m.class].name;
        self.this_struct = match self.homes[index] {
            Home::Class(c) | Home::Fields(c) | Home::Enum(c) => c,
            Home::Free | Home::Nowhere => m.class,
        };
        let is_enum = self.class.types[self.this_struct].enum_constants.is_some();
        let on_enum = matches!(self.homes[index], Home::Enum(_)) && !m.is_ctor;
        let borrows =
            !m.is_static && !m.is_ctor && !on_enum && (is_enum || self.facts.self_methods.contains(&index));
        // A method of a type's enum is a function of its module, which takes the value itself.
        let thin = self.thin_fn(index);
        let outer_thin = std::mem::replace(&mut self.thin_this, thin);
        self.this_form = match () {
            _ if on_enum => ThisForm::Enum,
            _ if borrows => ThisForm::SelfRef,
            _ => ThisForm::Shared,
        };
        let mut params = Vec::new();
        if borrows && is_enum {
            // An enum is a `Copy` value, so its methods take `self` by value.
            params.push("self".to_string());
        } else if borrows {
            // A celled object's fields are cells, written through `&self`.
            let mutable = self.facts.mutating.contains(&index)
                && !self.celled(&self.class.types[self.this_struct].name);
            params.push(if mutable { "&mut self" } else { "&self" }.to_string());
        } else if thin {
            params.push(format!("this: {}", struct_name(&self.class.types[self.this_struct].name)));
        } else if on_enum {
            params.push("&self".to_string());
        } else if !m.is_static && !m.is_ctor {
            params.push(format!("this: {}", self.this_type()));
        }
        let receivers = params.len();
        params.extend(self.typed_params(index, true));
        let ret = self.ret_text(index);
        // The body of a method or constructor that never runs may call what was never emitted,
        // such as the default methods of an interface on a class that implements it without
        // objects, or the methods of a superclass whose values are all of another subclass.
        // Reach marks a method of a class without objects with a `NoObject` body.
        // So does the factory of a class built whole (`analysis::whole_objects`) when no object
        // of the class is created: its `construct<Name>` method is such a body.
        // So does a factory whose `construct<Name>` call never runs, when nothing calls that
        // constructor of a class that has objects (`UnsupportedOperationException(String,
        // Throwable)` in javac).
        let stub = |m: &Method| {
            matches!(
                &m.body[..],
                [Stmt::Expr(Expr { kind: ExprKind::Builtin(Builtin::NoObject { .. }, _), .. })]
            )
        };
        // (A census's stub has a message, which its constructor ends the program with.)
        let pruned = |m: &Method| matches!(&m.body[..], [Stmt::Expr(Expr { kind: ExprKind::Builtin(Builtin::NoObject { .. }, args), .. })] if args.is_empty());
        let factory_without_objects = m.is_static
            && match &m.body[..] {
                [
                    Stmt::Local { init: Some(Expr { kind: ExprKind::New { class, .. }, .. }), .. },
                    rest @ ..,
                ] => {
                    !self.runs[*class]
                        || matches!(rest, [Stmt::Expr(Expr { kind: ExprKind::MethodCall { resolved: Some(c), .. }, .. }), ..]
                            if pruned(&self.class.methods[*c]))
                }
                _ => false,
            };
        let never_runs =
            !m.is_static && m.name != "<clinit>" && !self.runs[m.class] || factory_without_objects || stub(m);
        if never_runs {
            self.line("#[allow(unused_variables)]");
        }
        let fn_name = if m.name == "<clinit>" {
            let init = statics::init_name(name);
            self.init_fast_path(m.class, &init, &ret);
            format!("{init}_slow")
        } else if self.module_fn(index) {
            format!("{}__{}", self.struct_of(self.this_struct), self.fn_names[index])
        } else if thin {
            self.enum_fn(self.this_struct, &self.fn_names[index])
        } else {
            self.fn_names[index].clone()
        };
        // The `new`s and calls whose objects the method makes in slots on its stack.
        let sites = if never_runs { Vec::new() } else { self.stack_sites(index) };
        self.name_slots(index, &sites);
        let fn_name = if self.on_stack {
            // The slot comes after the receiver.
            let param = format!("{}: {}", self.slot_param, self.slot_param_type(index));
            params.insert(receivers, param);
            format!("{fn_name}_on_stack")
        } else {
            fn_name
        };
        let generics = self.type_params(&m.type_params).0;
        self.line(&format!("{}fn {fn_name}{generics}({}){ret} {{", self.vis(), params.join(", ")));
        self.indent += 1;
        self.declare_slots(&sites);
        if m.name == "<clinit>" {
            self.init_guard(m.class);
            // Java initializes the superclass first, then the superinterfaces that declare
            // default methods (`default_superinterfaces`).
            let superclass = self.class.hierarchy.superclass(m.class);
            if let Some(s) = superclass.and_then(|s| self.facts.init_class(self.class, s)) {
                let call = self.init_run(s);
                self.line(&format!("{call};"));
            }
            for i in default_superinterfaces(self.class, m.class) {
                if self.facts.eager_init.contains(&i) {
                    let call = self.init_run(i);
                    self.line(&format!("{call};"));
                }
            }
        } else if self.facts.entry_init.contains(&index)
            && let Some(t) = self.facts.init_class(self.class, m.class)
        {
            let call = self.init_run(t);
            self.line(&format!("{call};"));
        }
        // The charsets of the JDK that the program may look up by name, before anything does.
        if m.is_main {
            self.register_charsets();
        }
        // The program ends when its last thread does, not when `main` returns.
        if m.is_main && self.facts.threaded {
            let guard = self.runtime("main_thread");
            self.line(&format!("let _main = {guard}();"));
        }
        self.method_monitor(index);
        let initializes = m.name == "<clinit>" && self.facts.process_statics(m.class);
        if !never_runs {
            self.written.insert(index);
        }
        // A census's stub that names its rejection ends the program with it (`Builtin::NoObject`).
        let stub_message = match &m.body[..] {
            [Stmt::Expr(Expr { kind: ExprKind::Builtin(Builtin::NoObject { .. }, args), .. })] => {
                match &args[..] {
                    [Expr { kind: ExprKind::Str(message), .. }] => Some(message),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(message) = stub_message {
            let call = format!("{}({})", self.runtime("not_translated"), code::rust_string(message));
            self.line(&call);
        } else if never_runs {
            self.line(&no_object(&self.class.types[m.class].name));
        } else if m.is_ctor {
            self.constructor(m);
        } else if let Some(update) = self.atomic_update(m, index) {
            self.line(&update);
        } else {
            self.stmts(&m.body, !initializes);
            if initializes {
                self.init_done(m.class);
            }
            if self.throwing && m.ret == Type::Void && exceptions::completes_all(&m.body) {
                self.line("Ok(())");
            }
        }
        self.indent -= 1;
        self.line("}");
        self.resolve_annotations(start);
        if let Err(e) = exceptions::check_closures(&self.out[start..]) {
            self.closures.push(Error { pos: m.pos, ..e });
        }
        self.slots = outer_slots;
        self.thin_this = outer_thin;
        WRITING.with(|w| w.set(outer));
    }

    /// Picks Rust names for the method's parameters and locals: snake case, distinct from each
    /// other and from the free functions, which share Rust's value namespace.
    fn name_locals(&mut self, m: &Method) {
        let mut java_names = Vec::new();
        java_names.extend(m.params.iter().map(|p| p.name.clone()));
        collect_locals(&m.body, &mut java_names);
        // Static methods are free functions, in the value namespace that locals share; instance
        // methods and constructors are reached through their type or object.
        let mut used: HashSet<String> = HashSet::from(["java".to_string()]);
        // A constructor builds its object in `obj` (`objects.rs`), which a parameter such as
        // `CleanerImpl.PhantomCleanableRef`'s `obj` must not shadow.
        if m.is_ctor {
            used.insert("obj".into());
        }
        self.locals.clear();
        for name in java_names {
            if let std::collections::hash_map::Entry::Vacant(entry) = self.locals.entry(name) {
                let rust = unique_beside(snake_case(entry.key()), &mut used, self.static_fn_names);
                entry.insert(rust);
            }
        }
        self.boxed = boxed_locals(m);
    }
}

/// The body of a method that never runs, as no object of the class named `name` is created.
fn no_object(name: &str) -> String {
    format!("unreachable!(\"no object of {name} is created\")")
}

/// A borrow scoped with braces, `{ x.borrow().f }`, is a function argument of its own, an
/// assigned value or the whole value of a block in a guarded statement, where rustc's
/// unused_braces lint would ask to drop braces that end the borrow; there it is written
/// `{ let value = x.borrow().f; value }`. The lint leaves the
/// arguments of a macro alone, and so does this. Text in string literals is left alone, and so
/// is a block with statements (`{ a = 1; a }`, `{ init_prog(); X.get() }`), which the lint
/// accepts and which would not be an expression after `let value =`.
fn scoped_arguments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;
    // For each open parenthesis, whether it holds a macro's arguments.
    let mut parens: Vec<bool> = Vec::new();
    while i < bytes.len() {
        let c = bytes[i];
        if escaped {
            escaped = false;
        } else if in_string {
            escaped = c == b'\\';
            in_string = c != b'"';
        } else if c == b'"' {
            in_string = true;
        } else if c == b'(' {
            parens.push(text[..i].ends_with('!'));
        } else if c == b')' {
            parens.pop();
        } else if text[i..].starts_with("{ ") {
            let before = &text[..i];
            // A function's argument, an assigned value (`e = { .. };`, not a comparison), or the
            // value of a block that holds nothing else (`if c { { .. } } else`).
            let argument = parens.last() == Some(&false) && (before.ends_with('(') || before.ends_with(", "));
            let assigned =
                before.ends_with("= ") && !["== ", "!= ", "<= ", ">= "].iter().any(|op| before.ends_with(op));
            let returned = before.ends_with("{ ");
            // A block with no braces or quotes inside, followed by the end of what it is.
            if let Some(len) = text[i + 2..].find(['{', '}', '"'])
                && text[i + 2 + len..].starts_with('}')
                && text[i + 2..i + 2 + len].ends_with(' ')
                && !text[i + 2..i + 2 + len].contains("let ")
                && !text[i + 2..i + 2 + len].contains(';')
                && match bytes.get(i + 3 + len) {
                    Some(b')' | b',') => argument,
                    Some(b';') => assigned,
                    Some(b' ') => returned && text[i + 3 + len..].starts_with(" }"),
                    _ => false,
                }
            {
                let inner = text[i + 2..i + 1 + len].trim_end();
                out.push_str(&format!("{{ let value = {inner}; value }}"));
                i += 3 + len;
                continue;
            }
        }
        let next = i + text[i..].chars().next().map_or(1, char::len_utf8);
        out.push_str(&text[i..next]);
        i = next;
    }
    out
}

fn collect_locals(stmts: &[Stmt], out: &mut Vec<String>) {
    for stmt in stmts {
        visit_stmt(stmt, &mut |_| {}, &mut |s| {
            if let Stmt::Local { name, .. } | Stmt::ForEach { name, .. } = s {
                out.push(name.clone());
            }
            if let Stmt::Try { catches, .. } = s {
                out.extend(catches.iter().map(|c| c.name.clone()));
            }
        });
    }
    // The variables of patterns.
    for stmt in stmts {
        visit_stmt(
            stmt,
            &mut |e| {
                visit_expr(e, &mut |x| {
                    if let ExprKind::InstanceOf { binding: Some(name), .. } = &x.kind {
                        out.push(name.clone());
                    }
                })
            },
            &mut |_| {},
        );
    }
}

/// The superinterfaces of class `t` that its initialization initializes, in Java's order: those
/// that declare a method with code that is not static (a default or private one), by a walk of
/// the interfaces it implements in the order of its `implements` clause, each interface after its
/// own superinterfaces (JLS 12.4.2; HotSpot counts private methods too). An interface's own
/// initialization initializes none, so `t` that is an interface has none. Only the methods that
/// the translation has count, so an interface whose default methods are all unreachable is left
/// out, which only its initializer's effects could tell.
pub(crate) fn default_superinterfaces(class: &Class, t: usize) -> Vec<usize> {
    fn walk(class: &Class, t: usize, out: &mut Vec<usize>) {
        for interface in &class.types[t].interfaces {
            let Interface::Type(i) = *interface else { continue };
            walk(class, i, out);
            let has_code = class.hierarchy.methods_of(i).iter().any(|&m| {
                let m = &class.methods[m];
                !m.is_static && !m.is_abstract && !m.is_ctor && m.name != "<clinit>"
            });
            if has_code && !out.contains(&i) {
                out.push(i);
            }
        }
    }
    let mut out = Vec::new();
    if class.types[t].kind != TypeKind::Interface {
        walk(class, t, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::scoped_arguments;

    #[test]
    fn scoped_borrows_keep_their_scope_without_unused_braces() {
        // An argument, an assigned value and the whole value of a block.
        assert_eq!(scoped_arguments("f({ a.borrow().x }, 1);"), "f({ let value = a.borrow().x; value }, 1);");
        assert_eq!(
            scoped_arguments("e = { e.borrow().next.clone() };"),
            "e = { let value = e.borrow().next.clone(); value };"
        );
        assert_eq!(
            scoped_arguments("return if c { { p.borrow().val } } else { None };"),
            "return if c { { let value = p.borrow().val; value } } else { None };"
        );
        // A comparison, a block with statements and a macro's argument stay as they are.
        for text in ["if a == { b.borrow().x } {", "e = { init(); X.get() };", "println!({ a.borrow().x });"]
        {
            assert_eq!(scoped_arguments(text), text);
        }
    }
}
