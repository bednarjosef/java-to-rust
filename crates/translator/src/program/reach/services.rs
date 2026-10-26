//! The implementations of security services that a provider makes by name: `Provider.Service`
//! has the binary name of its implementation's class, which the JDK finds and makes by
//! reflection, and the translation through `TranslatedServices.construct`, a `switch` over the
//! names with a case for each class (`lower/by_name.rs`).
//!
//! The JDK's providers register their services in code, each with a call whose string literals
//! are the service's type, its algorithm and the class (`add(p, "MessageDigest", "SHA-256",
//! "sun.security.provider.SHA2$SHA256", attrs)`, `ps("Cipher", "AES", ..)`, `new
//! Provider.Service(this, "SSLContext", ..)`): a call of reachable code with two string literals
//! or more, the first a name and a later one that of a class, registers that class for a service
//! of the first's type. A provider registers hundreds of services, and making every class
//! reachable would translate most of `javax.crypto` for a program that only digests: so a class
//! is a case of the `switch` only once reachable code makes objects of the engine class of its
//! service's type (`MessageDigest` for `"MessageDigest"`), which hold the services that it looks
//! up (the class is live, see `Walk::live`), and it and its superclasses are translated
//! (`outside`); its constructors with no parameter or one are then reachable, and it is noted in
//! `Reach::service_classes`. A class that is not translated, or extends one that is not, has no
//! case, and its service fails when it is made, as the JDK's does for a class it cannot find.
//!
//! The class names of the registrations are not names that reflection finds (`Walk::named`):
//! where reachable code creates an object from a computed class name or a `Class<?>`, as
//! `URL` does its protocol handlers, every class named by a literal would otherwise be a
//! candidate, the services of every type with them.

use super::{Item, Walk, outside, provided, strip_arguments, supertype, text};
use crate::value::Value;

/// The owner and name of the method whose calls the translator writes as the `switch`.
pub const CONSTRUCT: (&str, &str) = ("sun.security.jca.TranslatedServices", "construct");

/// The packages of the engine classes, whose objects hold the services that code looks up by type.
const ENGINE_PACKAGES: &[&str] = &[
    "java.security",
    "java.security.cert",
    "javax.crypto",
    "javax.net.ssl",
    "javax.security.auth.login",
    "javax.security.sasl",
    "javax.smartcardio",
    "javax.xml.crypto.dsig",
    "javax.xml.crypto.dsig.keyinfo",
    "org.ietf.jgss",
];

/// Whether the call `node` is the one that the translator writes as the `switch`.
pub fn constructs(node: &Value) -> bool {
    let symbol = &node["symbol"];
    text(node, "kind") == "METHOD_INVOCATION"
        && (strip_arguments(text(symbol, "owner")).as_str(), text(symbol, "name")) == CONSTRUCT
}

/// The service type and the class's binary name that the call `node` registers, if it is a
/// registration (see the module's comment).
fn registration(node: &Value) -> Option<(String, String)> {
    let literals: Vec<&str> = node["arguments"]
        .as_array()?
        .iter()
        .filter(|a| text(a, "kind") == "STRING_LITERAL")
        .filter_map(|a| a["value"].as_str())
        .collect();
    let (ty, rest) = literals.split_first()?;
    if ty.is_empty() || !ty.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let class = rest.iter().find(|l| names_class(l))?;
    Some((ty.to_string(), class.to_string()))
}

/// Whether a string could be the binary name of a class in a package: identifiers joined by dots,
/// at least two, the last of which may name a member class after a `$`.
fn names_class(text: &str) -> bool {
    let identifier = |part: &str| {
        part.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_')
            && part.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$')
    };
    text.contains('.') && text.split('.').all(identifier)
}

impl Walk<'_> {
    /// A call of reachable code, which may register a service or make one (see the module's
    /// comment).
    pub(super) fn service_scan(&mut self, node: &Value) {
        if constructs(node) {
            self.constructs_services = true;
        }
        if let Some((ty, class)) = registration(node) {
            self.registered.insert(class.clone());
            self.registrations.entry(ty).or_default().insert(class);
        }
    }

    /// Makes reachable the classes that services are made of, and whether that is anything new
    /// (see the module's comment). A class that no unit declares yet is asked for, and made
    /// reachable once a later round declares it.
    pub(super) fn make_services(&mut self) -> bool {
        if !self.constructs_services {
            return false;
        }
        let mut new = false;
        let registrations = std::mem::take(&mut self.registrations);
        for (ty, classes) in &registrations {
            let looked_up =
                ENGINE_PACKAGES.iter().any(|package| self.reach.live.contains(&format!("{package}.{ty}")));
            if !looked_up {
                continue;
            }
            for binary in classes {
                if self.reach.service_classes.contains(binary) {
                    continue;
                }
                let class = binary.replace('$', ".");
                if outside(&class) {
                    continue;
                }
                if !self.classes.contains_key(&class) {
                    let top_level = binary.split('$').next().unwrap_or(binary);
                    self.missing(top_level);
                    continue;
                }
                if !self.superclasses_translated(&class).unwrap_or(false) {
                    continue;
                }
                self.reach.service_classes.insert(binary.clone());
                let constructors: Vec<_> = (self.methods_by_name.get(&class))
                    .and_then(|methods| methods.get("<init>"))
                    .into_iter()
                    .flatten()
                    .filter(|(_, _, signature)| !signature.contains(','))
                    .cloned()
                    .collect();
                for constructor in constructors {
                    self.work.push(Item::Method(constructor));
                }
                new = true;
            }
        }
        for (ty, classes) in registrations {
            self.registrations.entry(ty).or_default().extend(classes);
        }
        new
    }

    /// Whether the superclasses of the declared class `class` are all translated, the runtime's
    /// own (`Object`) ending the chain; `None` while a unit that declares one is still to come,
    /// which is asked for.
    fn superclasses_translated(&mut self, class: &str) -> Option<bool> {
        let mut at = class.to_string();
        loop {
            let (decl, _) = self.classes.get(&at)?;
            let Some(superclass) = supertype(&decl["extendsClause"]) else { return Some(true) };
            if provided(&superclass) {
                return Some(true);
            }
            if outside(&superclass) {
                return Some(false);
            }
            if !self.classes.contains_key(&superclass) {
                self.missing(&superclass);
                return None;
            }
            at = superclass;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::json;

    fn call(literals: &[&str]) -> Value {
        let arguments: Vec<serde_json::Value> =
            literals.iter().map(|l| serde_json::json!({"kind": "STRING_LITERAL", "value": *l})).collect();
        json!({"kind": "METHOD_INVOCATION", "arguments": arguments})
    }

    #[test]
    fn a_registration_names_the_type_and_the_class() {
        let registered =
            registration(&call(&["MessageDigest", "SHA-256", "sun.security.provider.SHA2$SHA256"]));
        assert_eq!(
            registered,
            Some(("MessageDigest".to_string(), "sun.security.provider.SHA2$SHA256".to_string()))
        );
    }

    #[test]
    fn other_calls_with_literals_register_nothing() {
        // The first literal is no type name, or no later literal names a class.
        assert_eq!(registration(&call(&["Alg.Alias.MessageDigest.SHA", "SHA-1"])), None);
        assert_eq!(registration(&call(&["MessageDigest", "SHA-256"])), None);
        assert_eq!(registration(&call(&["MessageDigest", "1.2.840.113549.2.5"])), None);
    }
}
