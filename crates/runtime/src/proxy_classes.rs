//! The run-time names of the translator's proxy classes. The JVM names a proxy class when it
//! defines it, `jdk.proxy1.$Proxy0`, by the number of proxy classes defined before; the
//! translation has a class of its own for each list of interfaces, which its
//! `TranslatedProxies` gives that name when it defines it
//! (`java/overlay/java/lang/reflect/TranslatedProxies.java`), and which `Class` shows in its
//! place. Java makes annotations as proxies too, which the translation does not, so a class
//! defined after an annotation was made has no name that is known: asking for it throws.

use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::Class;

/// Whether any proxy class was named, so that classes look no further when none was.
static ANY: AtomicBool = AtomicBool::new(false);

/// The proxy classes named, by binary name, each with its run-time name and the module and loader
/// that a `ClassCastException` names (`module jdk.proxy1 of loader 'app'`), or `None`, and
/// whether that module is a dynamic one.
type Named = Option<(&'static str, &'static str)>;
static NAMES: RwLock<Vec<(&'static str, Named, bool)>> = RwLock::new(Vec::new());

/// Whether an annotation was made (`note_annotation`).
static ANNOTATIONS: AtomicBool = AtomicBool::new(false);

/// `TranslatedProxies.name0(c, name, module, dynamic)`: gives the proxy class `class` its run-time
/// name, or none that is known for an empty name, its module and loader, and whether that module
/// is a dynamic one. `TranslatedProxies` passes a class of its table, never null, which comes as
/// an `Option` where the translation does not know that.
pub fn name_proxy_class(
    class: impl Into<Option<&'static Class>>,
    name: &crate::Wtf8,
    module: &crate::Wtf8,
    dynamic: bool,
) {
    let Some(class) = class.into() else { return };
    let leak = |text: &crate::Wtf8| &*Box::leak(text.to_string().into_boxed_str());
    let named = (!name.is_empty()).then(|| (leak(name), leak(module)));
    NAMES.write().unwrap_or_else(|e| e.into_inner()).push((class.binary_name(), named, dynamic));
    ANY.store(true, Ordering::Release);
}

/// `Module.dynamicProxy0(c)`: whether `class` is a proxy class defined in a dynamic module
/// (`jdk.proxy1`), of which the translation has no `Module`. The class comes as an `Option` where
/// the translation does not know that it is not null.
pub fn in_dynamic_module(class: impl Into<Option<&'static Class>>) -> bool {
    let Some(class) = class.into() else { return false };
    if !ANY.load(Ordering::Acquire) {
        return false;
    }
    let names = NAMES.read().unwrap_or_else(|e| e.into_inner());
    names.iter().any(|&(binary, _, dynamic)| dynamic && binary == class.binary_name())
}

/// The run-time name of the class whose binary name is `binary`, if it is a proxy class.
pub(crate) fn proxy_name(binary: &str) -> Option<&'static str> {
    proxy_class(binary).map(|(name, _)| name)
}

/// The run-time name and the module and loader of the class whose binary name is `binary`, if it
/// is a proxy class.
pub(crate) fn proxy_class(binary: &str) -> Option<(&'static str, &'static str)> {
    if !ANY.load(Ordering::Acquire) {
        return None;
    }
    let names = NAMES.read().unwrap_or_else(|e| e.into_inner());
    let &(_, named, _) = names.iter().find(|(b, ..)| *b == binary)?;
    match named {
        Some(named) => Some(named),
        None => crate::throw(
            "java.lang.UnsupportedOperationException",
            "the name of a proxy class defined after an annotation was made, which the translation does not know",
        ),
    }
}

/// `TranslatedAnnotations.made0()`: an annotation is made, as a proxy in Java.
pub fn note_annotation() {
    ANNOTATIONS.store(true, Ordering::Relaxed);
}

/// `TranslatedProxies.annotationsMade0()`.
pub fn annotations_made() -> bool {
    ANNOTATIONS.load(Ordering::Relaxed)
}
