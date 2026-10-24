//! Reachability: the declarations of a program that code reachable from `main` uses. It runs on
//! javac's trees, before lowering, so that what is unreachable is never lowered at all: a
//! library's unused classes and methods may use any Java feature.
//!
//! javac has resolved every name, call and `new` to its declaration (the `symbol` of a node), so
//! a method is scanned once, when it becomes reachable, and every symbol in it makes its
//! declaration reachable in turn: a worklist that runs until nothing new is found.
//!
//! `JAVA_TO_RUST_REACH_WHY=prefix,prefix` writes on standard error, for each prefix of a
//! `class.method` (`java.awt.Component.<init>`), the chain of methods, classes and fields through
//! which reachable code first reached the first method it names whose body runs, and ends the
//! translation after reachability.

use crate::value::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

mod by_name;
mod initialize;
mod reflection;
mod sequential;
mod stubs;

pub use by_name::{
    class_bound, class_for_name, computed_for_name, created_by_name, locale_data_classes, module_for_name,
};
pub use initialize::{is_ensure_initialized, literal_class};
pub use reflection::{Kind as LookupKind, Lookup, lookup};

/// A method or constructor: the qualified name of its class, its name (`<init>` for a
/// constructor) and javac's signature, such as `(int)void`.
pub type MethodKey = (String, String, String);

/// A field: the qualified name of its class and its name.
pub type FieldKey = (String, String);

/// What reachable code uses, by qualified name.
#[derive(Default, Debug)]
pub struct Reach {
    /// Classes that reachable code names, as a type or through a member, and their outer classes.
    pub classes: HashSet<String>,
    /// Classes whose static initialization runs: a static member is used or an object created.
    pub initialized: HashSet<String>,
    /// Classes whose objects are created: one of their constructors is reachable.
    pub instantiated: HashSet<String>,
    /// Classes that an instantiated class is or extends or implements, directly or not: the classes
    /// whose instance methods can run.
    pub live: HashSet<String>,
    pub methods: HashSet<MethodKey>,
    /// Instance methods of classes that are not live, among `methods`: reachable code calls them,
    /// but no object of their class exists, so the receiver of every call is null and their
    /// bodies never run. They are declared with a body that says so (`Builtin::NoObject`), and
    /// what their bodies use is not reached. Interfaces are left out, as the runtime's strings
    /// and boxes are objects of `CharSequence` and `Comparable` that no constructor shows.
    pub dormant: HashSet<MethodKey>,
    pub fields: HashSet<FieldKey>,
    /// Methods that reachable code calls but that are not among the sources: those of the
    /// classes the runtime provides (see `RUNTIME_CLASSES`), and any that are missing; and the
    /// `REPLACED_METHODS` that it calls.
    pub external: HashSet<MethodKey>,
    /// Whether no stream of the program can be parallel, so that what a pipeline's
    /// `isParallel()` guards is not reached (`sequential.rs`).
    pub sequential: bool,
    /// Whether `Arrays.sort` and `Arrays.fill` of a `String[]` reach the JDK's methods for an
    /// `Object[]` rather than being done by the runtime (`writes_string_array`): where a
    /// `String[]` is an `Object[]` (`lower::covariant`), the runtime's in-place methods would
    /// change only a copy. `translate_program` sets it when the lowering without it finds one.
    pub string_array_writes: bool,
    /// Classes that reachable code names, or makes live, that no unit declares, other than those
    /// where reachability stops (`outside`): those that the sources do not have, and those whose
    /// units javac has not given yet when it gives them as they are asked for (`javac::Session`).
    pub missing: BTreeSet<String>,
    /// Classes that are not translated but that reachable code uses as the types of variables,
    /// which are only ever null, with the instance methods that it calls on them, by name and
    /// signature (`stubs.rs`).
    pub stubs: BTreeMap<String, BTreeSet<(String, String)>>,
    /// Classes that reachable code initializes through a `Class` value, which it gives
    /// `MethodHandles.Lookup.ensureInitialized` (`initialize.rs`).
    pub ensured: BTreeSet<String>,
    /// The classes that a name in reachable code can find: those whose names its string
    /// literals spell out, and the locale data that a computed `Class.forName(module, name)`
    /// finds (`by_name.rs`).
    pub named: BTreeSet<String>,
    /// The locale data classes that a computed `Class.forName(module, name)` finds.
    pub found_by_name: BTreeSet<String>,
    /// The providers that `ServiceLoader` finds of each service that reachable code loads, in
    /// its order, each with whether a module declares it (`reflection.rs`).
    pub services: BTreeMap<String, Vec<(String, bool)>>,
    /// The members that reflection looks up by name, as (the class it looks in, the member's
    /// name), `<init>` for constructors (`reflection.rs`).
    pub reflected: BTreeSet<(String, String)>,
    /// The modules of a program with libraries, which its `java.lang.Module` answers with
    /// (`modules::contents`, `lower/modules.rs`); set after reachability.
    pub modules: Vec<super::modules::Contents>,
}

impl Reach {
    /// Whether a member of the class with qualified name `class` (a node of its `members`) is
    /// reachable and so is translated.
    pub fn member(&self, class: &str, member: &Value) -> bool {
        let name = text(member, "name");
        match text(member, "kind") {
            "METHOD" => self.methods.contains(&(class.into(), name.into(), symbol_type(member).into())),
            "VARIABLE" => self.fields.contains(&(class.into(), name.into())),
            // Initializer blocks run with the class's initialization or with every constructor.
            "BLOCK" if member["static"].as_bool() == Some(true) => self.initialized.contains(class),
            "BLOCK" => self.instantiated.contains(class),
            kind if CLASS_KINDS.contains(&kind) => self.classes.contains(&class_name(member)),
            _ => true,
        }
    }

    /// Whether a method of the class with qualified name `class` (a node of its `members`) is
    /// one of the `dormant` ones, whose body never runs.
    pub fn dormant(&self, class: &str, member: &Value) -> bool {
        let key = (class.to_string(), text(member, "name").to_string(), symbol_type(member).to_string());
        self.dormant.contains(&key)
    }
}

/// The methods of `Object` that a class may override, which code outside the program (printing,
/// string concatenation, collections) calls on its objects.
pub(super) const OBJECT_METHODS: &[(&str, &str)] =
    &[("toString", "()java.lang.String"), ("equals", "(java.lang.Object)boolean"), ("hashCode", "()int")];

/// The classes that the runtime provides and the translator maps to it, such as `String` and
/// `System`: their sources are never read, even when a library root has them, as their code
/// reaches deep into the JVM.
pub const RUNTIME_CLASSES: &[&str] = &[
    // The builders' base class, whose methods are the builders' (see `lower::strings`).
    "java.lang.AbstractStringBuilder",
    "java.lang.Boolean",
    "java.lang.Byte",
    "java.lang.Class",
    "java.lang.ClassLoader",
    // `CharSequence` and `Comparable` are the object model's (see `lower::object`), which strings
    // (and builders, and boxes) are.
    "java.lang.CharSequence",
    "java.lang.Comparable",
    "java.lang.Double",
    "java.lang.Enum",
    "java.lang.Float",
    "java.lang.Integer",
    "java.lang.Long",
    "java.lang.Math",
    "java.lang.Number",
    "java.lang.Record",
    "java.lang.Object",
    "java.lang.Runtime",
    "java.lang.Short",
    "java.lang.StrictMath",
    "java.lang.String",
    "java.lang.StringBuffer",
    "java.lang.StringBuilder",
    "java.lang.System",
    "java.lang.Thread",
    // A charset is the runtime's value (`lower::charsets`), and so are the standard ones; the
    // JDK's code of `Charset` looks charsets up among all of them.
    "java.nio.charset.Charset",
    "java.nio.charset.StandardCharsets",
    // The JDK's classes of the charsets the runtime has (`lower/charsets.rs`).
    "sun.nio.cs.ISO_8859_1",
    "sun.nio.cs.US_ASCII",
    "sun.nio.cs.UTF_16",
    "sun.nio.cs.UTF_16BE",
    "sun.nio.cs.UTF_16LE",
    "sun.nio.cs.UTF_8",
    "sun.util.logging.PlatformLogger",
];

/// Classes nested in one of the `RUNTIME_CLASSES` that are translated from the JDK's sources all
/// the same, as they are plain code over the JDK's other classes: `Runtime.Version`, a version
/// string parsed and compared (`JarFile`'s and `ModulePath`'s release), with the pattern it
/// parses with; and the decoder and encoder of each standard charset, which the translator's
/// `newDecoder` and `newEncoder` make (`CHARSET_CODERS`).
pub const TRANSLATED_NESTED: &[&str] = &[
    "java.lang.Runtime.Version",
    "java.lang.Runtime.VersionPattern",
    "sun.nio.cs.ISO_8859_1.Decoder",
    "sun.nio.cs.ISO_8859_1.Encoder",
    "sun.nio.cs.US_ASCII.Decoder",
    "sun.nio.cs.US_ASCII.Encoder",
    "sun.nio.cs.UTF_16.Decoder",
    "sun.nio.cs.UTF_16.Encoder",
    "sun.nio.cs.UTF_16BE.Decoder",
    "sun.nio.cs.UTF_16BE.Encoder",
    "sun.nio.cs.UTF_16LE.Decoder",
    "sun.nio.cs.UTF_16LE.Encoder",
    "sun.nio.cs.UTF_8.Decoder",
    "sun.nio.cs.UTF_8.Encoder",
];

/// What is not translated, by the prefix of qualified class names, with the feature it is:
/// reachability stops there, as at the runtime's classes, and the survey counts its use. Library
/// code reaches these on paths that only its data rules out, such as `TreeMap.addAll` reading
/// from a stream that is null there. Class and resource loading finds classes and data by name,
/// which is reflection in all but name: a closed-world translation knows its classes. Some is
/// translated all the same: `ResourceBundle`, whose locale data found by name are the classes of
/// the program, and `ServiceLoader`, whose providers are those the sources declare (the
/// translator's, `java/patches/java/util/ServiceLoader.java`), and `Module` and `ModuleLayer`,
/// whose modules are those of the translation (`java/patches/java/lang/Module.java`).
/// `ObjectMethods` and `SwitchBootstraps` are the bootstrap methods of `invokedynamic` for a
/// record's methods and a `switch` on patterns, which the translation makes without them; only a
/// class name that code computes, which `Class.forName` initializes (`reach::by_name`), reaches
/// them, and their static state is method handles.
pub const UNTRANSLATED: &[(&str, &str)] = &[
    ("java.io.ObjectInputStream", "serialization"),
    ("java.io.ObjectOutputStream", "serialization"),
    ("java.io.ObjectStreamClass", "serialization"),
    ("java.lang.invoke.", "reflection"),
    ("java.lang.runtime.ObjectMethods", "reflection"),
    ("java.lang.runtime.SwitchBootstraps", "reflection"),
    ("java.lang.reflect.", "reflection"),
    ("java.security.", "security"),
    // There is no security manager (`System.getSecurityManager()` is null, see `lower::system`).
    ("java.lang.SecurityManager", "security"),
    ("jdk.internal.loader.", "class loading"),
    ("jdk.internal.reflect.", "reflection"),
    ("sun.security.", "security"),
];

/// Classes under a prefix of `UNTRANSLATED` that are translated all the same:
/// `java.lang.reflect.Array` creates arrays of a component type and measures them, which is what
/// `Arrays.copyOf` and `Collection.toArray` need of it, not reflection. `Modifier` tests the bits
/// of a member's modifiers. `Method`, `Field` and `Constructor` are the translator's own, over the
/// reflection table (`reflection.rs`), and so is `InvocationTargetException`, which wraps the
/// exceptions of what they call (`java/Standard.java`). `TypeDescriptor` is the interface of
/// descriptors that `java.lang.constant`'s `ClassDesc` and `MethodTypeDesc` implement, plain
/// methods over the descriptor strings.
pub const TRANSLATED_ANYWAY: &[&str] = &[
    "java.lang.invoke.TypeDescriptor",
    "java.lang.invoke.TypeDescriptor.OfField",
    "java.lang.invoke.TypeDescriptor.OfMethod",
    // An action that `AccessController.doPrivileged` runs, which the translator's runs directly
    // (`REDIRECTED_CALLS`): a plain interface.
    "java.security.PrivilegedAction",
    // Plain interfaces that library code implements with classes of its own: abstract methods,
    // default methods over them and constants, nothing that the JVM or reflection gives. A class
    // of the program implements them as it would an interface of its own; what makes their
    // objects in the JDK (`Class` as a `Type`, a provider's keys, `Proxy` calling a handler)
    // stays untranslated, so only the program's objects ever implement them.
    "java.lang.reflect.GenericArrayType",
    "java.lang.reflect.InvocationHandler",
    "java.lang.reflect.ParameterizedType",
    "java.lang.reflect.Type",
    "java.lang.reflect.WildcardType",
    "java.security.AsymmetricKey",
    "java.security.DEREncodable",
    "java.security.Key",
    "java.security.PrivateKey",
    "java.security.PrivilegedExceptionAction",
    "java.security.PublicKey",
    "java.security.spec.AlgorithmParameterSpec",
    "java.security.spec.KeySpec",
    // The security code's debugging output, off unless a property turns it on, which library
    // code asks for as it starts (`JarVerifier.debug = Debug.getInstance("jar")`); patched not
    // to name the caller's frame (`java/patches/sun/security/util/Debug.java`).
    "sun.security.util.Debug",
    // The signers of a jar's entry, which `JarEntry` and `JarFile` keep in fields: abstract and
    // final classes, of which only signature verification, not translated, makes objects.
    "java.security.cert.Certificate",
    "java.security.CodeSigner",
    // Reading a jar's manifest, as javac does for each jar on its class path, checks its size
    // against `MAX_SIG_FILE_SIZE`, a static of this class, and `JarVerifier` asks it which
    // entries are signature files; verifying a signature (`new SignatureFileVerifier`) needs
    // `PKCS7` and stays rejected.
    "sun.security.util.SignatureFileVerifier",
    // Message digests, as javac's `-XDshowClass` computes a class file's checksum: the JDK's
    // own code of `MessageDigest` and of the `SUN` provider's digests, which
    // `MessageDigest.getInstance` makes directly (`java/patches/java/security/MessageDigest.java`,
    // `java/overlay/sun/security/provider/TranslatedDigests.java`).
    "java.security.DigestException",
    "java.security.DigestInputStream",
    "java.security.DigestOutputStream",
    "java.security.GeneralSecurityException",
    "java.security.MessageDigest",
    "java.security.MessageDigest.Delegate",
    "java.security.MessageDigest.Delegate.CloneableDelegate",
    "java.security.MessageDigestSpi",
    "java.security.NoSuchAlgorithmException",
    "java.security.ProviderException",
    "sun.security.provider.ByteArrayAccess",
    "sun.security.provider.ByteArrayAccess.BE",
    "sun.security.provider.ByteArrayAccess.LE",
    "sun.security.provider.DigestBase",
    "sun.security.provider.MD2",
    "sun.security.provider.MD5",
    "sun.security.provider.SHA",
    "sun.security.provider.SHA2",
    "sun.security.provider.SHA2.SHA224",
    "sun.security.provider.SHA2.SHA256",
    "sun.security.provider.SHA3",
    "sun.security.provider.SHA3.SHA224",
    "sun.security.provider.SHA3.SHA256",
    "sun.security.provider.SHA3.SHA384",
    "sun.security.provider.SHA3.SHA512",
    "sun.security.provider.SHA5",
    "sun.security.provider.SHA5.SHA384",
    "sun.security.provider.SHA5.SHA512",
    "sun.security.provider.SHA5.SHA512_224",
    "sun.security.provider.SHA5.SHA512_256",
    "sun.security.provider.TranslatedDigests",
    // The interface of digests that take a key, which `MessageDigest`'s own `Delegate`
    // implements.
    "sun.security.util.MessageDigestSpi2",
    "java.lang.reflect.Array",
    "java.lang.reflect.InvocationTargetException",
    "java.lang.reflect.Modifier",
    "java.lang.reflect.Method",
    "java.lang.reflect.Field",
    "java.lang.reflect.Constructor",
];

/// Methods of translated classes whose calls the lowering replaces with the runtime's code
/// (`lower/natives.rs`), so that reachability does not enter them, as with the runtime's classes.
/// They take a `Class`, which the translation has no value for: every call that `Unsafe`'s own
/// initialization makes of the first two passes a class literal of an array class, and
/// `objectFieldOffset` of a class literal and a field's name, or of the field that
/// `getDeclaredField` finds by its name, makes a field handle (`lower/handles.rs`).
pub const REPLACED_METHODS: &[(&str, &str, &str)] = &[
    ("jdk.internal.misc.Unsafe", "arrayBaseOffset", "(java.lang.Class<?>)long"),
    ("jdk.internal.misc.Unsafe", "arrayIndexScale", "(java.lang.Class<?>)int"),
    ("jdk.internal.misc.Unsafe", "objectFieldOffset", "(java.lang.Class<?>,java.lang.String)long"),
    ("jdk.internal.misc.Unsafe", "objectFieldOffset", "(java.lang.reflect.Field)long"),
    ("jdk.internal.misc.Unsafe", "staticFieldOffset", "(java.lang.reflect.Field)long"),
    ("jdk.internal.misc.Unsafe", "staticFieldBase", "(java.lang.reflect.Field)java.lang.Object"),
    ("java.util.HashMap", "comparableClassFor", "(java.lang.Object)java.lang.Class<?>"),
    ("java.util.concurrent.ConcurrentHashMap", "comparableClassFor", "(java.lang.Object)java.lang.Class<?>"),
    // A saved property is a property (`lower/system.rs`).
    ("jdk.internal.misc.VM", "getSavedProperty", "(java.lang.String)java.lang.String"),
    // Nothing is garbage collected, so a file's descriptor needs no cleaner (`lower/natives.rs`).
    ("java.io.FileCleanable", "register", "(java.io.FileDescriptor)void"),
    // Nor a direct buffer's memory: no thread waits for buffers to be collected.
    ("java.nio.BufferCleaner", "startCleaningThreadIfNeeded", "()void"),
    // No virtual thread blocks a carrier thread (`lower/natives.rs`).
    ("jdk.internal.misc.Blocker", "begin", "()boolean"),
    ("jdk.internal.misc.Blocker", "begin", "(boolean)boolean"),
    ("jdk.internal.misc.Blocker", "end", "(boolean)void"),
    // A translated program has no native library to load, and no garbage collector whose
    // collections a cleaner thread would wait for (`lower/libraries.rs`).
    (
        "jdk.internal.ref.CleanerImpl",
        "start",
        "(java.lang.ref.Cleaner,java.util.concurrent.ThreadFactory)void",
    ),
    ("jdk.internal.loader.BootLoader", "loadLibrary", "(java.lang.String)void"),
    ("java.security.SecureRandom", "getSeed", "(int)byte[]"),
    // Preview features are off in a plain run, and there is no security manager to check the
    // `jrt:` file system's callers (`lower/system.rs`).
    ("jdk.internal.misc.PreviewFeatures", "isPreviewEnabled", "()boolean"),
    ("jdk.internal.jrtfs.JrtFileSystemProvider", "checkPermission", "()void"),
];

