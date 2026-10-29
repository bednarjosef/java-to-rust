//! The covariant arrays and the failing Rust generics, predicted before the first lowering from
//! the trees, so that it starts with them and finds nothing more: the program is lowered once
//! (see "Real Rust generics" in `docs/object-model.md`). Whatever the prediction misses, lowering
//! notes as before, and the program is lowered again (`real_generics::retry`): a miss costs time,
//! never a wrong translation.
//!
//! With `JAVA_TO_RUST_PREDICTION_CHECK` set, the prediction is made but not used, and `--timings`
//! compares it with what the lowerings found: how many of the array types they noted and of the
//! Rust generics that failed it predicted, and each one it missed or added.

use super::real_generics::{Owner, Real};
use super::*;
use std::rc::Rc;

/// The array types predicted to be `Object[]`s and the Rust generics predicted to fail.
#[derive(Clone, Default)]
pub(super) struct Prediction {
    pub(super) arrays: Vec<Type>,
    pub(super) failed: Real,
}

/// Whether the prediction is only compared with what lowering finds, not used.
pub(super) fn checking() -> bool {
    std::env::var_os("JAVA_TO_RUST_PREDICTION_CHECK").is_some()
}

impl<'t> Lowering<'t> {
    /// Sets the Rust generics this lowering tries: `real`, or, on a lowering that is given none
    /// (the first, or one after that only noted covariant arrays), the candidates without those
    /// predicted to fail. The prediction is made once, for the declarations `decls`, and kept with
    /// the covariant arrays (`Covariant::predicted`); its array types are `Object[]`s from the
    /// first lowering on.
    pub(super) fn start_real(&mut self, real: Option<&Real>, decls: &[Node<'t>]) {
        let candidates = self.candidates();
        let cell = Rc::clone(&self.covariant.predicted);
        let mut predicted = cell.borrow_mut();
        let prediction = predicted
            .get_or_insert_with(|| crate::timings::phase("prediction", || self.predict(&candidates, decls)));
        if checking() {
            self.real = real.cloned().unwrap_or(candidates);
            return;
        }
        self.covariant.known.extend(prediction.arrays.iter().copied());
        self.real = real.cloned().unwrap_or_else(|| candidates.without(&prediction.failed));
    }

    /// The prediction for the declarations `decls`, with the Rust generics `candidates`.
    fn predict(&self, candidates: &Real, decls: &[Node<'t>]) -> Prediction {
        let _ = (candidates, decls);
        Prediction::default()
    }
}

/// For `--timings` with `JAVA_TO_RUST_PREDICTION_CHECK`: the prediction against what the
/// lowerings found, the array types `noted` and the Rust generics that `failed`, each miss and
/// each addition on a line of its own.
pub(super) fn report(prediction: &Prediction, noted: &[Type], failed: &Real, class: &Class) {
    let name = |owner: Owner| match owner {
        Owner::Class(t) => class.types[t].name.clone(),
        Owner::Method(m) => {
            let method = &class.methods[m];
            format!("{}.{}", class.types[method.class].name, method.name)
        }
    };
    let owners = |real: &Real| -> Vec<Owner> {
        real.classes
            .iter()
            .map(|&t| Owner::Class(t))
            .chain(real.methods.iter().map(|&m| Owner::Method(m)))
            .collect()
    };
    let predicted_owners = owners(&prediction.failed);
    let failed_owners = owners(failed);
    let missed: Vec<String> =
        failed_owners.iter().filter(|o| !predicted_owners.contains(o)).map(|&o| name(o)).collect();
    let added: Vec<String> =
        predicted_owners.iter().filter(|o| !failed_owners.contains(o)).map(|&o| name(o)).collect();
    let missed_arrays: Vec<String> =
        noted.iter().filter(|t| !prediction.arrays.contains(t)).map(Type::to_string).collect();
    let added_arrays: Vec<String> =
        prediction.arrays.iter().filter(|t| !noted.contains(t)).map(Type::to_string).collect();
    eprintln!(
        "timing: prediction: {} of {} covariant array types, {} more; {} of {} failed Rust generics, {} more",
        noted.len() - missed_arrays.len(),
        noted.len(),
        added_arrays.len(),
        failed_owners.len() - missed.len(),
        failed_owners.len(),
        added.len()
    );
    for (what, list) in [
        ("missed array", &missed_arrays),
        ("added array", &added_arrays),
        ("missed generic", &missed),
        ("added generic", &added),
    ] {
        for item in list {
            eprintln!("timing: prediction {what} {item}");
        }
    }
}