/// A method, by class, name and javac's signature.
type Member = (&'static str, &'static str, &'static str);

/// The methods of the JDK that the translator's standard classes (`lower::exceptions`) call, by
/// the standard method that calls them: reachability does not read the standard classes' code.
/// What a new thread calls to copy its parent's inheritable thread locals is reached by
/// `setInheritableThreadLocals`, with which a program has that copy
/// (`lower/exceptions.rs`).
const STANDARD_CALLS: &[(Member, Member)] = &[
    (
        ("java.lang.Throwable", "printStackTrace", "(java.io.PrintStream)void"),
        ("java.io.PrintStream", "println", "(java.lang.String)void"),
    ),
    (
        ("java.lang.Throwable", "printStackTrace", "(java.io.PrintWriter)void"),
        ("java.io.PrintWriter", "println", "(java.lang.String)void"),
    ),
    (
        ("java.lang.Thread", "setInheritableThreadLocals", "(java.lang.ThreadLocal.ThreadLocalMap)void"),
        ("java.lang.ThreadLocal.ThreadLocalMap", "size", "()int"),
    ),
    (
        ("java.lang.Thread", "setInheritableThreadLocals", "(java.lang.ThreadLocal.ThreadLocalMap)void"),
        (
            "java.lang.ThreadLocal",
            "createInheritedMap",
            "(java.lang.ThreadLocal.ThreadLocalMap)java.lang.ThreadLocal.ThreadLocalMap",
        ),
    ),
];

/// The methods that answer which class called the method that calls them: the lowering makes the
/// answer the class of each call of that method (`lower/callers.rs`), so reachability does not
/// enter them, nor counts the one of `jdk.internal.reflect` as reflection.
pub const CALLER_QUERIES: &[(&str, &str, &str)] = &[
    ("jdk.internal.reflect.Reflection", "getCallerClass", "()java.lang.Class<?>"),
    ("java.lang.StackWalker", "getCallerClass", "()java.lang.Class<?>"),
];

/// The class of the method among `CALLER_QUERIES` with this class, name and signature, if it is one.
pub fn caller_query(class: &str, name: &str, signature: &str) -> Option<&'static str> {
    CALLER_QUERIES.iter().find(|q| **q == (class, name, signature)).map(|q| q.0)
}

/// The field of its own class that `MhUtil.findVarHandle(MethodHandles.lookup(), "name", type)`
/// makes a handle of: the lookup is of the class whose code calls it, so the field is reached
/// though no code names it (`lower/handles.rs`).
fn own_field_handle(node: &Value) -> Option<String> {
    let symbol = &node["symbol"];
    if text(node, "kind") != "METHOD_INVOCATION"
        || (text(symbol, "owner"), text(symbol, "name")) != ("jdk.internal.invoke.MhUtil", "findVarHandle")
    {
        return None;
    }
    let [_, field, _] = &node["arguments"].as_array()?[..] else { return None };
    field["value"].as_str().filter(|_| text(field, "kind") == "STRING_LITERAL").map(str::to_string)
}

/// The class and the field that `node` is the offset of, when it is a call of `Unsafe`'s
/// `objectFieldOffset` of a class literal and the field's name, or of the field that
/// `getDeclaredField` of a class literal finds by its name: the field is reached, though no code
/// names it (`lower/handles.rs`).
pub fn field_offset_named(node: &Value) -> Option<(String, String)> {
    let symbol = &node["symbol"];
    if text(node, "kind") != "METHOD_INVOCATION" {
        return None;
    }
    let args = node["arguments"].as_array()?;
    // The field of a `VarHandle` that `findVarHandle` (or `findStaticVarHandle`) of a class
    // literal makes, or `ConstantBootstraps.fieldVarHandle` (or `staticFieldVarHandle`).
    let finds = matches!(
        (text(symbol, "owner"), text(symbol, "name")),
        ("java.lang.invoke.MethodHandles.Lookup", "findVarHandle" | "findStaticVarHandle")
            | ("jdk.internal.invoke.MhUtil", "findVarHandle")
    );
    if finds && let [.., class, field, _] = &args[..] {
        let field = field["value"].as_str().filter(|_| text(field, "kind") == "STRING_LITERAL")?;
        return Some((literal_class(class)?, field.to_string()));
    }
    let bootstraps = text(symbol, "owner") == "java.lang.invoke.ConstantBootstraps"
        && matches!(text(symbol, "name"), "fieldVarHandle" | "staticFieldVarHandle");
    if bootstraps && let [_, field, _, class, _] = &args[..] {
        let field = field["value"].as_str().filter(|_| text(field, "kind") == "STRING_LITERAL")?;
        return Some((literal_class(class)?, field.to_string()));
    }
    // The field that `getDeclaredField` finds, which only a field's handle may use (`handles`):
    // `ForkJoinPool` finds `poolIds` so through a local of type `Class<ForkJoinPool>`.
    if (text(symbol, "owner"), text(symbol, "name")) == ("java.lang.Class", "getDeclaredField")
        && let [field] = &args[..]
    {
        let field = field["value"].as_str().filter(|_| text(field, "kind") == "STRING_LITERAL")?;
        return Some((class_named(&node["methodSelect"]["expression"])?, field.to_string()));
    }
    if text(symbol, "owner") != "jdk.internal.misc.Unsafe"
        || !matches!(text(symbol, "name"), "objectFieldOffset" | "staticFieldOffset")
    {
        return None;
    }
    let (class, field) = match &args[..] {
        [class, field] => (class, field),
        [get]
            if text(get, "kind") == "METHOD_INVOCATION"
                && text(&get["symbol"], "owner") == "java.lang.Class"
                && text(&get["symbol"], "name") == "getDeclaredField" =>
        {
            (&get["methodSelect"]["expression"], get["arguments"].get(0)?)
        }
        _ => return None,
    };
    let field = field["value"].as_str().filter(|_| text(field, "kind") == "STRING_LITERAL")?;
    Some((class_named(class)?, field.to_string()))
}

/// The class that the expression `node` of type `Class<C>` is: a class literal, or a value of
/// that exact type, as `ForkJoinPool`'s local `Class<ForkJoinPool> klass` that finds its fields.
fn class_named(node: &Value) -> Option<String> {
    literal_class(node).or_else(|| {
        let class = text(node, "javaType").strip_prefix("java.lang.Class<")?.strip_suffix('>')?;
        (!class.starts_with('?')).then(|| strip_arguments(class))
    })
}

/// The methods that lowering replaces whatever their signature, as `REPLACED_METHODS`: the
/// constants of an enum class, which the JDK reads through `SharedSecrets` and reflection, and
/// `PrintStream`'s `printf` and `format`, which are formatted where they are called and printed
/// with `PRINT_FORMATTED`.
const REPLACED_BY_NAME: &[(&str, &str)] = &[
    ("java.util.EnumMap", "getKeyUniverse"),
    ("java.util.EnumSet", "getUniverse"),
    ("java.io.PrintStream", "printf"),
    ("java.io.PrintStream", "format"),
];

/// The method of the translator's `PrintStream` (`java/patches/java/io/PrintStream.java`) that
/// prints what a call of `printf` or `format` formatted.
pub const PRINT_FORMATTED: (&str, &str, &str) =
    ("java.io.PrintStream", "formattedText", "(java.lang.String)java.io.PrintStream");

/// The field of the translator's classes that `System.in`, `System.out` or `System.err` reads as
/// a value (`java/overlay/java/io/StandardInput.java` and `StandardOutput.java`), by the name of
/// the field of `System`.
pub fn standard_stream_field(name: &str) -> Option<(&'static str, &'static str)> {
    match name {
        "in" => Some(("java.io.StandardInput", "in")),
        "out" => Some(("java.io.StandardOutput", "out")),
        "err" => Some(("java.io.StandardOutput", "err")),
        _ => None,
    }
}

/// The field of the translator's classes that a static field of a class it does not translate
/// reads as a value: `System.in`, `System.out` and `System.err` (`standard_stream_field`), and
/// `String.CASE_INSENSITIVE_ORDER` (`java/overlay/java/lang/TranslatedStrings.java`), as
/// (class, field).
pub fn redirected_field(owner: &str, name: &str) -> Option<(&'static str, &'static str)> {
    match (owner, name) {
        ("java.lang.System", _) => standard_stream_field(name),
        ("java.lang.String", "CASE_INSENSITIVE_ORDER") => {
            Some(("java.lang.TranslatedStrings", "CASE_INSENSITIVE_ORDER"))
        }
        _ => None,
    }
}

/// Whether the runtime makes the call of `PrintStream`'s method `name` on `System.out` or
/// `System.err` (`stream`, the field's name) itself (`lower::natives` and `lower`'s `call`), so
/// that it needs no object: printing and flushing, and `System.out.write(int)`. Any other call
/// on them goes to the translator's `PrintStream` object (`standard_stream_field`).
pub fn direct_stream_call(stream: &str, name: &str, signature: &str) -> bool {
    match stream {
        "out" => {
            matches!(name, "print" | "println" | "printf" | "format" | "flush")
                || (name == "write" && signature == "(int)void")
        }
        "err" => matches!(name, "print" | "println" | "flush"),
        _ => false,
    }
}

/// The name of the field of `System` that `node`, a call, is made on: `out` of
/// `System.out.println(x)`.
pub fn stream_of_call(node: &Value) -> Option<&str> {
    let select = &node["methodSelect"];
    let receiver = &select["expression"];
    let symbol = &receiver["symbol"];
    (text(node, "kind") == "METHOD_INVOCATION"
        && text(select, "kind") == "MEMBER_SELECT"
        && text(symbol, "kind") == "FIELD"
        && text(symbol, "owner") == "java.lang.System")
        .then(|| text(symbol, "name"))
}

/// Whether `node` is a call that the runtime makes itself on `System.out` or `System.err`
/// (`direct_stream_call`), which reaches only what its arguments do.
pub fn runtime_stream_call(node: &Value) -> bool {
    stream_of_call(node).is_some_and(|stream| {
        let symbol = &node["symbol"];
        text(symbol, "owner") == "java.io.PrintStream"
            && direct_stream_call(stream, text(symbol, "name"), text(symbol, "type"))
    })
}

/// Whether the method is one of the `REPLACED_METHODS`, an access of `Unsafe` through a field's
/// offset, which the lowering replaces with the field, or a method that makes or uses a
/// `VarHandle`.
pub fn replaced(class: &str, name: &str, signature: &str) -> bool {
    REPLACED_METHODS.contains(&(class, name, signature))
        // A method that makes a `VarHandle` makes a constant that lowering keeps as what it
        // reaches, and the handle's accesses are those of the field or element (`lower/handles.rs`).
        || signature.ends_with(")java.lang.invoke.VarHandle")
        || class == "java.lang.invoke.VarHandle"
        // A field updater constant is a handle of its field, and its accesses are the field's.
        || matches!(
            class,
            "java.util.concurrent.atomic.AtomicIntegerFieldUpdater"
                | "java.util.concurrent.atomic.AtomicLongFieldUpdater"
                | "java.util.concurrent.atomic.AtomicReferenceFieldUpdater"
        )
        || REPLACED_BY_NAME.contains(&(class, name))
        || caller_query(class, name, signature).is_some()
        || (class == "java.lang.Character" && crate::lower::runtime_character_method(name, signature))
        || unsafe_field_access(class, name, signature).is_some()
        || unsafe_field_update(class, name, signature).is_some()
}

/// One of `Unsafe`'s atomic updates of a field at an offset (see `unsafe_field_update`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldUpdate {
    /// `getAndAdd<Kind><Mode>(o, offset, delta)`: the old value, the field incremented.
    GetAndAdd,
    /// `getAndSet<Kind><Mode>(o, offset, x)`: the old value, the field set to `x`.
    GetAndSet,
    /// `compareAndSet<Kind>` and `weakCompareAndSet<Kind><Mode>(o, offset, expected, x)`: whether
    /// the field was `expected`, and then set to `x`. The weak form never fails spuriously here.
    CompareAndSet,
    /// `compareAndExchange<Kind><Mode>(o, offset, expected, x)`: the old value, the field set to
    /// `x` when it was `expected`.
    CompareAndExchange,
    /// `getAndBitwiseOr<Kind><Mode>(o, offset, mask)`, and the `And` and `Xor` forms: the old
    /// value, the field combined with `mask` by the operator (`BitOr`, `BitAnd` or `BitXor`).
    GetAndBitwise(crate::ast::BinOp),
}

/// Whether the method is one of `Unsafe`'s atomic updates of a field at an offset, in any of its
/// memory-order modes, and which: of an `int`, `long`, reference, `float` or `double` field, or
/// the compare forms and get-and-set of a `byte`, `short`, `char` or `boolean` one.
pub fn unsafe_field_update(class: &str, name: &str, signature: &str) -> Option<FieldUpdate> {
    if class != "jdk.internal.misc.Unsafe" {
        return None;
    }
    use crate::ast::BinOp;
    let forms: [(&str, FieldUpdate, &[&str]); 8] = [
        ("getAndAdd", FieldUpdate::GetAndAdd, &["", "Acquire", "Release"]),
        ("getAndBitwiseOr", FieldUpdate::GetAndBitwise(BinOp::BitOr), &["", "Acquire", "Release"]),
        ("getAndBitwiseAnd", FieldUpdate::GetAndBitwise(BinOp::BitAnd), &["", "Acquire", "Release"]),
        ("getAndBitwiseXor", FieldUpdate::GetAndBitwise(BinOp::BitXor), &["", "Acquire", "Release"]),
        ("getAndSet", FieldUpdate::GetAndSet, &["", "Acquire", "Release"]),
        ("compareAndSet", FieldUpdate::CompareAndSet, &[""]),
        ("weakCompareAndSet", FieldUpdate::CompareAndSet, &["", "Plain", "Acquire", "Release"]),
        ("compareAndExchange", FieldUpdate::CompareAndExchange, &["", "Acquire", "Release"]),
    ];
    // The `float` and `double` forms, and the compare forms and `getAndSet` of the narrower
    // integers, are written in Java over the `int` and `long` ones, at the same offset or at the
    // word that holds the field; they make the same update of the field itself.
    let kinds = [
        ("Int", "int"),
        ("Long", "long"),
        ("Reference", "java.lang.Object"),
        ("Float", "float"),
        ("Double", "double"),
        ("Byte", "byte"),
        ("Short", "short"),
        ("Char", "char"),
        ("Boolean", "boolean"),
    ];
    forms.into_iter().find_map(|(prefix, update, modes)| {
        let rest = name.strip_prefix(prefix)?;
        let (kind, ty) = kinds.into_iter().find(|(k, _)| rest.starts_with(k))?;
        let supported = match update {
            FieldUpdate::GetAndAdd => matches!(kind, "Int" | "Long" | "Float" | "Double"),
            FieldUpdate::GetAndBitwise(_) => matches!(kind, "Int" | "Long"),
            _ => true,
        };
        if !modes.contains(&&rest[kind.len()..]) || !supported {
            return None;
        }
        let expected = match update {
            FieldUpdate::GetAndAdd | FieldUpdate::GetAndSet | FieldUpdate::GetAndBitwise(_) => {
                format!("(java.lang.Object,long,{ty}){ty}")
            }
            FieldUpdate::CompareAndSet => format!("(java.lang.Object,long,{ty},{ty})boolean"),
            FieldUpdate::CompareAndExchange => format!("(java.lang.Object,long,{ty},{ty}){ty}"),
        };
        (signature == expected).then_some(update)
    })
}

/// Whether the method is one of `Unsafe`'s reads or writes of a field at an offset:
/// `get<Kind><Mode>(Object, long)` (`Some(false)`) or `put<Kind><Mode>(Object, long, value)`
/// (`Some(true)`), of a primitive kind or `Reference`, plain, `Volatile`, `Acquire`, `Release` or
/// `Opaque`.
pub fn unsafe_field_access(class: &str, name: &str, signature: &str) -> Option<bool> {
    if class != "jdk.internal.misc.Unsafe" {
        return None;
    }
    let (put, rest) = match (name.strip_prefix("get"), name.strip_prefix("put")) {
        (Some(rest), _) => (false, rest),
        (_, Some(rest)) => (true, rest),
        _ => return None,
    };
    let kinds = [
        ("Reference", "java.lang.Object"),
        ("Boolean", "boolean"),
        ("Byte", "byte"),
        ("Short", "short"),
        ("Char", "char"),
        ("Int", "int"),
        ("Long", "long"),
        ("Float", "float"),
        ("Double", "double"),
    ];
    let (kind, ty) = kinds.into_iter().find(|(k, _)| rest.starts_with(k))?;
    let mode = &rest[kind.len()..];
    let modes: &[&str] =
        if put { &["", "Volatile", "Release", "Opaque"] } else { &["", "Volatile", "Acquire", "Opaque"] };
    let expected = if put {
        format!("(java.lang.Object,long,{ty})void")
    } else {
        format!("(java.lang.Object,long){ty}")
    };
    (modes.contains(&mode) && signature == expected).then_some(put)
}

/// Static fields that are `false` in every translated program, so the code they guard never runs.
/// The `jfrTracing` flags are set only by Java Flight Recorder, through reflection when it starts
/// (`jdk.internal.event.JFRTracing`), and guard code that records events through
/// `jdk.internal.event`. The streams' `Tripwire.ENABLED` is `Boolean.getBoolean` of a system
/// property that only `-D` on the JVM's command line sets, which a translated program has no
/// equivalent of; it guards warnings through `PlatformLogger` that name a class by reflection.
/// `ThreadLocal.TRACE_VTHREAD_LOCALS` is the property `jdk.traceVirtualThreadLocals`, which is
/// unset (`UNSET_PROPERTIES`); it guards a `StackWalker` dump of where a virtual thread sets one.
/// `ResourceBundle.TRACE_ON` is `Boolean.getBoolean("resource.bundle.debug")`, a `-D` property as
/// `Tripwire`'s; it guards the tracing of how a bundle is found, which prints modules and classes.
pub const FALSE_FLAGS: &[(&str, &str)] = &[
    ("java.io.FileInputStream", "jfrTracing"),
    ("java.lang.ThreadLocal", "TRACE_VTHREAD_LOCALS"),
    ("java.io.FileOutputStream", "jfrTracing"),
    ("java.io.RandomAccessFile", "jfrTracing"),
    ("java.lang.Throwable", "jfrTracing"),
    ("java.util.ResourceBundle", "TRACE_ON"),
    ("java.util.Tripwire", "ENABLED"),
    ("java.util.stream.Tripwire", "ENABLED"),
    ("sun.nio.ch.FileChannelImpl", "jfrTracing"),
];

/// Static methods without parameters that return `false` in every translated program, like the
/// `FALSE_FLAGS`. `ContinuationSupport.isSupported()` returns what the native `isSupported0`
/// gave, which the runtime answers false (no virtual threads, as with `-XX:-VMContinuations`);
/// it guards the pinning of the current continuation (`Continuation.pin`) that
/// `ReferenceQueue.poll` and other code under a lock do.
pub const FALSE_CALLS: &[(&str, &str)] = &[("jdk.internal.vm.ContinuationSupport", "isSupported")];

/// Static methods without parameters that return `true` in every translated program, as the
/// `FALSE_CALLS` return false: a translation is headless, as if run with
/// `-Djava.awt.headless=true` (`lower::system::HEADLESS`), since AWT's natives are not translated.
/// `GraphicsEnvironment.isHeadless()` and the private `getHeadlessProperty()` behind it read that
/// property, so the branches that would open a window, find a screen or start AWT's toolkit
/// never run, and their code (Swing, `java.awt.Component`, the X11 toolkit) is not reached.
pub const TRUE_CALLS: &[(&str, &str)] = &[
    ("java.awt.GraphicsEnvironment", "isHeadless"),
    ("java.awt.GraphicsEnvironment", "getHeadlessProperty"),
];

/// Instance methods without parameters that return `false` in every translated program, called
/// on a receiver that reading has no effects: there are no virtual threads, so
/// `Thread.isVirtual()` is false, and the branches that would park, unpark or ask a virtual
/// thread (through the JVM's `JavaLangAccess`) never run. The receiver is not read, which differs
/// from Java only where it is null, where Java would throw a `NullPointerException`; the JDK
/// tests it first (`LockSupport.unpark`) or has the current thread (`Thread.currentThread()`,
/// which is never null).
pub const FALSE_INSTANCE_CALLS: &[(&str, &str)] = &[("java.lang.Thread", "isVirtual")];

/// Static fields that only the class data sharing archive sets, through the native
/// `CDS.initializeFromArchive` at the start of their class's static initializer, which then tests
/// each against null once to build the value itself when the archive had none. The runtime's
/// `initializeFromArchive` sets nothing, so that test always finds null, and the branch that
/// would use the archived value never runs.
pub const ARCHIVED_FIELDS: &[(&str, &str)] = &[
    ("java.lang.module.Configuration", "EMPTY_CONFIGURATION"),
    ("java.util.ImmutableCollections", "archivedObjects"),
    ("java.util.jar.Attributes.Name", "KNOWN_NAMES"),
];

/// Whether `condition` tests one of the `ARCHIVED_FIELDS` against null with `==` (`equal`) or
/// `!=`.
fn archived_null_test(condition: &Value, equal: bool) -> bool {
    let kind = if equal { "EQUAL_TO" } else { "NOT_EQUAL_TO" };
    let archived = |v: &Value| {
        let symbol = &v["symbol"];
        text(symbol, "kind") == "FIELD"
            && ARCHIVED_FIELDS.contains(&(text(symbol, "owner"), text(symbol, "name")))
    };
    let null = |v: &Value| text(v, "kind") == "NULL_LITERAL";
    let (left, right) = (&condition["leftOperand"], &condition["rightOperand"]);
    text(condition, "kind") == kind && (archived(left) && null(right) || null(left) && archived(right))
}

/// The system properties that only the command line sets (`-D`), never the JVM: unset in a
/// translated program, as in a program run without options (`lower/system.rs`). `Arrays.sort`
/// reads the first to choose its old merge sort, and `ThreadLocalRandom` the second to seed
/// itself from `SecureRandom`.
pub const UNSET_PROPERTIES: &[&str] = &[
    "java.util.Arrays.useLegacyMergeSort",
    "java.util.secureRandomSeed",
    // The common `ForkJoinPool`'s settings, and whether every thread is tracked for thread dumps.
    "java.util.concurrent.ForkJoinPool.common.parallelism",
    "java.util.concurrent.ForkJoinPool.common.threadFactory",
    "java.util.concurrent.ForkJoinPool.common.exceptionHandler",
    "java.util.concurrent.ForkJoinPool.common.maximumSpares",
    "jdk.trackAllThreads",
    // Whether `ThreadLocal` prints where a virtual thread sets one.
    "jdk.traceVirtualThreadLocals",
];

/// The `if` statements among the statements of `block` whose `then` branch never runs: their
/// condition is `Boolean.parseBoolean(v)` or `v != null` (or `||` of such tests, as the common
/// `ForkJoinPool` tests its thread factory's and handler's names) of a local `v` that the block
/// declares earlier with the
/// value of one of the `UNSET_PROPERTIES` (`System.getProperty` or `VM.getSavedProperty` of it,
/// which is null) and never assigns, as `ThreadLocalRandom`'s static block tests
/// `java.util.secureRandomSeed` before it reaches `SecureRandom`.
pub fn unset_property_ifs(block: &Value) -> Vec<&Value> {
    let Some(statements) = block["statements"].as_array() else { return Vec::new() };
    let unset_value = |init: &Value| {
        let symbol = &init["symbol"];
        let read = matches!(
            (text(symbol, "owner"), text(symbol, "name")),
            ("java.lang.System", "getProperty") | ("jdk.internal.misc.VM", "getSavedProperty")
        );
        let key = &init["arguments"][0];
        text(init, "kind") == "METHOD_INVOCATION"
            && read
            && text(symbol, "type") == "(java.lang.String)java.lang.String"
            && text(key, "kind") == "STRING_LITERAL"
            && UNSET_PROPERTIES.contains(&text(key, "value"))
    };
    let mut unset: Vec<&str> = statements
        .iter()
        .filter(|s| text(s, "kind") == "VARIABLE" && unset_value(&s["initializer"]))
        .map(|s| text(s, "name"))
        .collect();
    if unset.is_empty() {
        return Vec::new();
    }
    fn assigned<'v>(v: &'v Value, out: &mut Vec<&'v str>) {
        match v {
            Value::Object(map) => {
                let kind = text(v, "kind");
                if kind == "ASSIGNMENT" || kind.ends_with("_ASSIGNMENT") {
                    out.push(text(&v["variable"], "name"));
                }
                map.values().for_each(|c| assigned(c, out));
            }
            Value::Array(items) => items.iter().for_each(|i| assigned(i, out)),
            _ => {}
        }
    }
    let mut written = Vec::new();
    assigned(block, &mut written);
    unset.retain(|name| !written.contains(name));
    let is_unset = |v: &Value| text(v, "kind") == "IDENTIFIER" && unset.contains(&text(v, "name"));
    // `Boolean.parseBoolean(v)`, `v != null`, or either of two such conditions.
    fn never<'v>(condition: &'v Value, is_unset: &dyn Fn(&'v Value) -> bool) -> bool {
        let symbol = &condition["symbol"];
        let (left, right) = (&condition["leftOperand"], &condition["rightOperand"]);
        let null = |v: &Value| text(v, "kind") == "NULL_LITERAL";
        match text(condition, "kind") {
            "PARENTHESIZED" => never(&condition["expression"], is_unset),
            "CONDITIONAL_OR" => never(left, is_unset) && never(right, is_unset),
            "NOT_EQUAL_TO" => (is_unset(left) && null(right)) || (null(left) && is_unset(right)),
            "METHOD_INVOCATION" => {
                (text(symbol, "owner"), text(symbol, "name")) == ("java.lang.Boolean", "parseBoolean")
                    && is_unset(&condition["arguments"][0])
            }
            _ => false,
        }
    }
    statements.iter().filter(|s| text(s, "kind") == "IF" && never(&s["condition"], &is_unset)).collect()
}

/// The variable whose exception the `try` statement `node` replaces with a new one of the same
/// class, as `ForkJoinTask.getException` does by reflection for an exception that another thread
/// threw: it finds the public constructors of `ex.getClass()` (two `Constructor` locals, a loop
/// over `getConstructors()`, then an `if` that calls the one of a `Throwable`, else the one
/// without parameters and `initCause`). `lower::recreate` writes it without reflection.
pub fn recreated_exception(node: &Value) -> Option<&str> {
    if text(node, "kind") != "TRY"
        || node["resources"].as_array().is_some_and(|r| !r.is_empty())
        || !node["finallyBlock"].is_null()
    {
        return None;
    }
    let statements = node["block"]["statements"].as_array()?;
    let kinds: Vec<&str> = statements.iter().map(|s| text(s, "kind")).collect();
    if kinds != ["VARIABLE", "VARIABLE", "ENHANCED_FOR_LOOP", "IF"]
        || !statements[..2]
            .iter()
            .all(|v| text(&v["symbol"], "type").starts_with("java.lang.reflect.Constructor"))
    {
        return None;
    }
    let all = &statements[2]["expression"];
    let get_class = &all["methodSelect"]["expression"];
    let exception = &get_class["methodSelect"]["expression"];
    let called = |call: &Value, owner: &str, name: &str| {
        text(call, "kind") == "METHOD_INVOCATION"
            && (text(&call["symbol"], "owner"), text(&call["symbol"], "name")) == (owner, name)
    };
    (called(all, "java.lang.Class", "getConstructors")
        && called(get_class, "java.lang.Object", "getClass")
        && text(exception, "kind") == "IDENTIFIER"
        && text(exception, "javaType") == "java.lang.Throwable")
        .then(|| text(exception, "name"))
}

/// The only branch of an `if` statement or a `?:` that can run, when its condition is never true
/// (`never_true`: the `else` branch, `Null` for an `if` without one) or always true (a test that
/// one of the `ARCHIVED_FIELDS` is null, or a call of one of the `TRUE_CALLS`: the `then` branch).
pub fn live_branch(node: &Value, sequential: bool) -> Option<&Value> {
    if !matches!(text(node, "kind"), "IF" | "CONDITIONAL_EXPRESSION") {
        return None;
    }
    let condition = &node["condition"];
    let condition =
        if text(condition, "kind") == "PARENTHESIZED" { &condition["expression"] } else { condition };
    if never_true(condition, sequential) {
        return Some(else_branch(node));
    }
    (archived_null_test(condition, true) || stub_null_test(condition, true) || always_true(condition))
        .then(|| then_branch(node))
}

/// Whether `node` is `Runtime.version().feature()`, the release of the JDK, which the translation
/// knows (`lower::system`).
pub fn release_query(node: &Value) -> bool {
    let symbol = &node["symbol"];
    let receiver = &node["methodSelect"]["expression"];
    text(node, "kind") == "METHOD_INVOCATION"
        && (text(symbol, "owner"), text(symbol, "name"), text(symbol, "type"))
            == ("java.lang.Runtime.Version", "feature", "()int")
        && text(receiver, "kind") == "METHOD_INVOCATION"
        && (text(&receiver["symbol"], "owner"), text(&receiver["symbol"], "name"))
            == ("java.lang.Runtime", "version")
}

/// Whether `condition` is `x == null` (`equal`) or `x != null` of a value `x` of a stub's type
/// (`stubs`), which is only ever null, and which it reads without effects: a variable, or
/// `System.getSecurityManager()`. So `if (sm != null)` skips what a security manager would check.
fn stub_null_test(condition: &Value, equal: bool) -> bool {
    let kind = if equal { "EQUAL_TO" } else { "NOT_EQUAL_TO" };
    let only_null = |v: &Value| {
        let read = match text(v, "kind") {
            "IDENTIFIER" => text(&v["symbol"], "kind") != "METHOD",
            "METHOD_INVOCATION" => {
                (text(&v["symbol"], "owner"), text(&v["symbol"], "name"))
                    == ("java.lang.System", "getSecurityManager")
            }
            _ => false,
        };
        read && stubs::is_stub(text(v, "javaType"))
    };
    let null = |v: &Value| text(v, "kind") == "NULL_LITERAL";
    let (left, right) = (&condition["leftOperand"], &condition["rightOperand"]);
    text(condition, "kind") == kind && (only_null(left) && null(right) || null(left) && only_null(right))
}

/// Whether the condition of an `if` statement or a `?:` is never true, so that only its `else`
/// branch can run: it reads one of the `FALSE_FLAGS` or a constant variable that is `false` by its
/// simple name (whose branch javac writes no code for), or calls one of the `FALSE_CALLS` or
/// `FALSE_INSTANCE_CALLS`, or is `!` of a call of one of the `TRUE_CALLS`, or, when the program's streams are all
/// `sequential` (see `sequential.rs`), it is a pipeline's `isParallel()` or a test whether one
/// of its nodes has children (`getChildCount() > 0`); alone, or as the first
/// operand of `&&`, or as the second after one that only computes a value (the lowering drops the
/// whole condition).
pub fn never_true(condition: &Value, sequential: bool) -> bool {
    match text(condition, "kind") {
        "PARENTHESIZED" => never_true(&condition["expression"], sequential),
        "CONDITIONAL_AND" => {
            let (left, right) = (&condition["leftOperand"], &condition["rightOperand"]);
            never_true(left, sequential) || (never_true(right, sequential) && !has_effects(left))
        }
        "IDENTIFIER" | "MEMBER_SELECT" => {
            let symbol = &condition["symbol"];
            // A constant variable that is `false`, named by its simple name: javac writes no
            // code for the branch (`private final boolean debug = false;`).
            let false_constant =
                text(condition, "kind") == "IDENTIFIER" && text(symbol, "constant") == "false";
            text(symbol, "kind") == "FIELD"
                && (false_constant || FALSE_FLAGS.contains(&(text(symbol, "owner"), text(symbol, "name"))))
        }
        "METHOD_INVOCATION" => {
            let symbol = &condition["symbol"];
            let method = (text(symbol, "owner"), text(symbol, "name"));
            let receiver = &condition["methodSelect"]["expression"];
            let false_call = (FALSE_CALLS.contains(&method)
                || FALSE_INSTANCE_CALLS.contains(&method)
                    && receiver.is_object()
                    && !reading_has_effects(receiver))
                && text(symbol, "type") == "()boolean";
            false_call || (sequential && sequential::is_parallel_test(condition))
        }
        "LOGICAL_COMPLEMENT" => always_true(&condition["expression"]),
        "NOT_EQUAL_TO" => archived_null_test(condition, false) || stub_null_test(condition, false),
        "GREATER_THAN" => sequential && sequential::has_children_test(condition),
        _ => false,
    }
}

/// Whether `condition` is a call of one of the `TRUE_CALLS`, maybe in parentheses.
fn always_true(condition: &Value) -> bool {
    let symbol = &condition["symbol"];
    match text(condition, "kind") {
        "PARENTHESIZED" => always_true(&condition["expression"]),
        "METHOD_INVOCATION" => {
            TRUE_CALLS.contains(&(text(symbol, "owner"), text(symbol, "name")))
                && text(symbol, "type") == "()boolean"
        }
        _ => false,
    }
}

/// Whether reading the receiver `value` of one of the `FALSE_INSTANCE_CALLS` has effects:
/// `Thread.currentThread()` has none (`LockSupport.park` asks it whether it is virtual).
fn reading_has_effects(value: &Value) -> bool {
    let symbol = &value["symbol"];
    let current_thread = text(value, "kind") == "METHOD_INVOCATION"
        && (text(symbol, "owner"), text(symbol, "name"), text(symbol, "type"))
            == ("java.lang.Thread", "currentThread", "()java.lang.Thread");
    !current_thread && has_effects(value)
}

/// The branch of an `if` statement or a `?:` that can run when its condition is never true.
fn else_branch(node: &Value) -> &Value {
    &node[if text(node, "kind") == "IF" { "elseStatement" } else { "falseExpression" }]
}

/// The branch of an `if` statement or a `?:` that runs when its condition is true.
fn then_branch(node: &Value) -> &Value {
    &node[if text(node, "kind") == "IF" { "thenStatement" } else { "trueExpression" }]
}

/// The feature that the class with qualified name `class` belongs to, if it is not translated.
pub fn untranslated(class: &str) -> Option<&'static str> {
    if TRANSLATED_ANYWAY.contains(&class) {
        return None;
    }
    UNTRANSLATED.iter().find(|(prefix, _)| class.starts_with(prefix)).map(|&(_, feature)| feature)
}

/// Whether reachability stops at the class with qualified name `class`: the runtime provides it,
/// or it is not translated.
pub fn outside(class: &str) -> bool {
    provided(class) || untranslated(class).is_some()
}

/// Whether the translator provides the class with qualified name `class`: one of the runtime's
/// classes, or a standard exception class (see `lower::exceptions`).
pub fn provided(class: &str) -> bool {
    RUNTIME_CLASSES.contains(&class) || crate::lower::is_standard(class)
}

/// The array `a` and the length `n` of `Array.newInstance(a.getClass().getComponentType(), n)`,
/// the new array of `a`'s own class that the JDK's collections make in `toArray(T[])` and its
/// sorts make for their work (see `lower::arrays::component_array`): it reaches what `a` and `n`
/// do, and not `java.lang.reflect.Array` or `Class`.
pub fn component_array(node: &Value) -> Option<(&Value, &Value)> {
    let call = |v: &Value, owner: &str, name: &str| {
        text(v, "kind") == "METHOD_INVOCATION"
            && text(&v["symbol"], "owner") == owner
            && text(&v["symbol"], "name") == name
            && v["methodSelect"]["kind"] == "MEMBER_SELECT"
    };
    if !call(node, "java.lang.reflect.Array", "newInstance") {
        return None;
    }
    let [component, length] = &node["arguments"].as_array()?[..] else { return None };
    let class = &component["methodSelect"]["expression"];
    if !call(component, "java.lang.Class", "getComponentType") || !call(class, "java.lang.Object", "getClass")
    {
        return None;
    }
    let array = &class["methodSelect"]["expression"];
    text(array, "javaType").ends_with("[]").then_some((array, length))
}

/// Whether the node is a call of `java.util.Arrays` that the runtime does rather than the JDK's
/// source, such as `Arrays.sort` of an `int[]`.
pub fn runtime_arrays_call(node: &Value) -> bool {
    let symbol = &node["symbol"];
    text(node, "kind") == "METHOD_INVOCATION" && text(symbol, "owner") == "java.util.Arrays" && {
        let args: Vec<&str> =
            node["arguments"].as_array().into_iter().flatten().map(|a| text(a, "javaType")).collect();
        crate::lower::runtime_arrays_call(text(symbol, "name"), &args)
    }
}

/// Whether the node is an `Arrays.sort` or `Arrays.fill` of a `String[]`, which changes the array
/// in place (see `Reach::string_array_writes`).
pub fn writes_string_array(node: &Value) -> bool {
    let symbol = &node["symbol"];
    text(node, "kind") == "METHOD_INVOCATION"
        && text(symbol, "owner") == "java.util.Arrays"
        && matches!(text(symbol, "name"), "sort" | "fill")
        && node["arguments"]
            .as_array()
            .and_then(|a| a.first())
            .is_some_and(|a| text(a, "javaType") == "java.lang.String[]")
}

/// The translator's `System.getProperties()` (`java/overlay/java/lang/TranslatedProperties.java`,
/// see `lower::system`).
pub const TRANSLATED_PROPERTIES: JdkMethod =
    ("java.lang.TranslatedProperties", "all", "()java.util.Properties");

/// The translator's methods that make an annotation that `getAnnotation` finds and read its
/// members (`java/overlay/sun/reflect/annotation/TranslatedAnnotations.java`, see
/// `lower::annotation_values`).
pub const ANNOTATION_VALUES: [JdkMethod; 2] = [
    (
        "sun.reflect.annotation.TranslatedAnnotations",
        "annotation",
        "(java.lang.Class<? extends java.lang.annotation.Annotation>,java.lang.Object[],java.lang.Object[])java.lang.annotation.Annotation",
    ),
    (
        "sun.reflect.annotation.TranslatedAnnotations",
        "member",
        "(java.lang.annotation.Annotation,java.lang.String)java.lang.Object",
    ),
];

/// The methods of `JavaLangAccess` that are the translator's static methods of the same name and
/// type in `TRANSLATED_LANG_ACCESS_CLASS` (`java/overlay/jdk/internal/access/`), as
/// `SharedSecrets.getJavaLangAccess()` is null in a translated program (see `lower.rs`).
pub const TRANSLATED_LANG_ACCESS: &[&str] = &[
    "countPositives",
    "countNonZeroAscii",
    "uncheckedNewStringWithLatin1Bytes",
    "inflateBytesToChars",
    "start",
    "uncheckedNewStringOrThrow",
    "uncheckedGetBytesOrThrow",
    "getBytesUTF8OrThrow",
    "uncheckedGetUTF16Char",
    "uncheckedPutCharUTF16",
    "decodeASCII",
    "uncheckedEncodeASCII",
    "concat",
    "blockedOn",
];

/// The class of `TRANSLATED_LANG_ACCESS`.
pub const TRANSLATED_LANG_ACCESS_CLASS: &str = "jdk.internal.access.TranslatedLangAccess";

/// The translator's `newDecoder` and `newEncoder` of a charset, by the name of the charset's
/// method: a charset is the runtime's value (`lower::charsets`), whose coders are the JDK's,
/// made by `java/overlay/sun/nio/cs/TranslatedCoders.java` (see `TRANSLATED_NESTED`).
pub const CHARSET_CODERS: &[(&str, JdkMethod)] = &[
    (
        "newDecoder",
        (
            "sun.nio.cs.TranslatedCoders",
            "newDecoder",
            "(java.nio.charset.Charset)java.nio.charset.CharsetDecoder",
        ),
    ),
    (
        "newEncoder",
        (
            "sun.nio.cs.TranslatedCoders",
            "newEncoder",
            "(java.nio.charset.Charset)java.nio.charset.CharsetEncoder",
        ),
    ),
];

/// The translator's method that makes what the charset method `name` of type `signature` of
/// the class `owner` (`Charset` or a standard charset's) makes (`CHARSET_CODERS`).
pub fn charset_coder(owner: &str, name: &str, signature: &str) -> Option<JdkMethod> {
    let charset = owner == "java.nio.charset.Charset" || owner.starts_with("sun.nio.cs.");
    let coder = CHARSET_CODERS.iter().find(|(method, _)| *method == name).map(|&(_, m)| m)?;
    (charset && signature.starts_with("()")).then_some(coder)
}

/// The methods of a charset that the JDK does by a coder with the charset's own methods and the
/// arguments, by the method's name and type: `Charset.decode(ByteBuffer)` and the `encode`s
/// replace malformed input, which the translator's `TranslatedCoders` does with the charset's
/// coder (`CHARSET_CODERS`), the charset being its first argument.
pub const CHARSET_CODINGS: &[((&str, &str), JdkMethod)] = &[
    (
        ("decode", "(java.nio.ByteBuffer)java.nio.CharBuffer"),
        (
            "sun.nio.cs.TranslatedCoders",
            "decode",
            "(java.nio.charset.Charset,java.nio.ByteBuffer)java.nio.CharBuffer",
        ),
    ),
    (
        ("encode", "(java.nio.CharBuffer)java.nio.ByteBuffer"),
        (
            "sun.nio.cs.TranslatedCoders",
            "encode",
            "(java.nio.charset.Charset,java.nio.CharBuffer)java.nio.ByteBuffer",
        ),
    ),
    (
        ("encode", "(java.lang.String)java.nio.ByteBuffer"),
        (
            "sun.nio.cs.TranslatedCoders",
            "encode",
            "(java.nio.charset.Charset,java.lang.String)java.nio.ByteBuffer",
        ),
    ),
];

/// The translator's method that does what the charset method `name` of type `signature` of the
/// class `owner` does (`CHARSET_CODINGS`).
pub fn charset_coding(owner: &str, name: &str, signature: &str) -> Option<JdkMethod> {
    let charset = owner == "java.nio.charset.Charset" || owner.starts_with("sun.nio.cs.");
    let coding = CHARSET_CODINGS.iter().find(|((method, ty), _)| *method == name && *ty == signature)?;
    charset.then_some(coding.1)
}

/// Static methods of the JDK whose calls are calls of a static method of the translator's
/// (`java/overlay`) with the same type, by the JDK's method and the translator's class and name
/// (see `lower.rs`). Reachability goes to the translator's method, and not into the JDK's.
pub const REDIRECTED_CALLS: &[(JdkMethod, (&str, &str))] = &[
    // The version of the JDK, which the JVM makes from the numbers of its build; the
    // translator's parses `java.runtime.version`, which a plain run of that JDK has
    // (`java/lang/TranslatedRuntime.java`).
    (
        ("java.lang.Runtime", "version", "()java.lang.Runtime.Version"),
        ("java.lang.TranslatedRuntime", "version"),
    ),
    // An action run with the caller's privileges, which, with no security manager, is running it
    // (`java/lang/TranslatedAccessController.java`).
    (
        ("java.security.AccessController", "doPrivileged", "<T>(java.security.PrivilegedAction<T>)T"),
        ("java.lang.TranslatedAccessController", "doPrivileged"),
    ),
    // The default file system, whose provider a property could name and reflection create (the
    // property is not set in a translated program, see `lower::system`), and which the JDK finds
    // by the URI `file:///` (`sun/nio/fs/TranslatedFiles.java`).
    (
        (
            "java.nio.file.FileSystems.DefaultFileSystemHolder",
            "defaultFileSystem",
            "()java.nio.file.FileSystem",
        ),
        ("sun.nio.fs.TranslatedFiles", "defaultFileSystem"),
    ),
    // The first mismatch of two arrays, which the JDK finds reading memory at offsets from the
    // arrays' bases; the translator's leaves every element to its callers' loops
    // (`jdk/internal/util/TranslatedArraysSupport.java`).
    (
        (
            "jdk.internal.util.ArraysSupport",
            "vectorizedMismatch",
            "(java.lang.Object,long,java.lang.Object,long,int,int)int",
        ),
        ("jdk.internal.util.TranslatedArraysSupport", "vectorizedMismatch"),
    ),
    // A file's bytes, which the JDK reads through a channel and `ByteBuffer`s; the translator's
    // read `Files.newInputStream` (`sun/nio/fs/TranslatedFiles.java`).
    (
        ("java.nio.file.Files", "readAllBytes", "(java.nio.file.Path)byte[]"),
        ("sun.nio.fs.TranslatedFiles", "readAllBytes"),
    ),
    // A file's text, which the JDK copies the bytes of for a `Path` of another module, found by
    // `Class.getModule`.
    (
        (
            "java.nio.file.Files",
            "readString",
            "(java.nio.file.Path,java.nio.charset.Charset)java.lang.String",
        ),
        ("sun.nio.fs.TranslatedFiles", "readString"),
    ),
    (
        (
            "java.nio.file.Files",
            "writeString",
            "(java.nio.file.Path,java.lang.CharSequence,java.nio.charset.Charset,java.nio.file.OpenOption[])java.nio.file.Path",
        ),
        ("sun.nio.fs.TranslatedFiles", "writeString"),
    ),
    // A reader and a writer of a file, which the JDK makes with a `CharsetDecoder` and a
    // `CharsetEncoder`; the translator's with the `Charset`.
    (
        (
            "java.nio.file.Files",
            "newBufferedReader",
            "(java.nio.file.Path,java.nio.charset.Charset)java.io.BufferedReader",
        ),
        ("sun.nio.fs.TranslatedFiles", "newBufferedReader"),
    ),
    (
        (
            "java.nio.file.Files",
            "newBufferedWriter",
            "(java.nio.file.Path,java.nio.charset.Charset,java.nio.file.OpenOption[])java.io.BufferedWriter",
        ),
        ("sun.nio.fs.TranslatedFiles", "newBufferedWriter"),
    ),
    // A number parsed from a range of a `CharSequence`, which the JDK reads with `length` and
    // `charAt` (`java/lang/TranslatedNumbers.java`); a string's range the runtime parses instead
    // (`lower.rs`).
    (
        ("java.lang.Integer", "parseInt", "(java.lang.CharSequence,int,int,int)int"),
        ("java.lang.TranslatedNumbers", "parseInt"),
    ),
    (
        ("java.lang.Long", "parseLong", "(java.lang.CharSequence,int,int,int)long"),
        ("java.lang.TranslatedNumbers", "parseLong"),
    ),
    (
        ("java.lang.Integer", "parseUnsignedInt", "(java.lang.CharSequence,int,int,int)int"),
        ("java.lang.TranslatedNumbers", "parseUnsignedInt"),
    ),
    (
        ("java.lang.Long", "parseUnsignedLong", "(java.lang.CharSequence,int,int,int)long"),
        ("java.lang.TranslatedNumbers", "parseUnsignedLong"),
    ),
    // The boot loader's unnamed module, classes and resources, and the JDK's class loaders, as
    // the translation's modules have them (`java/lang/TranslatedModules.java`).
    (
        ("jdk.internal.loader.BootLoader", "getUnnamedModule", "()java.lang.Module"),
        (TRANSLATED_MODULES, "bootUnnamedModule"),
    ),
    (
        ("jdk.internal.loader.BootLoader", "loadClassOrNull", "(java.lang.String)java.lang.Class<?>"),
        (TRANSLATED_MODULES, "bootClassOrNull"),
    ),
    (
        ("jdk.internal.loader.BootLoader", "findResource", "(java.lang.String)java.net.URL"),
        (TRANSLATED_MODULES, "bootResource"),
    ),
    (
        ("jdk.internal.loader.ClassLoaders", "appClassLoader", "()java.lang.ClassLoader"),
        (TRANSLATED_MODULES, "appClassLoader"),
    ),
    (
        ("jdk.internal.loader.ClassLoaders", "platformClassLoader", "()java.lang.ClassLoader"),
        (TRANSLATED_MODULES, "platformClassLoader"),
    ),
    // Whether a module may use a member of a class, which the JDK's reflection checks.
    (
        (
            "jdk.internal.reflect.Reflection",
            "verifyModuleAccess",
            "(java.lang.Module,java.lang.Class<?>)boolean",
        ),
        (TRANSLATED_MODULES, "verifyModuleAccess"),
    ),
    // Whether a network interface has an address, and the interface of a name, which the JVM
    // asks the system for (`java/net/TranslatedNet.java`).
    (
        ("java.net.NetworkInterface", "boundInetAddress0", "(java.net.InetAddress)boolean"),
        ("java.net.TranslatedNet", "boundInetAddress0"),
    ),
    (
        ("java.net.NetworkInterface", "getByName0", "(java.lang.String)java.net.NetworkInterface"),
        ("java.net.TranslatedNet", "getByName0"),
    ),
];

/// The translator's class of the JDK's modules and class loaders as the translation has them
/// (`java/overlay/java/lang/TranslatedModules.java`).
pub const TRANSLATED_MODULES: &str = "java.lang.TranslatedModules";

/// Calls of instance methods of `Class` and `ClassLoader` that give a module or a class loader,
/// and the static methods of `TRANSLATED_MODULES` that a translation with the JDK's sources calls
/// instead, with the receiver as the first argument (`lower/modules.rs`): a class's module is the
/// translation's module of its package, and its class loader that module's.
pub const MODULE_CALLS: &[(JdkMethod, (&str, &str))] = &[
    (
        ("java.lang.Class", "getModule", "()java.lang.Module"),
        ("moduleOf", "(java.lang.Class<?>)java.lang.Module"),
    ),
    (
        ("java.lang.Class", "getClassLoader", "()java.lang.ClassLoader"),
        ("classLoaderOf", "(java.lang.Class<?>)java.lang.ClassLoader"),
    ),
    (
        ("java.lang.ClassLoader", "getUnnamedModule", "()java.lang.Module"),
        ("unnamedModule", "(java.lang.ClassLoader)java.lang.Module"),
    ),
];

/// `Class.forName(module, name)` in a translation with the JDK's sources: the class that the
/// name finds, if it is in the module (`lower::by_name`).
pub const IN_MODULE: (&str, &str) = ("inModule", "(java.lang.Module,java.lang.Class<?>)java.lang.Class<?>");

/// The method of `TRANSLATED_MODULES` that a call of the JDK's method `(owner, name, signature)`
/// calls instead (`MODULE_CALLS`).
pub fn module_call(owner: &str, name: &str, signature: &str) -> Option<(&'static str, &'static str)> {
    MODULE_CALLS.iter().find(|((o, n, s), _)| (*o, *n, *s) == (owner, name, signature)).map(|(_, to)| *to)
}

/// The translator's class and method that a call of the JDK's method `(owner, name, signature)`
/// calls instead (`REDIRECTED_CALLS`), where a variable-arity parameter is written as an array
/// (javac writes `T...`).
pub fn redirected_call(owner: &str, name: &str, signature: &str) -> Option<(&'static str, &'static str)> {
    let signature = signature.replace("...", "[]");
    REDIRECTED_CALLS
        .iter()
        .find(|((o, n, s), _)| (*o, *n, *s) == (owner, name, &*signature))
        .map(|(_, to)| *to)
}

/// A method of the JDK, by class, name and javac's type of it.
pub type JdkMethod = (&'static str, &'static str, &'static str);

/// The translator's class of the accesses of a memory segment through a value layout's handle.
pub const TRANSLATED_SEGMENTS: &str = "jdk.internal.foreign.TranslatedSegments";

/// The translator's method that the access `node` calls in its place, by name and signature, when
/// it is `layout.varHandle().get(segment, offset)` or `layout.varHandle().set(segment, offset,
/// value)` of a value layout of a primitive (`ValueLayout.OfInt`), as `AbstractMemorySegmentImpl`'s
/// typed accessors make them: the handle is made at run time, and the translator's method
/// (`java/overlay/jdk/internal/foreign/TranslatedSegments.java`) does what it does, with the
/// layout as an argument after the segment.
pub fn segment_access(node: &Value) -> Option<(&'static str, String)> {
    let symbol = &node["symbol"];
    if text(node, "kind") != "METHOD_INVOCATION" || text(symbol, "owner") != "java.lang.invoke.VarHandle" {
        return None;
    }
    let name = match text(symbol, "name") {
        "get" => "get",
        "set" => "set",
        _ => return None,
    };
    let receiver = &node["methodSelect"]["expression"];
    if text(receiver, "kind") != "METHOD_INVOCATION"
        || text(&receiver["symbol"], "name") != "varHandle"
        || receiver["arguments"].as_array().is_none_or(|a| !a.is_empty())
    {
        return None;
    }
    let layout = text(&receiver["methodSelect"]["expression"], "javaType");
    let primitive = match layout.strip_prefix("java.lang.foreign.ValueLayout.Of")? {
        "Byte" => "byte",
        "Boolean" => "boolean",
        "Char" => "char",
        "Short" => "short",
        "Int" => "int",
        "Float" => "float",
        "Long" => "long",
        "Double" => "double",
        _ => return None,
    };
    let arguments = node["arguments"].as_array()?;
    let signature = match (name, arguments.len()) {
        ("get", 2) => format!("(java.lang.foreign.MemorySegment,{layout},long){primitive}"),
        ("set", 3) => format!("(java.lang.foreign.MemorySegment,{layout},long,{primitive})void"),
        _ => return None,
    };
    Some((name, signature))
}

/// `Locale.getLanguage()`, which `String.toUpperCase(locale)` and `toLowerCase(locale)` call to
/// tell the languages with their own case mapping (see `lower::strings`).
pub const LOCALE_LANGUAGE: JdkMethod = ("java.util.Locale", "getLanguage", "()java.lang.String");

/// What `String.format(locale, format, args...)` runs, as the JDK's does: `new
/// Formatter(locale).format(format, args).toString()`, each called on what the one before
/// returns (see `lower::strings`). So do `printf` and `format` of a `PrintStream` with a locale.
pub const FORMAT_IN_LOCALE: [JdkMethod; 3] = [
    ("java.util.Formatter", "<init>", "(java.util.Locale)void"),
    ("java.util.Formatter", "format", "(java.lang.String,java.lang.Object[])java.util.Formatter"),
    ("java.util.Formatter", "toString", "()java.lang.String"),
];

/// Whether a method's javac type is that of a formatting method with a locale first.
pub fn formats_in_locale(signature: &str) -> bool {
    signature.starts_with("(java.util.Locale,java.lang.String,")
}

/// The methods of `String` that stream with the JDK's methods (see `lower::strings`), and those
/// methods, each called on what the one before returns. `chars()` streams the string's UTF-16 code
/// units as an `int[]`. `lines()` streams its lines, an `Object[]` of strings, through an iterator,
/// so that the stream's size is unknown, as with the JDK's own spliterator.
pub const STRING_STREAMS: [(&str, &[JdkMethod]); 2] = [
    ("chars", &[("java.util.Arrays", "stream", "(int[])java.util.stream.IntStream")]),
    (
        "lines",
        &[
            ("java.util.Spliterators", "spliterator", "<T>(java.lang.Object[],int)java.util.Spliterator<T>"),
            (
                "java.util.Spliterators",
                "iterator",
                "<T>(java.util.Spliterator<? extends T>)java.util.Iterator<T>",
            ),
            (
                "java.util.Spliterators",
                "spliteratorUnknownSize",
                "<T>(java.util.Iterator<? extends T>,int)java.util.Spliterator<T>",
            ),
            (
                "java.util.stream.StreamSupport",
                "stream",
                "<T>(java.util.Spliterator<T>,boolean)java.util.stream.Stream<T>",
            ),
        ],
    ),
];

/// The JDK's methods that a regular expression call of `String` runs where its pattern is not a
/// constant (see `lower::regex`), by the method of `String`: what the JDK's `String` calls, each
/// called on what the one before returns.
pub const REGEX_CALLS: [(&str, &[JdkMethod]); 4] = [
    (
        "matches",
        &[("java.util.regex.Pattern", "matches", "(java.lang.String,java.lang.CharSequence)boolean")],
    ),
    (
        "replaceAll",
        &[
            PATTERN_COMPILE,
            PATTERN_MATCHER,
            ("java.util.regex.Matcher", "replaceAll", "(java.lang.String)java.lang.String"),
        ],
    ),
    (
        "replaceFirst",
        &[
            PATTERN_COMPILE,
            PATTERN_MATCHER,
            ("java.util.regex.Matcher", "replaceFirst", "(java.lang.String)java.lang.String"),
        ],
    ),
    (
        "split",
        &[
            PATTERN_COMPILE,
            ("java.util.regex.Pattern", "split", "(java.lang.CharSequence,int)java.lang.String[]"),
        ],
    ),
];
const PATTERN_COMPILE: JdkMethod =
    ("java.util.regex.Pattern", "compile", "(java.lang.String)java.util.regex.Pattern");
const PATTERN_MATCHER: JdkMethod =
    ("java.util.regex.Pattern", "matcher", "(java.lang.CharSequence)java.util.regex.Matcher");

/// The JDK's methods (`REGEX_CALLS`) that the call `node` runs, where it is a regular expression
/// call of `String` whose pattern is not a constant.
pub fn regex_call(node: &Value) -> Option<&'static [JdkMethod]> {
    fn constant(node: &Value) -> bool {
        match text(node, "kind") {
            "STRING_LITERAL" => true,
            "PARENTHESIZED" => constant(&node["expression"]),
            "IDENTIFIER" | "MEMBER_SELECT" => {
                text(node, "javaType") == "java.lang.String" && node["symbol"]["constant"].as_str().is_some()
            }
            _ => false,
        }
    }
    let symbol = &node["symbol"];
    if text(node, "kind") != "METHOD_INVOCATION" || text(symbol, "owner") != "java.lang.String" {
        return None;
    }
    let (_, methods) = REGEX_CALLS.iter().find(|(name, _)| *name == text(symbol, "name"))?;
    let pattern = node["arguments"].as_array()?.first()?;
    (!constant(pattern)).then_some(*methods)
}

/// The methods that a for-each over an `Iterable` calls.
pub const ITERATION: &[(&str, &str, &str)] = &[
    ("java.lang.Iterable", "iterator", "()java.util.Iterator<T>"),
    ("java.util.Iterator", "hasNext", "()boolean"),
    ("java.util.Iterator", "next", "()E"),
];

/// The method that a try-with-resources calls on each resource, which javac's tree does not show:
/// reaching it reaches the `close()` of every live class of a resource.
pub const CLOSE: (&str, &str, &str) = ("java.lang.AutoCloseable", "close", "()void");

/// The node kinds of class declarations, and the symbol kinds of classes.
pub const CLASS_KINDS: &[&str] = &["CLASS", "INTERFACE", "ENUM", "RECORD", "ANNOTATION_TYPE"];

/// What reachable code uses in the program made of these compilation units, starting from the
/// `main` method of the class with qualified name `main`. Streams are first assumed to stay
/// sequential; when the code so reached can make one parallel, the walk is made again without.
#[cfg(test)]
pub fn reach(units: &[&Value], main: &str) -> Result<Reach, String> {
    let mut rounds = Rounds::new(main);
    rounds.add(units)?;
    Ok(rounds.finish())
}

/// Reachability over units that come in rounds, as javac gives them when they are asked for
/// (`Reach::missing`): what `reach` gives over the units so far, where each round walks only
/// what the new units add. A walk keeps what it reached; the items that it could not visit
/// because no unit declared their class wait for that class, and a new unit's methods that
/// override a method whose overriders may run become reachable as they would have had the unit
/// been there from the start. A walk that reached what it would not have with the new units from
/// the start walks again from `main` (`Walk::stale`). The units walked must not change: when
/// they do, a new `Rounds` walks them all again.
pub struct Rounds<'t> {
    main: String,
    units: Vec<&'t Value>,
    /// The walk that assumes sequential streams, once there are units.
    sequential: Option<Walk<'t>>,
    /// The code of that walk that the check of the assumption has indexed (`sequential.rs`).
    check: sequential::Index<'t>,
    /// Whether the assumption holds over the units so far.
    stays_sequential: bool,
    /// The locale data classes of the libraries (`Walk::locale_data`).
    locale_data: Vec<String>,
    /// The service providers of the sources (`Walk::providers`).
    providers: Vec<super::modules::ServiceProvider>,
    /// The walk without the assumption, made once it fails, and then kept up too.
    parallel: Option<Walk<'t>>,
    /// How many times a walk started again (`Walk::stale`).
    restarts: usize,
    /// `Reach::string_array_writes`.
    string_array_writes: bool,
}

impl<'t> Rounds<'t> {
    pub fn new(main: &str) -> Rounds<'t> {
        Rounds {
            main: main.to_string(),
            units: Vec::new(),
            sequential: None,
            check: sequential::Index::default(),
            stays_sequential: true,
            locale_data: Vec::new(),
            providers: Vec::new(),
            parallel: None,
            restarts: 0,
            string_array_writes: false,
        }
    }

    /// Reachability in which `Arrays.sort` and `Arrays.fill` of a `String[]` reach the JDK's
    /// methods (`Reach::string_array_writes`).
    pub fn with_string_array_writes(mut self) -> Rounds<'t> {
        self.string_array_writes = true;
        self
    }

    /// Reachability in which a computed `Class.forName(module, name)` finds these locale data
    /// classes of the libraries (`by_name::locale_data_classes`).
    pub fn with_locale_data(mut self, classes: Vec<String>) -> Rounds<'t> {
        self.locale_data = classes;
        self
    }

    /// Reachability in which `ServiceLoader` finds these service providers, in its order
    /// (`modules::service_providers`).
    pub fn with_providers(mut self, providers: Vec<super::modules::ServiceProvider>) -> Rounds<'t> {
        self.providers = providers;
        self
    }

    /// How many times a walk started again from `main` over all the units so far, because
    /// what it reached before a unit came differs from what it would have reached with it
    /// (`Walk::stale`).
    pub fn restarts(&self) -> usize {
        self.restarts
    }

    /// Adds units to the program, and reaches what they add.
    pub fn add(&mut self, units: &[&'t Value]) -> Result<(), String> {
        self.units.extend(units);
        let sequential = match &mut self.sequential {
            Some(walk) => {
                walk.add(units);
                if walk.stale {
                    self.restarts += 1;
                    self.check = sequential::Index::default();
                    walk.restart(&self.main);
                }
                walk
            }
            None => {
                let walk = Walk::start(
                    &self.units,
                    &self.main,
                    true,
                    self.string_array_writes,
                    &self.locale_data,
                    &self.providers,
                )?;
                self.sequential.insert(walk)
            }
        };
        self.stays_sequential = self.check.streams_stay_sequential(sequential);
        match &mut self.parallel {
            Some(walk) => {
                walk.add(units);
                if walk.stale {
                    self.restarts += 1;
                    walk.restart(&self.main);
                }
            }
            None if !self.stays_sequential => {
                let walk = Walk::start(
                    &self.units,
                    &self.main,
                    false,
                    self.string_array_writes,
                    &self.locale_data,
                    &self.providers,
                )?;
                self.parallel = Some(walk);
            }
            None => {}
        }
        Ok(())
    }

    /// The classes that reachable code needs and no unit so far declares (`Reach::missing`).
    pub fn missing(&self) -> Vec<String> {
        let mut missing = BTreeSet::new();
        if let Some(walk) = &self.sequential {
            missing.extend(&walk.reach.missing);
        }
        if let Some(walk) = self.parallel.as_ref().filter(|_| !self.stays_sequential) {
            missing.extend(&walk.reach.missing);
        }
        missing.into_iter().cloned().collect()
    }

    /// What reachable code uses, over the units so far.
    pub fn finish(self) -> Reach {
        if let Some(walk) = &self.sequential {
            walk.explain();
        }
        if let Some(walk) = &self.parallel {
            walk.explain();
        }
        let sequential = self.sequential.map(|walk| walk.reach).unwrap_or_default();
        match self.parallel {
            Some(walk) if !self.stays_sequential => {
                let mut parallel = walk.reach;
                parallel.missing.extend(sequential.missing);
                parallel
            }
            _ => sequential,
        }
    }
}

/// A declaration that has become reachable and is still to be visited.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Item {
    Class(String),
    Init(String),
    Method(MethodKey),
    Field(FieldKey),
}

/// The declarations of the program, and the worklist over them.
#[derive(Default)]
struct Walk<'t> {
    /// Every class, with its declaration and its outer class.
    classes: HashMap<String, (&'t Value, Option<String>)>,
    methods: HashMap<MethodKey, &'t Value>,
    fields: HashMap<FieldKey, &'t Value>,
    /// The direct supertypes of each class.
    supertypes: HashMap<String, Vec<String>>,
    /// The methods that override each method, as javac reports them.
    overriders: HashMap<MethodKey, Vec<MethodKey>>,
    /// Overriders of reached methods whose class is not live yet, by that class: they become
    /// reachable when it is.
    waiting: HashMap<String, Vec<MethodKey>>,
    /// The methods whose overriders reachable code may run (`overriders`), and those of them
    /// that were `external` when one of their overriders was found.
    dispatched: HashSet<MethodKey>,
    dispatched_external: HashSet<MethodKey>,
    /// Whether the walk reached what it would not have reached with all its units from the
    /// start: a method that it took for an external one, as no unit declared its class, made
    /// its overriders reachable, and once a unit did, the method is one of the `dormant` ones,
    /// which make none reachable. The walk is then made again (`Rounds`).
    stale: bool,
    /// Items that could not be visited because no unit declares their class, by that class:
    /// they are visited when a unit added later does (`block`).
    blocked: HashMap<String, HashSet<Item>>,
    /// The classes declared, and the overriders found as (overridden, overrider), since the walk
    /// last took them up (`add`).
    declared: Vec<String>,
    found_overriders: Vec<(MethodKey, MethodKey)>,
    work: Vec<Item>,
    reach: Reach,
    /// Whether streams are assumed to stay sequential (`Reach::sequential`).
    sequential: bool,
    /// The classes that reachable code creates objects of by class name (`by_name`).
    by_name: HashSet<String>,
    /// The string literals of reachable code that are names of classes.
    named: HashSet<String>,
    /// The string literals of reachable code that could be names of classes that no unit
    /// declares (`by_name`).
    literals: HashSet<String>,
    /// The method whose code is being scanned.
    scanning: Option<MethodKey>,
    /// The class whose code (a method, a field's initializer or an initializer block) is being
    /// scanned, which `MethodHandles.lookup()` looks up in (`own_field_handle`).
    lookup_class: Option<String>,
    /// The locale data classes of the libraries, which a computed `Class.forName(module, name)`
    /// can find (`by_name::locale_data_classes`), and whether reachable code has one.
    locale_data: Vec<String>,
    finds_locale_data: bool,
    /// Whether reachable code finds a class by a name it computes, `Class.forName(name)`, which
    /// initializes every class that it can find (`by_name.rs`).
    finds_named: bool,
    /// Whether reachable code makes a new exception of an exception's class
    /// (`recreated_exception`): each exception class whose objects are created has the
    /// constructor that does it.
    recreates: bool,
    /// Whether reachable code calls `Enum.valueOf(c, name)`, which may make a constant of any
    /// enum whose class it has: each enum that reachable code names is initialized
    /// (`lower::enums`).
    enum_by_class: bool,
    /// The parameters, by method and position, that a method gives `ensureInitialized`, and the
    /// class literals that reachable code passes to each parameter (`initialize.rs`).
    initializing: HashSet<(MethodKey, usize)>,
    class_arguments: HashMap<(MethodKey, usize), BTreeSet<String>>,
    /// The lookups of members by name of reachable code, its string literals and the classes
    /// of its class literals, and the lookups made so far, by class and name (`reflection.rs`).
    lookups: BTreeSet<Lookup>,
    strings: HashSet<String>,
    class_literals: HashSet<String>,
    reflected_keys: HashSet<(String, String)>,
    /// The service providers of the sources, and the services that
    /// `ServiceLoader.load` of reachable code loads: by a class literal, or `None` for a
    /// `Class` value, which may be any service whose class literal reachable code has
    /// (`reflection.rs`).
    providers: Vec<super::modules::ServiceProvider>,
    service_loads: BTreeSet<Option<String>>,
    /// The providers found whose classes no unit declares yet, so that what makes them
    /// (`load_services`) is reached once a later round declares them.
    unmade: BTreeSet<String>,
    /// With `JAVA_TO_RUST_REACH_WHY`, the item whose visit first made each item reachable.
    why: Option<HashMap<Item, Item>>,
}

impl<'t> Walk<'t> {
    /// Writes on standard error, for each prefix in `JAVA_TO_RUST_REACH_WHY` (comma-separated, of
    /// `class.method`), the chain of items that made the first running method it names reachable.
    fn explain(&self) {
        let (Some(why), Ok(wanted)) = (&self.why, std::env::var("JAVA_TO_RUST_REACH_WHY")) else { return };
        for want in wanted.split(',') {
            let mut targets: Vec<&MethodKey> = self
                .reach
                .methods
                .iter()
                .filter(|m| !self.reach.dormant.contains(*m) && format!("{}.{}", m.0, m.1).starts_with(want))
                .collect();
            targets.sort();
            let Some(target) = targets.first() else {
                eprintln!("why {want}: not reached");
                continue;
            };
            eprintln!("why {want}: {} matches", targets.len());
            let mut item = Item::Method((*target).clone());
            let mut seen = HashSet::new();
            while seen.insert(item.clone()) {
                let line = match &item {
                    Item::Class(c) => format!("class {c}"),
                    Item::Init(c) => format!("init {c}"),
                    Item::Method(m) => {
                        let live = if self.reach.live.contains(&m.0) { "" } else { " (class not live)" };
                        format!("method {}.{}{}{live}", m.0, m.1, m.2)
                    }
                    Item::Field(f) => format!("field {}.{}", f.0, f.1),
                };
                eprintln!("  {line}");
                match why.get(&item) {
                    Some(parent) => item = parent.clone(),
                    None => break,
                }
            }
        }
    }

    /// Walks from `main` over these units, with streams assumed to stay sequential or not, with
    /// `Reach::string_array_writes`, and with the service providers of the sources
    /// (`Walk::providers`), which the first walk already loads.
    fn start(
        units: &[&'t Value],
        main: &str,
        sequential: bool,
        string_array_writes: bool,
        locale_data: &[String],
        providers: &[super::modules::ServiceProvider],
    ) -> Result<Walk<'t>, String> {
        let mut walk = Walk {
            sequential,
            why: std::env::var_os("JAVA_TO_RUST_REACH_WHY").map(|_| HashMap::new()),
            locale_data: locale_data.to_vec(),
            providers: providers.to_vec(),
            ..Walk::default()
        };
        walk.reach.sequential = sequential;
        walk.reach.string_array_writes = string_array_writes;
        walk.declare_units(units);
        walk.declared.clear();
        walk.found_overriders.clear();
        if !walk.classes.contains_key(main) {
            return Err(format!("the main class `{main}` is not among the sources"));
        }
        let entry = (main.to_string(), "main".to_string(), "(java.lang.String[])void".to_string());
        if !walk.methods.contains_key(&entry) {
            return Err(format!("`{main}` has no `public static void main(String[])`"));
        }
        walk.enter(main);
        Ok(walk)
    }

    /// Walks from `main`, the class's static initialization and its `main` method.
    fn enter(&mut self, main: &str) {
        let entry = (main.to_string(), "main".to_string(), "(java.lang.String[])void".to_string());
        self.work.push(Item::Init(main.to_string()));
        self.work.push(Item::Method(entry));
        self.run();
        self.note_named();
    }

    /// Walks again from `main`, as `start` would over the units declared, which it keeps.
    fn restart(&mut self, main: &str) {
        let (sequential, string_array_writes) = (self.sequential, self.reach.string_array_writes);
        *self = Walk {
            classes: std::mem::take(&mut self.classes),
            methods: std::mem::take(&mut self.methods),
            fields: std::mem::take(&mut self.fields),
            supertypes: std::mem::take(&mut self.supertypes),
            overriders: std::mem::take(&mut self.overriders),
            sequential,
            locale_data: std::mem::take(&mut self.locale_data),
            providers: std::mem::take(&mut self.providers),
            why: self.why.as_ref().map(|_| HashMap::new()),
            ..Walk::default()
        };
        self.reach.sequential = sequential;
        self.reach.string_array_writes = string_array_writes;
        self.enter(main);
    }

    /// Adds units, and walks on to what they add: the items that waited for a class they
    /// declare, the supertypes of their classes that are live, and their methods that override
    /// a method whose overriders may run.
    fn add(&mut self, units: &[&'t Value]) {
        self.declare_units(units);
        for class in std::mem::take(&mut self.declared) {
            self.reach.missing.remove(&class);
            self.work.extend(self.blocked.remove(&class).into_iter().flatten());
            if self.reach.live.contains(&class) {
                for supertype in self.supertypes[&class].clone() {
                    self.live(supertype);
                }
            }
        }
        for (overridden, overrider) in std::mem::take(&mut self.found_overriders) {
            if !self.dispatched.contains(&overridden) {
                continue;
            }
            if self.reach.external.contains(&overridden) {
                self.dispatched_external.insert(overridden.clone());
            }
            if self.reach.live.contains(&overrider.0) {
                self.work.push(Item::Method(overrider));
            } else {
                self.waiting.entry(overrider.0.clone()).or_default().push(overrider);
            }
        }
        self.add_named();
        self.run();
        self.note_named();
    }

    fn declare_units(&mut self, units: &[&'t Value]) {
        for unit in units {
            for decl in unit["typeDecls"].as_array().into_iter().flatten() {
                self.declare(decl, None);
            }
        }
    }

    fn declare(&mut self, decl: &'t Value, outer: Option<&str>) {
        if !CLASS_KINDS.contains(&text(decl, "kind")) {
            return;
        }
        let class = class_name(decl);
        if outside(&class) {
            if provided(&class) {
                for member in members(decl) {
                    if CLASS_KINDS.contains(&text(member, "kind"))
                        && TRANSLATED_NESTED.contains(&class_name(member).as_str())
                    {
                        self.declare(member, Some(&class));
                    }
                }
            }
            return;
        }
        for member in members(decl) {
            let name = text(member, "name").to_string();
            if !CLASS_KINDS.contains(&text(member, "kind")) {
                let mut locals = Vec::new();
                local_classes(member, &mut locals);
                for local in locals {
                    self.declare(local, Some(&class));
                }
            }
            match text(member, "kind") {
                "METHOD" => {
                    let key = (class.clone(), name, symbol_type(member).to_string());
                    for overridden in member["symbol"]["overrides"].as_array().into_iter().flatten() {
                        let overridden = (
                            text(overridden, "owner").to_string(),
                            key.1.clone(),
                            text(overridden, "type").to_string(),
                        );
                        self.overriders.entry(overridden.clone()).or_default().push(key.clone());
                        self.found_overriders.push((overridden, key.clone()));
                    }
                    self.methods.insert(key, member);
                }
                "VARIABLE" => {
                    self.fields.insert((class.clone(), name), member);
                }
                _ => self.declare(member, Some(&class)),
            }
        }
        // A method inherited from a superclass that implements a method of this class's interfaces.
        for inherited in decl["symbol"]["inherits"].as_array().into_iter().flatten() {
            let name = text(inherited, "name").to_string();
            let implemented = &inherited["implements"];
            let overridden =
                (text(implemented, "owner").to_string(), name.clone(), text(implemented, "type").to_string());
            let key = (text(inherited, "owner").to_string(), name, text(inherited, "type").to_string());
            let overriders = self.overriders.entry(overridden.clone()).or_default();
            if !overriders.contains(&key) {
                overriders.push(key.clone());
                self.found_overriders.push((overridden, key));
            }
        }
        let mut supertypes: Vec<String> = ["extendsClause", "implementsClause"]
            .iter()
            .flat_map(|header| match &decl[header] {
                Value::Array(items) => items.iter().collect(),
                one => vec![one],
            })
            .filter_map(supertype)
            .collect();
        let implicit = match text(decl, "kind") {
            "ENUM" => Some("java.lang.Enum"),
            "RECORD" => Some("java.lang.Record"),
            "CLASS" if decl["extendsClause"].is_null() && class != "java.lang.Object" => {
                Some("java.lang.Object")
            }
            _ => None,
        };
        supertypes.extend(implicit.map(str::to_string));
        self.supertypes.insert(class.clone(), supertypes);
        self.classes.insert(class.clone(), (decl, outer.map(str::to_string)));
        self.declared.push(class);
    }

    fn run(&mut self) {
        loop {
            while let Some(item) = self.work.pop() {
                let (before, current) = (self.work.len(), self.why.as_ref().map(|_| item.clone()));
                match item {
                    Item::Class(class) => self.class(class),
                    Item::Init(class) => self.init(class),
                    Item::Method(key) => self.method(key),
                    Item::Field(key) => self.field(key),
                }
                if let (Some(why), Some(current)) = (&mut self.why, current) {
                    for pushed in &self.work[before.min(self.work.len())..] {
                        if *pushed != current {
                            why.entry(pushed.clone()).or_insert(current.clone());
                        }
                    }
                }
            }
            // What reflection's lookups find among the classes found so far (`reflection.rs`).
            if !self.reflect() {
                break;
            }
        }
    }

    /// A class named by reachable code: it exists as a type, with its outer class and the types in
    /// its header.
    fn class(&mut self, class: String) {
        let Some((decl, outer)) = self.classes.get(&class).map(|(d, o)| (*d, o.clone())) else {
            return self.block(&class.clone(), Item::Class(class));
        };
        if !self.reach.classes.insert(class.clone()) {
            return;
        }
        if self.enum_by_class && self.is_enum(&class) {
            self.work.push(Item::Init(class));
        }
        if let Some(outer) = outer {
            self.work.push(Item::Class(outer));
        }
        for header in ["extendsClause", "implementsClause", "permitsClause", "typeParameters"] {
            self.scan(&decl[header]);
        }
    }

    /// A class whose static initialization runs: its static initializer blocks run, and its static
    /// fields with initializers that may do something besides computing a value are kept even if
    /// nothing reads them. (When classes can extend others, the superclass is initialized first.)
    fn init(&mut self, class: String) {
        let Some(&(decl, _)) = self.classes.get(&class) else {
            return self.block(&class.clone(), Item::Init(class));
        };
        if !self.reach.initialized.insert(class.clone()) {
            return;
        }
        self.work.push(Item::Class(class.clone()));
        self.initializers(decl, &class, true);
    }

    /// The initializers that run for the class's static initialization (`statics`) or for every
    /// object it creates. A class's `serialPersistentFields` is kept only if code reads it
    /// (`for_serialization`).
    fn initializers(&mut self, decl: &'t Value, class: &str, statics: bool) {
        for member in members(decl) {
            let is_static = match text(member, "kind") {
                "BLOCK" => member["static"].as_bool() == Some(true),
                "VARIABLE" => has_flag(member, "STATIC"),
                _ => continue,
            };
            if is_static != statics {
                continue;
            }
            if text(member, "kind") == "BLOCK" {
                let outer = self.lookup_class.replace(class.to_string());
                self.scan(member);
                self.lookup_class = outer;
            } else if has_effects(&member["initializer"]) && !for_serialization(member) {
                self.work.push(Item::Field((class.to_string(), text(member, "name").to_string())));
            }
        }
    }

    fn method(&mut self, key: MethodKey) {
        if key.0 == "java.lang.Enum"
            && key.1 == "valueOf"
            && !std::mem::replace(&mut self.enum_by_class, true)
        {
            let named: Vec<String> = self.reach.classes.iter().filter(|c| self.is_enum(c)).cloned().collect();
            self.work.extend(named.into_iter().map(Item::Init));
        }
        let node = self.methods.get(&key).filter(|_| !replaced(&key.0, &key.1, &key.2));
        let Some(&node) = node else {
            self.block(&key.0, Item::Method(key.clone()));
            // An enum's `values()` and `valueOf()`, which javac declares without code, initialize
            // the enum: its constants are built with what their arguments call.
            if matches!(key.1.as_str(), "values" | "valueOf")
                && self.classes.get(&key.0).is_some_and(|(decl, _)| text(decl, "kind") == "ENUM")
            {
                self.work.push(Item::Init(key.0.clone()));
            }
            // What overrides a method of the runtime, such as `Object.toString`, can still run.
            if self.reach.external.insert(key.clone()) {
                for overrider in self.overriders(&key) {
                    self.work.push(Item::Method(overrider));
                }
                // The thread that `start()` starts runs the thread's `run()`, which runs its
                // `Runnable` (see `java/Threads.java` in the translator).
                if key.0 == "java.lang.Thread" && key.1 == "start" {
                    for class in ["java.lang.Thread", "java.lang.Runnable"] {
                        self.work.push(Item::Method((class.into(), "run".into(), "()void".into())));
                    }
                }
                // `CharSequence.isEmpty()` runs the sequence's `length()`, as the translation
                // writes it (`lower::object`).
                if key.0 == "java.lang.CharSequence" && key.1 == "isEmpty" {
                    self.work.push(Item::Method((key.0.clone(), "length".into(), "()int".into())));
                }
            }
            return;
        };
        // A method called before a unit declared its class was taken for one of the runtime's.
        let was_external = self.reach.external.remove(&key);
        let class = key.0.clone();
        let is_ctor = key.1 == "<init>";
        let is_static = node["symbol"]["static"].as_bool() == Some(true);
        if !self.reach.methods.insert(key.clone()) {
            // A dormant method runs once its class is live.
            if !self.reach.live.contains(&class) || !self.reach.dormant.remove(&key) {
                return;
            }
        } else if !is_static && !is_ctor && !self.reach.live.contains(&class) && self.has_objects(&class) {
            // Until an object of its class is created, only its signature is used.
            if was_external && self.dispatched.remove(&key) && self.dispatched_external.contains(&key) {
                self.stale = true;
            }
            self.reach.dormant.insert(key.clone());
            self.waiting.entry(class.clone()).or_default().push(key);
            self.work.push(Item::Class(class));
            for part in ["typeParameters", "returnType", "parameters", "thrown"] {
                self.scan(&node[part]);
            }
            return;
        }
        if (is_static || is_ctor)
            && was_external
            && self.dispatched.remove(&key)
            && self.dispatched_external.contains(&key)
        {
            self.stale = true;
        }
        self.work.push(Item::Class(class.clone()));
        if is_static || is_ctor {
            self.work.push(Item::Init(class.clone()));
        }
        if is_ctor {
            self.instantiate(class);
        }
        if !is_static && !is_ctor {
            for overrider in self.overriders(&key) {
                self.work.push(Item::Method(overrider));
            }
        }
        let outer_class = self.lookup_class.replace(key.0.clone());
        let outer = self.scanning.replace(key);
        self.scan(node);
        self.scanning = outer;
        self.lookup_class = outer_class;
    }

    /// Whether `class` is a class whose objects come from its constructors, which reachability
    /// sees: not an interface, which the runtime's classes and lambdas implement too.
    fn has_objects(&self, class: &str) -> bool {
        self.classes
            .get(class)
            .is_some_and(|(decl, _)| matches!(text(decl, "kind"), "CLASS" | "ENUM" | "RECORD"))
    }

    /// Whether `class` is an enum class.
    fn is_enum(&self, class: &str) -> bool {
        self.classes.get(class).is_some_and(|(decl, _)| text(decl, "kind") == "ENUM")
    }

    /// A class whose objects are created: it is live, and its instance initializers run.
    fn instantiate(&mut self, class: String) {
        if !self.reach.instantiated.insert(class.clone()) {
            return;
        }
        if self.recreates {
            self.recreatable(&class);
        }
        self.live(class.clone());
        let decl = self.classes[&class].0;
        self.initializers(decl, &class, false);
        // Library code calls these on any object, as string conversion calls `toString`.
        for (name, signature) in OBJECT_METHODS {
            self.work.push(Item::Method((class.clone(), name.to_string(), signature.to_string())));
        }
    }

    /// Reachable code makes a new exception of an exception's class (`recreated_exception`).
    fn recreates_exceptions(&mut self) {
        if std::mem::replace(&mut self.recreates, true) {
            return;
        }
        let classes: Vec<String> = self.reach.instantiated.iter().cloned().collect();
        for class in classes {
            self.recreatable(&class);
        }
    }

    /// The constructor that makes a new exception of class `class` from another
    /// (`recreated_exception`): its public one of a `Throwable`, else its public one without
    /// parameters (and `initCause`, a standard method).
    fn recreatable(&mut self, class: &str) {
        let mut work = vec![class.to_string()];
        let mut throwable = false;
        while let Some(c) = work.pop() {
            if c == "java.lang.Throwable" || crate::lower::standard_subclass(&c, "java.lang.Throwable") {
                throwable = true;
                break;
            }
            work.extend(self.supertypes.get(&c).cloned().unwrap_or_default());
        }
        if !throwable {
            return;
        }
        for signature in ["(java.lang.Throwable)void", "()void"] {
            let key = (class.to_string(), "<init>".to_string(), signature.to_string());
            if self.methods.get(&key).is_some_and(|m| has_flag(m, "PUBLIC")) {
                self.work.push(Item::Method(key));
                return;
            }
        }
    }

    /// The methods that a call of the instance method `method` may run instead of it: those that
    /// override it in live classes. Those in classes that are not live yet wait until they are.
    fn overriders(&mut self, method: &MethodKey) -> Vec<MethodKey> {
        self.dispatched.insert(method.clone());
        let mut now = Vec::new();
        let overriders = self.overriders.get(method).cloned().unwrap_or_default();
        if !overriders.is_empty() && self.reach.external.contains(method) {
            self.dispatched_external.insert(method.clone());
        }
        for overrider in overriders {
            if self.reach.live.contains(&overrider.0) {
                now.push(overrider);
            } else {
                self.waiting.entry(overrider.0.clone()).or_default().push(overrider);
            }
        }
        now
    }

    /// Makes a class live, and its supertypes: the overriders that waited for one become
    /// reachable.
    fn live(&mut self, class: String) {
        let mut work = vec![class];
        while let Some(class) = work.pop() {
            if !self.reach.live.insert(class.clone()) {
                continue;
            }
            self.missing(&class);
            for overrider in self.waiting.remove(&class).unwrap_or_default() {
                self.work.push(Item::Method(overrider));
            }
            work.extend(self.supertypes.get(&class).cloned().unwrap_or_default());
        }
    }

    fn field(&mut self, key: FieldKey) {
        let Some(&node) = self.fields.get(&key) else {
            // A field of a standard class (the translator's `Thread`) is declared with the class
            // of its type, though the code may never create one (`ForkJoinWorkerThread` clears
            // `Thread.inheritableThreadLocals` through `Unsafe`).
            if let Some(ty) = crate::lower::standard_field_type(&key.0, &key.1) {
                self.work.push(Item::Class(ty));
            }
            return self.block(&key.0.clone(), Item::Field(key));
        };
        if !self.reach.fields.insert(key.clone()) {
            return;
        }
        self.work.push(Item::Class(key.0.clone()));
        if has_flag(node, "STATIC") {
            self.work.push(Item::Init(key.0.clone()));
        }
        let outer = self.lookup_class.replace(key.0);
        self.scan(node);
        self.lookup_class = outer;
    }

    /// An item that needs a class: when no unit declares it, the class is missing, and the item
    /// waits for it.
    fn block(&mut self, class: &str, item: Item) {
        if !self.classes.contains_key(class) && !outside(class) {
            self.missing(class);
            self.blocked.entry(class.to_string()).or_default().insert(item);
        }
    }

    /// Notes a class that reachable code needs, if no unit declares it (`Reach::missing`).
    fn missing(&mut self, class: &str) {
        if !self.classes.contains_key(class) && !outside(class) && !self.reach.missing.contains(class) {
            self.reach.missing.insert(class.to_string());
        }
    }

    /// Makes every declaration that a node of this subtree refers to reachable. The local and
    /// anonymous classes declared inside are not scanned with it: their members are reached as
    /// those of any other class, a constructor by its `new` and an instance method by a call or
    /// by dispatch once the class is instantiated. (The body of an enum constant is not such a
    /// class, see `local_classes`, and is scanned with its constant.) Annotations are skipped:
    /// without reflection they do nothing at run time. So is the branch of an `if` or a `?:`
    /// that never runs (`live_branch`).
    fn scan(&mut self, value: &'t Value) {
        match value {
            Value::Object(_) if matches!(text(value, "kind"), "ANNOTATION" | "TYPE_ANNOTATION") => {}
            Value::Object(_)
                if CLASS_KINDS.contains(&text(value, "kind"))
                    && self.classes.contains_key(&class_name(value)) => {}
            // An `if` that tests a property that is never set runs only its `else` branch.
            Value::Object(_) if text(value, "kind") == "BLOCK" && !unset_property_ifs(value).is_empty() => {
                let dead = unset_property_ifs(value);
                for s in value["statements"].as_array().into_iter().flatten() {
                    if dead.iter().any(|d| std::ptr::eq(*d, s)) {
                        self.scan(&s["elseStatement"]);
                    } else {
                        self.scan(s);
                    }
                }
            }
            // `Runtime.version().feature()`, a constant of the translation (`lower::system`),
            // which makes no version.
            Value::Object(_) if release_query(value) => {}
            // The branch that never runs is not reached, and neither is the condition, which the
            // lowering drops: a flag's class need not exist.
            Value::Object(_) if let Some(branch) = live_branch(value, self.sequential) => {
                self.scan(branch);
            }
            Value::Object(_) if let Some((array, length)) = component_array(value) => {
                self.scan(array);
                self.scan(length);
            }
            // A call of `java.util.Arrays` that the runtime does (`lower::runtime_arrays_call`)
            // reaches only what its arguments do.
            // So does an in-place one of a `String[]`, unless it reaches the JDK's
            // (`Reach::string_array_writes`).
            Value::Object(_)
                if runtime_arrays_call(value)
                    && !(self.reach.string_array_writes && writes_string_array(value)) =>
            {
                self.scan(&value["arguments"])
            }
            // So does a call that the runtime makes on `System.out` or `System.err`
            // (`direct_stream_call`).
            Value::Object(_) if runtime_stream_call(value) => self.scan(&value["arguments"]),
            // An access of a segment through a value layout's handle calls the translator's
            // method, with the layout, and makes no handle.
            Value::Object(_) if let Some((name, signature)) = segment_access(value) => {
                self.work.push(Item::Method((TRANSLATED_SEGMENTS.into(), name.into(), signature)));
                self.scan(&value["methodSelect"]["expression"]["methodSelect"]["expression"]);
                self.scan(&value["arguments"]);
            }
            // An object created from a class name (`by_name`) reaches the constructors of the
            // classes that the name can be, not reflection.
            Value::Object(_) if let Some(name) = created_by_name(value) => {
                self.created_by_name(value);
                self.scan(&value["type"]);
                self.scan(name);
            }
            Value::Object(fields) => {
                // A for-each over an `Iterable` calls its iterator's methods, which javac's tree
                // does not show.
                if text(value, "kind") == "ENHANCED_FOR_LOOP"
                    && !text(&value["expression"], "javaType").ends_with(']')
                {
                    for (class, name, signature) in ITERATION {
                        self.work.push(Item::Method((
                            class.to_string(),
                            name.to_string(),
                            signature.to_string(),
                        )));
                    }
                }
                if text(value, "kind") == "TRY"
                    && value["resources"].as_array().is_some_and(|r| !r.is_empty())
                {
                    let (class, name, signature) = CLOSE;
                    self.work.push(Item::Method((
                        class.to_string(),
                        name.to_string(),
                        signature.to_string(),
                    )));
                }
                if text(value, "kind") == "VARIABLE" {
                    self.stub_type(text(&value["symbol"], "type"));
                }
                self.initialization(value);
                // A class that `Class.forName` names exists, and is initialized if the call says
                // so (`lower::by_name`).
                if let Some((class, initialize)) = class_for_name(value) {
                    self.work.push(if initialize { Item::Init(class) } else { Item::Class(class) });
                }
                self.reflective(value);
                self.reflection_scan(value);
                if recreated_exception(value).is_some() {
                    self.recreates_exceptions();
                }
                if let Some(methods) = regex_call(value) {
                    for (class, method, signature) in methods {
                        self.work.push(Item::Method((
                            (*class).into(),
                            (*method).into(),
                            (*signature).into(),
                        )));
                    }
                }
                if let Some(field) = field_offset_named(value) {
                    self.work.push(Item::Field(field));
                }
                if let Some(field) = own_field_handle(value)
                    && let Some(class) = self.lookup_class.clone()
                {
                    self.work.push(Item::Field((class, field)));
                }
                if text(value, "kind") == "STRING_LITERAL"
                    && let Some(literal) = value["value"].as_str()
                {
                    self.named(literal);
                }
                // A lambda creates an object of a class that implements its interface with the
                // interface's abstract method.
                let descriptor = &value["descriptor"];
                if matches!(text(value, "kind"), "LAMBDA_EXPRESSION" | "MEMBER_REFERENCE")
                    && descriptor.is_object()
                {
                    let target = text(descriptor, "target").to_string();
                    self.work.push(Item::Class(target.clone()));
                    self.live(target);
                    let (owner, name) = (text(descriptor, "owner"), text(descriptor, "name"));
                    self.work.push(Item::Method((
                        owner.into(),
                        name.into(),
                        text(descriptor, "type").into(),
                    )));
                }
                for (field, child) in fields {
                    if field == "symbol" {
                        self.symbol(child);
                    } else {
                        self.scan(child);
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|item| self.scan(item)),
            _ => {}
        }
    }

    /// Reaches what a formatting with a locale runs (`FORMAT_IN_LOCALE`).
    fn format_in_locale(&mut self) {
        for (class, method, signature) in FORMAT_IN_LOCALE {
            self.work.push(Item::Method((class.into(), method.into(), signature.into())));
        }
    }

    fn symbol(&mut self, symbol: &Value) {
        let (owner, name) = (text(symbol, "owner").to_string(), text(symbol, "name").to_string());
        let member = (owner.as_str(), name.as_str(), text(symbol, "type"));
        for (_, (class, method, signature)) in STANDARD_CALLS.iter().filter(|(m, _)| *m == member) {
            self.work.push(Item::Method(((*class).into(), (*method).into(), (*signature).into())));
        }
        // The translator's methods that give a class's module and class loader (`MODULE_CALLS`).
        if text(symbol, "kind") == "METHOD"
            && let Some((to, signature)) = module_call(member.0, member.1, member.2)
        {
            self.work.push(Item::Method((TRANSLATED_MODULES.into(), to.into(), signature.into())));
        }
        match text(symbol, "kind") {
            "METHOD"
                if owner == "java.lang.String"
                    && matches!(name.as_str(), "toUpperCase" | "toLowerCase")
                    && text(symbol, "type") == "(java.util.Locale)java.lang.String" =>
            {
                let (class, method, signature) = LOCALE_LANGUAGE;
                self.work.push(Item::Method((class.into(), method.into(), signature.into())));
            }
            "METHOD"
                if owner == "java.lang.String"
                    && let Some((_, methods)) = STRING_STREAMS.iter().find(|(m, _)| *m == name) =>
            {
                for (class, method, signature) in *methods {
                    self.work.push(Item::Method(((*class).into(), (*method).into(), (*signature).into())));
                }
            }
            // The security manager, which is null: its type is a stub's (see `stubs`).
            "METHOD" if owner == "java.lang.System" && name == "getSecurityManager" => {
                self.stub_type("java.lang.SecurityManager");
            }
            // The annotations that reflection finds (`ANNOTATION_VALUES`).
            "METHOD" if name == "getAnnotation" && owner == "java.lang.Class" => {
                for (class, method, signature) in ANNOTATION_VALUES {
                    self.work.push(Item::Method((class.into(), method.into(), signature.into())));
                }
            }
            // The translator's properties (see `lower::system`).
            "METHOD" if owner == "java.lang.System" && name == "getProperties" => {
                let (class, method, signature) = TRANSLATED_PROPERTIES;
                self.work.push(Item::Method((class.into(), method.into(), signature.into())));
            }
            // The coders of a charset (`CHARSET_CODERS`).
            "METHOD"
                if let Some((class, to, signature)) = charset_coder(&owner, &name, text(symbol, "type"))
                    .or_else(|| charset_coding(&owner, &name, text(symbol, "type"))) =>
            {
                self.work.push(Item::Method((class.into(), to.into(), signature.into())));
            }
            // The translator's methods that replace some of the JDK's (`REDIRECTED_CALLS`).
            "METHOD" if let Some((class, to)) = redirected_call(&owner, &name, text(symbol, "type")) => {
                let signature = text(symbol, "type").to_string();
                self.work.push(Item::Method((class.into(), to.into(), signature)));
            }
            // The translator's code for what `JavaLangAccess` does (see `lower.rs`).
            "METHOD"
                if owner == "jdk.internal.access.JavaLangAccess"
                    && TRANSLATED_LANG_ACCESS.contains(&name.as_str()) =>
            {
                let signature = text(symbol, "type").to_string();
                self.work.push(Item::Method((TRANSLATED_LANG_ACCESS_CLASS.into(), name, signature)));
            }
            // Formatted where it is called, and printed by `PRINT_FORMATTED`.
            "METHOD" if owner == "java.io.PrintStream" && matches!(name.as_str(), "printf" | "format") => {
                let (class, method, signature) = PRINT_FORMATTED;
                self.work.push(Item::Method((class.into(), method.into(), signature.into())));
                if formats_in_locale(text(symbol, "type")) {
                    self.format_in_locale();
                }
            }
            "METHOD"
                if owner == "java.lang.String"
                    && name == "format"
                    && formats_in_locale(text(symbol, "type")) =>
            {
                self.format_in_locale();
            }
            "METHOD" | "CONSTRUCTOR" => {
                self.stub_method(symbol);
                self.work.push(Item::Method((owner, name, text(symbol, "type").to_string())));
            }
            // `System.out`, `String.CASE_INSENSITIVE_ORDER` and the like as a value: the
            // translator's object.
            "FIELD" if let Some((class, field)) = redirected_field(&owner, &name) => {
                self.work.push(Item::Field((class.into(), field.into())));
            }
            "FIELD" | "ENUM_CONSTANT" => self.work.push(Item::Field((owner, name))),
            kind if CLASS_KINDS.contains(&kind) => {
                self.work.push(Item::Class(strip_arguments(text(symbol, "type"))))
            }
            _ => {}
        }
    }
}

/// The local and anonymous class declarations in the code of a member, outside the bodies of
/// the classes found: they are declared with the member's class as their outer class, and
/// `Walk::scan` reaches their members when it scans the code.
fn local_classes<'t>(value: &'t Value, out: &mut Vec<&'t Value>) {
    match value {
        Value::Object(fields) => {
            if CLASS_KINDS.contains(&text(value, "kind")) {
                // javac names the body of an enum constant `<anonymous Planet>`: it belongs to
                // the enum (see `lower::constant_bodies`).
                if !symbol_type(value).starts_with('<') {
                    out.push(value);
                }
                return;
            }
            for (field, child) in fields {
                if field != "symbol" {
                    local_classes(child, out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| local_classes(item, out)),
        _ => {}
    }
}

/// The qualified name of a class declaration, from the type javac gives its symbol
/// (`com.example.Box<T>` for a generic class).
pub fn class_name(decl: &Value) -> String {
    strip_arguments(symbol_type(decl))
}

/// The qualified name of the class that a type in a class header names: `List<E>` names
/// `java.util.List`.
fn supertype(ty: &Value) -> Option<String> {
    match text(ty, "kind") {
        "PARAMETERIZED_TYPE" => supertype(&ty["type"]),
        _ => ty["symbol"]["type"].as_str().map(strip_arguments),
    }
}

/// A type without its type arguments: `java.util.ArrayList<E>.Itr` is `java.util.ArrayList.Itr`.
pub(super) fn strip_arguments(ty: &str) -> String {
    let mut depth = 0;
    ty.chars()
        .filter(|&c| {
            match c {
                '<' => depth += 1,
                '>' => depth -= 1,
                _ => return depth == 0,
            }
            false
        })
        .collect()
}

pub(super) fn members(decl: &Value) -> impl Iterator<Item = &Value> {
    decl["members"].as_array().into_iter().flatten()
}

pub(super) fn text<'v>(node: &'v Value, field: &str) -> &'v str {
    node[field].as_str().unwrap_or("")
}

pub(super) fn symbol_type(node: &Value) -> &str {
    text(&node["symbol"], "type")
}

pub(super) fn has_flag(node: &Value, flag: &str) -> bool {
    if flag == "STATIC"
        && let Some(s) = node["symbol"]["static"].as_bool()
    {
        return s;
    }
    node["modifiers"]["flags"].as_array().is_some_and(|f| f.iter().any(|x| x == flag))
}

/// Whether a field is the `serialPersistentFields` of a serializable class: serialization reads it
/// by reflection, which a translation has not, and its initializer only creates the
/// `ObjectStreamField`s that describe the fields, from arguments without effects. Some thirty
/// classes of java.base have one (`String`, `Random`, `ConcurrentHashMap`, ...), whose
/// `Long.TYPE` and `String.class` arguments are `Class` values.
fn for_serialization(field: &Value) -> bool {
    let describes = |element: &Value| {
        text(element, "kind") == "NEW_CLASS"
            && text(&element["symbol"], "owner") == "java.io.ObjectStreamField"
            && !has_effects(&element["arguments"])
    };
    let init = &field["initializer"];
    text(field, "name") == "serialPersistentFields"
        && has_flag(field, "STATIC")
        && text(init, "kind") == "NEW_ARRAY"
        && init["initializers"].as_array().is_some_and(|elements| elements.iter().all(describes))
}

/// Whether evaluating an initializer may do more than compute a value: call a method, create an
/// object, assign, or throw on a division.
fn has_effects(value: &Value) -> bool {
    match value {
        Value::Object(fields) => {
            let kind = text(value, "kind");
            matches!(
                kind,
                "METHOD_INVOCATION"
                    | "NEW_CLASS"
                    | "ASSIGNMENT"
                    | "PREFIX_INCREMENT"
                    | "PREFIX_DECREMENT"
                    | "POSTFIX_INCREMENT"
                    | "POSTFIX_DECREMENT"
                    | "DIVIDE"
                    | "REMAINDER"
            ) || kind.ends_with("_ASSIGNMENT")
                || fields.values().any(has_effects)
        }
        Value::Array(items) => items.iter().any(has_effects),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::json;

    /// A static call of `class.name()`, as a statement of javac's tree.
    fn call(class: &str, name: &str) -> Value {
        json!({"kind": "EXPRESSION_STATEMENT", "expression": {
            "kind": "METHOD_INVOCATION",
            "symbol": {"kind": "METHOD", "owner": class, "name": name, "type": "()void", "static": true},
        }})
    }

    /// A class with static methods of no arguments (`main` takes its `String[]`), each with the
    /// statements given.
    fn class(name: &str, methods: &[(&str, Vec<Value>)]) -> Value {
        let members: Vec<Value> = methods
            .iter()
            .map(|(method, statements)| {
                let ty = if *method == "main" { "(java.lang.String[])void" } else { "()void" };
                json!({"kind": "METHOD", "name": method, "symbol": {"type": ty, "static": true},
                       "body": {"kind": "BLOCK", "statements": statements}})
            })
            .collect();
        json!({"kind": "CLASS", "symbol": {"type": name}, "members": members})
    }

    #[test]
    fn unsafe_updates_of_int_long_and_reference_fields_are_recognized_in_every_mode() {
        let unsafe_ = "jdk.internal.misc.Unsafe";
        let found = |name: &str, sig: &str| unsafe_field_update(unsafe_, name, sig);
        assert_eq!(found("getAndAddInt", "(java.lang.Object,long,int)int"), Some(FieldUpdate::GetAndAdd));
        assert_eq!(
            found("getAndAddLongRelease", "(java.lang.Object,long,long)long"),
            Some(FieldUpdate::GetAndAdd)
        );
        assert_eq!(
            found("getAndAddReference", "(java.lang.Object,long,java.lang.Object)java.lang.Object"),
            None
        );
        let swap = "(java.lang.Object,long,java.lang.Object)java.lang.Object";
        assert_eq!(found("getAndSetReference", swap), Some(FieldUpdate::GetAndSet));
        let cas = "(java.lang.Object,long,int,int)boolean";
        assert_eq!(found("compareAndSetInt", cas), Some(FieldUpdate::CompareAndSet));
        assert_eq!(found("weakCompareAndSetIntPlain", cas), Some(FieldUpdate::CompareAndSet));
        assert_eq!(found("compareAndSetIntPlain", cas), None);
        let exchange = "(java.lang.Object,long,long,long)long";
        assert_eq!(found("compareAndExchangeLongAcquire", exchange), Some(FieldUpdate::CompareAndExchange));
        assert_eq!(found("getIntVolatile", "(java.lang.Object,long)int"), None);
        let add = "(java.lang.Object,long,float)float";
        assert_eq!(found("getAndAddFloatAcquire", add), Some(FieldUpdate::GetAndAdd));
        let narrow = "(java.lang.Object,long,byte,byte)byte";
        assert_eq!(found("compareAndExchangeByte", narrow), Some(FieldUpdate::CompareAndExchange));
        assert_eq!(found("getAndBitwiseOrByte", "(java.lang.Object,long,byte)byte"), None);
        assert_eq!(found("getAndAddShort", "(java.lang.Object,long,short)short"), None);
        assert!(replaced(unsafe_, "compareAndSetLong", "(java.lang.Object,long,long,long)boolean"));
    }

    #[test]
    fn rounds_reach_what_a_walk_over_all_the_units_does() {
        let units = [
            json!({"typeDecls": [class("Main", &[("main", vec![call("Library", "run")])])]}),
            json!({"typeDecls": [class("Library", &[("run", vec![call("Deeper", "go")]), ("unused", vec![])])]}),
            json!({"typeDecls": [class("Deeper", &[("go", vec![call("Library", "run")])])]}),
        ];
        let mut rounds = Rounds::new("Main");
        let mut asked = Vec::new();
        for unit in &units {
            rounds.add(&[unit]).unwrap();
            asked.push(rounds.missing());
        }
        assert_eq!(asked, [vec!["Library".to_string()], vec!["Deeper".to_string()], vec![]]);
        let (rounds, all) = (rounds.finish(), reach(&units.iter().collect::<Vec<_>>(), "Main").unwrap());
        assert_eq!(rounds.methods, all.methods);
        assert_eq!(rounds.classes, all.classes);
        assert_eq!(rounds.initialized, all.initialized);
        assert_eq!(rounds.external, all.external);
        assert!(rounds.external.is_empty() && rounds.missing.is_empty(), "{rounds:?}");
        let method = |class: &str, name: &str| (class.to_string(), name.to_string(), "()void".to_string());
        assert!(rounds.methods.contains(&method("Deeper", "go")), "{rounds:?}");
        assert!(!rounds.methods.contains(&method("Library", "unused")), "{rounds:?}");
    }

    #[test]
    fn a_translation_is_headless_so_what_needs_a_display_is_not_reached() {
        let headless = json!({"kind": "METHOD_INVOCATION", "symbol": {"kind": "METHOD",
            "owner": "java.awt.GraphicsEnvironment", "name": "isHeadless", "type": "()boolean", "static": true}});
        let gui = json!({"kind": "IDENTIFIER", "symbol": {"kind": "LOCAL_VARIABLE", "name": "gui"}});
        // `if (gui && !GraphicsEnvironment.isHeadless()) Window.show(); else Console.run();`
        let shows = json!({"kind": "IF",
            "condition": {"kind": "PARENTHESIZED", "expression": {"kind": "CONDITIONAL_AND",
                "leftOperand": gui,
                "rightOperand": {"kind": "LOGICAL_COMPLEMENT", "expression": headless}}},
            "thenStatement": call("Window", "show"),
            "elseStatement": call("Console", "run")});
        // `if (GraphicsEnvironment.isHeadless()) Console.log(); else Toolkit.start();`
        let starts = json!({"kind": "IF",
            "condition": {"kind": "PARENTHESIZED", "expression": headless},
            "thenStatement": call("Console", "log"),
            "elseStatement": call("Toolkit", "start")});
        let unit = json!({"typeDecls": [
            class("Main", &[("main", vec![shows, starts])]),
            class("Window", &[("show", vec![])]),
            class("Toolkit", &[("start", vec![])]),
            class("Console", &[("run", vec![]), ("log", vec![])]),
        ]});
        let reach = reach(&[&unit], "Main").unwrap();
        let method = |class: &str, name: &str| (class.to_string(), name.to_string(), "()void".to_string());
        assert!(reach.methods.contains(&method("Console", "run")), "{reach:?}");
        assert!(reach.methods.contains(&method("Console", "log")), "{reach:?}");
        assert!(!reach.classes.contains("Window") && !reach.classes.contains("Toolkit"), "{reach:?}");
    }

    #[test]
    fn a_branch_guarded_by_a_flag_only_flight_recorder_sets_is_not_reached() {
        let flag = json!({"kind": "MEMBER_SELECT",
            "symbol": {"kind": "FIELD", "owner": "java.lang.Throwable", "name": "jfrTracing", "static": true}});
        let guarded = json!({"kind": "IF",
            "condition": {"kind": "PARENTHESIZED", "expression": {"kind": "CONDITIONAL_AND",
                "leftOperand": flag, "rightOperand": {"kind": "LITERAL"}}},
            "thenStatement": call("Tracer", "trace"),
            "elseStatement": call("Other", "run")});
        let unit = json!({"typeDecls": [
            class("Main", &[("main", vec![guarded])]),
            class("Tracer", &[("trace", vec![])]),
            class("Other", &[("run", vec![])]),
        ]});
        let reach = reach(&[&unit], "Main").unwrap();
        let method = |class: &str, name: &str| (class.to_string(), name.to_string(), "()void".to_string());
        assert!(reach.methods.contains(&method("Other", "run")), "{reach:?}");
        assert!(!reach.methods.contains(&method("Tracer", "trace")), "{reach:?}");
        assert!(!reach.classes.contains("Tracer"), "{reach:?}");
    }

    #[test]
    fn only_the_branch_that_finds_no_archived_value_is_reached() {
        let archived = json!({"kind": "IDENTIFIER",
            "symbol": {"kind": "FIELD", "owner": "java.util.ImmutableCollections", "name": "archivedObjects", "static": true}});
        let test = |kind: &str| {
            json!({"kind": "PARENTHESIZED", "expression": {"kind": kind,
                "leftOperand": archived, "rightOperand": {"kind": "NULL_LITERAL"}}})
        };
        let is_null = json!({"kind": "IF", "condition": test("EQUAL_TO"),
            "thenStatement": call("Build", "run"), "elseStatement": call("Restore", "run")});
        let not_null = json!({"kind": "IF", "condition": test("NOT_EQUAL_TO"),
            "thenStatement": call("Restore", "again"), "elseStatement": call("Build", "again")});
        let unit = json!({"typeDecls": [
            class("Main", &[("main", vec![is_null, not_null])]),
            class("Build", &[("run", vec![]), ("again", vec![])]),
            class("Restore", &[("run", vec![]), ("again", vec![])]),
        ]});
        let reach = reach(&[&unit], "Main").unwrap();
        let method = |class: &str, name: &str| (class.to_string(), name.to_string(), "()void".to_string());
        assert!(reach.methods.contains(&method("Build", "run")), "{reach:?}");
        assert!(reach.methods.contains(&method("Build", "again")), "{reach:?}");
        assert!(!reach.classes.contains("Restore"), "{reach:?}");
    }

    #[test]
    fn a_new_array_of_another_arrays_class_does_not_reach_reflection() {
        let select = |expression: Value, owner: &str, name: &str| {
            json!({"kind": "METHOD_INVOCATION",
                "symbol": {"kind": "METHOD", "owner": owner, "name": name, "type": "()void"},
                "methodSelect": {"kind": "MEMBER_SELECT", "expression": expression}, "arguments": []})
        };
        let array = json!({"kind": "IDENTIFIER", "javaType": "java.lang.Object[]"});
        let component =
            select(select(array, "java.lang.Object", "getClass"), "java.lang.Class", "getComponentType");
        let mut make = select(json!({"kind": "IDENTIFIER"}), "java.lang.reflect.Array", "newInstance");
        make["arguments"] = json!([component, call("Size", "run")["expression"]]);
        let unit = json!({"typeDecls": [
            class("Main", &[("main", vec![json!({"kind": "EXPRESSION_STATEMENT", "expression": make})])]),
            class("java.lang.reflect.Array", &[("newInstance", vec![call("Native", "run")])]),
            class("Native", &[("run", vec![])]),
            class("Size", &[("run", vec![])]),
        ]});
        let reach = reach(&[&unit], "Main").unwrap();
        assert!(!reach.classes.contains("Native"), "{reach:?}");
        assert!(reach.classes.contains("Size"), "{reach:?}");
    }

    #[test]
    fn a_call_of_arrays_that_the_runtime_does_is_not_followed_into_the_jdk() {
        let sort = |arg: &str| {
            json!({"kind": "EXPRESSION_STATEMENT", "expression": {"kind": "METHOD_INVOCATION",
                "symbol": {"kind": "METHOD", "owner": "java.util.Arrays", "name": "sort", "type": "()void", "static": true},
                "arguments": [{"kind": "IDENTIFIER", "javaType": arg}]}})
        };
        let unit = json!({"typeDecls": [
            class("Main", &[("main", vec![sort("int[]")])]),
            class("java.util.Arrays", &[("sort", vec![call("DualPivot", "run")])]),
            class("DualPivot", &[("run", vec![])]),
        ]});
        let runtime = reach(&[&unit], "Main").unwrap();
        assert!(!runtime.classes.contains("DualPivot"), "{runtime:?}");
        let unit = json!({"typeDecls": [
            class("Main", &[("main", vec![sort("Shape[]")])]),
            class("java.util.Arrays", &[("sort", vec![call("DualPivot", "run")])]),
            class("DualPivot", &[("run", vec![])]),
        ]});
        let translated = reach(&[&unit], "Main").unwrap();
        assert!(translated.classes.contains("DualPivot"), "{translated:?}");
    }

    #[test]
    fn serial_persistent_fields_are_not_initialized_unless_read() {
        let describe = json!({"kind": "NEW_CLASS",
            "symbol": {"kind": "CONSTRUCTOR", "owner": "java.io.ObjectStreamField", "name": "<init>",
                       "type": "(java.lang.String,java.lang.Class)void"},
            "arguments": [{"kind": "STRING_LITERAL", "value": "seed"}]});
        let field = |name: &str, init: Value| {
            json!({"kind": "VARIABLE", "name": name, "symbol": {"type": "java.io.ObjectStreamField[]", "static": true},
                   "initializer": {"kind": "NEW_ARRAY", "initializers": [init]}})
        };
        let holder = |fields: Value| {
            let mut holder = class("Holder", &[("run", vec![])]);
            holder["members"].as_array_mut().unwrap().push(fields);
            let unit = json!({"typeDecls": [
                class("Main", &[("main", vec![call("Holder", "run")])]),
                holder,
                class("java.io.ObjectStreamField", &[]),
                class("Describe", &[("run", vec![])]),
            ]});
            reach(&[&unit], "Main").unwrap()
        };
        let key = ("Holder".to_string(), "serialPersistentFields".to_string());
        let reach = holder(field("serialPersistentFields", describe.clone()));
        assert!(!reach.fields.contains(&key), "{reach:?}");
        // Another name, or an element that does something, is initialized with its class.
        let reach = holder(field("persistent", describe));
        assert!(reach.fields.contains(&("Holder".to_string(), "persistent".to_string())), "{reach:?}");
        let reach = holder(field("serialPersistentFields", call("Describe", "run")["expression"].clone()));
        assert!(reach.fields.contains(&key), "{reach:?}");
        assert!(reach.methods.contains(&("Describe".into(), "run".into(), "()void".into())), "{reach:?}");
    }

    #[test]
    fn class_loading_is_not_translated_and_reflect_array_is() {
        assert_eq!(untranslated("java.util.ServiceLoader"), None);
        assert_eq!(untranslated("java.util.ResourceBundle.Control"), None);
        assert_eq!(untranslated("java.lang.ModuleLayer"), None);
        assert_eq!(untranslated("java.lang.invoke.MethodHandles"), Some("reflection"));
        assert_eq!(untranslated("java.lang.runtime.SwitchBootstraps"), Some("reflection"));
        assert_eq!(untranslated("jdk.internal.loader.BootLoader"), Some("class loading"));
        assert_eq!(untranslated("java.lang.reflect.Proxy"), Some("reflection"));
        assert_eq!(untranslated("java.lang.reflect.Method"), None);
        assert_eq!(untranslated("java.lang.reflect.Array"), None);
        assert_eq!(untranslated("java.lang.reflect.Modifier"), None);
        assert_eq!(untranslated("java.lang.reflect.ParameterizedType"), None);
        assert_eq!(untranslated("java.security.PrivateKey"), None);
        assert_eq!(untranslated("java.security.KeyPair"), Some("security"));
        assert_eq!(untranslated("java.util.ArrayList"), None);
        // Reachability stops at method handles, and goes into `Array`.
        let unit = json!({"typeDecls": [
            class("Main", &[("main", vec![call("java.lang.invoke.MethodHandles", "lookup"), call("java.lang.reflect.Array", "newInstance")])]),
            class("java.lang.invoke.MethodHandles", &[("lookup", vec![call("Provider", "run")])]),
            class("java.lang.reflect.Array", &[("newInstance", vec![call("Other", "run")])]),
            class("Provider", &[("run", vec![])]),
            class("Other", &[("run", vec![])]),
        ]});
        let reach = reach(&[&unit], "Main").unwrap();
        let method = |class: &str| (class.to_string(), "run".to_string(), "()void".to_string());
        assert!(!reach.classes.contains("java.lang.invoke.MethodHandles"), "{reach:?}");
        assert!(!reach.methods.contains(&method("Provider")), "{reach:?}");
        assert!(reach.classes.contains("java.lang.reflect.Array"), "{reach:?}");
        assert!(reach.methods.contains(&method("Other")), "{reach:?}");
    }

    #[test]
    fn an_anonymous_class_is_reached_by_dispatch_not_with_the_code_that_creates_it() {
        let method = |name: &str, statements: Vec<Value>, overrides: Option<&str>| {
            json!({"kind": "METHOD", "name": name, "symbol": {"type": "()void", "static": false,
                       "overrides": overrides.map(|o| vec![json!({"owner": o, "type": "()void"})])},
                   "body": {"kind": "BLOCK", "statements": statements}})
        };
        // `new Base() { void used() { Used.run(); } void unused() { Unused.run(); } }.used()`
        let anonymous = json!({"kind": "CLASS", "symbol": {"type": "Main$1"},
        "extendsClause": {"kind": "IDENTIFIER", "symbol": {"type": "Base"}},
        "members": [
            method("<init>", vec![], None),
            method("used", vec![call("Used", "run")], Some("Base")),
            method("unused", vec![call("Unused", "run")], Some("Base")),
        ]});
        let created = json!({"kind": "NEW_CLASS", "classBody": anonymous,
            "symbol": {"kind": "CONSTRUCTOR", "owner": "Main$1", "name": "<init>", "type": "()void"}});
        let used = json!({"kind": "EXPRESSION_STATEMENT", "expression": {"kind": "METHOD_INVOCATION",
            "methodSelect": {"kind": "MEMBER_SELECT", "expression": created},
            "symbol": {"kind": "METHOD", "owner": "Base", "name": "used", "type": "()void", "static": false}}});
        let base = json!({"kind": "CLASS", "symbol": {"type": "Base"}, "members": [
            method("<init>", vec![], None),
            method("used", vec![], None),
            method("unused", vec![], None),
        ]});
        let unit = json!({"typeDecls": [
            class("Main", &[("main", vec![used])]),
            base,
            class("Used", &[("run", vec![])]),
            class("Unused", &[("run", vec![])]),
        ]});
        let reach = reach(&[&unit], "Main").unwrap();
        let key = |class: &str, name: &str| (class.to_string(), name.to_string(), "()void".to_string());
        assert!(reach.instantiated.contains("Main$1"), "{reach:?}");
        assert!(reach.methods.contains(&key("Main$1", "used")), "{reach:?}");
        assert!(reach.methods.contains(&key("Used", "run")), "{reach:?}");
        assert!(!reach.methods.contains(&key("Main$1", "unused")), "{reach:?}");
        assert!(!reach.classes.contains("Unused"), "{reach:?}");
    }

    /// A pipeline in the shape of `java.util.stream.AbstractPipeline`: a `parallel` field that its
    /// constructor sets from its argument and `parallel()` sets to true, `isParallel()`, and an
    /// `evaluate()` that runs `Par.run()` when the pipeline is parallel and `Seq.run()` when not.
    fn pipeline_class() -> Value {
        let pipeline = "java.util.stream.AbstractPipeline";
        let field = json!({"kind": "MEMBER_SELECT",
            "symbol": {"kind": "FIELD", "owner": pipeline, "name": "parallel", "static": false}});
        let assign = |value: Value| {
            json!({"kind": "EXPRESSION_STATEMENT", "expression": {
                "kind": "ASSIGNMENT", "variable": field.clone(), "expression": value}})
        };
        let parameter = json!({"kind": "IDENTIFIER", "name": "parallel",
            "symbol": {"kind": "PARAMETER", "name": "parallel", "type": "boolean"}});
        let is_parallel = json!({"kind": "METHOD_INVOCATION",
            "symbol": {"kind": "METHOD", "owner": pipeline, "name": "isParallel", "type": "()boolean"}});
        let evaluate = json!({"kind": "CONDITIONAL_EXPRESSION", "condition": is_parallel,
            "trueExpression": call("Par", "run")["expression"],
            "falseExpression": call("Seq", "run")["expression"]});
        json!({"kind": "CLASS", "symbol": {"type": pipeline}, "members": [
            {"kind": "VARIABLE", "name": "parallel", "symbol": {"type": "boolean"}},
            {"kind": "METHOD", "name": "<init>", "symbol": {"type": "(boolean)void", "static": false},
             "parameters": [{"kind": "VARIABLE", "name": "parallel", "symbol": {"type": "boolean"}}],
             "body": {"kind": "BLOCK", "statements": [assign(parameter)]}},
            {"kind": "METHOD", "name": "parallel", "symbol": {"type": "()void", "static": true},
             "body": {"kind": "BLOCK", "statements": [assign(json!({"kind": "BOOLEAN_LITERAL", "value": true}))]}},
            {"kind": "METHOD", "name": "evaluate", "symbol": {"type": "()void", "static": true},
             "body": {"kind": "BLOCK", "statements": [{"kind": "EXPRESSION_STATEMENT", "expression": evaluate}]}},
        ]})
    }

    /// `new AbstractPipeline(parallel)`, as a statement.
    fn new_pipeline(parallel: bool) -> Value {
        json!({"kind": "EXPRESSION_STATEMENT", "expression": {"kind": "NEW_CLASS",
            "symbol": {"kind": "CONSTRUCTOR", "owner": "java.util.stream.AbstractPipeline",
                       "name": "<init>", "type": "(boolean)void"},
            "arguments": [{"kind": "BOOLEAN_LITERAL", "value": parallel}]}})
    }

    #[test]
    fn sequential_streams_do_not_reach_the_parallel_paths() {
        let pipeline = "java.util.stream.AbstractPipeline";
        let reach_with = |statements: Vec<Value>| {
            let unit = json!({"typeDecls": [
                class("Main", &[("main", statements)]),
                pipeline_class(),
                class("Par", &[("run", vec![])]),
                class("Seq", &[("run", vec![])]),
            ]});
            reach(&[&unit], "Main").unwrap()
        };
        let method = |class: &str| (class.to_string(), "run".to_string(), "()void".to_string());
        let sequential = reach_with(vec![new_pipeline(false), call(pipeline, "evaluate")]);
        assert!(sequential.sequential);
        assert!(sequential.methods.contains(&method("Seq")), "{sequential:?}");
        assert!(!sequential.methods.contains(&method("Par")), "{sequential:?}");
        // A pipeline created parallel, or made parallel, reaches both.
        for statements in [
            vec![new_pipeline(true), call(pipeline, "evaluate")],
            vec![new_pipeline(false), call(pipeline, "parallel"), call(pipeline, "evaluate")],
        ] {
            let parallel = reach_with(statements);
            assert!(!parallel.sequential);
            assert!(parallel.methods.contains(&method("Par")), "{parallel:?}");
            assert!(parallel.methods.contains(&method("Seq")), "{parallel:?}");
        }
    }

    #[test]
    fn a_sequential_program_has_no_nodes_with_children() {
        let node = "java.util.stream.Node";
        let instance = |name: &str, ty: &str, overrides: Option<&str>| {
            json!({"kind": "METHOD", "name": name, "symbol": {"type": ty, "static": false,
                       "overrides": overrides.map(|o| vec![json!({"owner": o, "type": ty})])},
                   "body": {"kind": "BLOCK", "statements": []}})
        };
        // `Nodes.flatten`: `if (node.getChildCount() > 0) Par.run(); else Seq.run();`
        let has_children = json!({"kind": "GREATER_THAN",
            "leftOperand": {"kind": "METHOD_INVOCATION",
                "symbol": {"kind": "METHOD", "owner": node, "name": "getChildCount", "type": "()int"}},
            "rightOperand": {"kind": "INT_LITERAL", "value": "0"}});
        let flatten = json!({"kind": "IF", "condition": has_children,
            "thenStatement": call("Par", "run"), "elseStatement": call("Seq", "run")});
        // A class whose nodes have children, as `Nodes.ConcNode`.
        let conc = json!({"kind": "CLASS", "symbol": {"type": "Conc"},
            "implementsClause": [{"kind": "IDENTIFIER", "symbol": {"type": node}}],
            "members": [instance("<init>", "()void", None), instance("getChildCount", "()int", Some(node))]});
        let new_conc = json!({"kind": "EXPRESSION_STATEMENT", "expression": {"kind": "NEW_CLASS",
            "symbol": {"kind": "CONSTRUCTOR", "owner": "Conc", "name": "<init>", "type": "()void"}}});
        let reach_with = |statements: Vec<Value>| {
            let unit = json!({"typeDecls": [
                class("Main", &[("main", statements)]),
                {"kind": "INTERFACE", "symbol": {"type": node}, "members": [instance("getChildCount", "()int", None)]},
                conc.clone(),
                class("Par", &[("run", vec![])]),
                class("Seq", &[("run", vec![])]),
            ]});
            reach(&[&unit], "Main").unwrap()
        };
        let method = |class: &str| (class.to_string(), "run".to_string(), "()void".to_string());
        let sequential = reach_with(vec![flatten.clone()]);
        assert!(sequential.sequential);
        assert!(sequential.methods.contains(&method("Seq")), "{sequential:?}");
        assert!(!sequential.methods.contains(&method("Par")), "{sequential:?}");
        let concatenated = reach_with(vec![new_conc, flatten]);
        assert!(!concatenated.sequential);
        assert!(concatenated.methods.contains(&method("Par")), "{concatenated:?}");
    }
}
