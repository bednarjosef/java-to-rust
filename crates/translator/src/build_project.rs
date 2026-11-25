//! `--project DIR`: a Maven or Gradle project as the program to translate. The project's own
//! build tool says what the program is made of, as it would to build it, so dependency versions,
//! repositories and multi-module layouts are resolved as the project resolves them:
//!
//! - **Gradle** (`build.gradle`, `build.gradle.kts` or `settings.gradle(.kts)`): the project's
//!   `gradlew`, or `gradle` on `PATH`, runs the task that `build-tools/model.init.gradle` adds to
//!   every project of the build. It reports each project's main Java and resource directories,
//!   the jars of the external dependencies on its runtime class path, their sources jars, and the
//!   application plugin's main class.
//! - **Maven** (`pom.xml`): the project's `mvnw`, or `mvn` on `PATH`, compiles the project (so
//!   that modules that depend on each other resolve), downloads the dependencies' sources jars
//!   (`dependency:sources`) and writes each module's runtime class path
//!   (`dependency:build-classpath`) to `target/java-to-rust-classpath.txt`. Each module's sources
//!   are its `src/main/java`, its resources `src/main/resources`; a sources jar is the one next to
//!   the dependency's jar in the local repository; the main class is the first `<mainClass>` of
//!   the top `pom.xml` (as the exec or jar plugin is given it), if any.
//!
//! The program's sources are those of every project or module of the build. Its libraries are its
//! resource directories, then for each dependency its sources and its resources (the jar's files
//! other than classes, such as `META-INF/services`), each unpacked once into
//! `libraries/<key>` of the translator's cache. A dependency without a sources jar contributes its
//! resources only, and is reported: code that uses it is rejected (with `--keep-going`, it throws
//! `NoClassDefFoundError` where it runs).

use std::path::{Path, PathBuf};
use std::process::Command;

/// What the translator takes from a build.
#[derive(Debug, Default)]
pub struct BuildProject {
    pub sources: Vec<PathBuf>,
    pub libraries: Vec<PathBuf>,
    pub main: Option<String>,
    /// The dependencies whose repositories have no sources jar, by their coordinates or file.
    pub without_sources: Vec<String>,
}

/// A dependency of the build: its coordinates (or file name), its jar, and its sources jar.
struct Dependency {
    name: String,
    jar: PathBuf,
    sources: Option<PathBuf>,
}

/// The program that the Maven or Gradle project in `dir` builds.
pub fn read(dir: &Path) -> Result<BuildProject, String> {
    let dir = dir.canonicalize().map_err(|e| format!("--project {}: {e}", dir.display()))?;
    let has = |name: &str| dir.join(name).is_file();
    let (sources, resources, dependencies, main) = if has("pom.xml") {
        maven(&dir)?
    } else if ["build.gradle", "build.gradle.kts", "settings.gradle", "settings.gradle.kts"]
        .iter()
        .any(|f| has(f))
    {
        gradle(&dir)?
    } else {
        return Err(format!(
            "--project {}: no pom.xml, build.gradle(.kts) or settings.gradle(.kts) there",
            dir.display()
        ));
    };
    if sources.is_empty() {
        return Err(format!(
            "--project {}: the build reports no Java sources (Maven's src/main/java, Gradle's main source set)",
            dir.display()
        ));
    }
    let mut project = BuildProject { sources, main, ..BuildProject::default() };
    project.libraries.extend(resources);
    let mut seen = std::collections::BTreeSet::new();
    for dependency in dependencies {
        if !seen.insert(dependency.jar.clone()) {
            continue;
        }
        let unpacked = crate::embedded::cache().join("libraries").join(key(&dependency.jar)?);
        match &dependency.sources {
            Some(sources) => {
                let root = unpacked.join("sources");
                crate::jdk_sources::unpack(sources, &root, |p| p.extension().is_some_and(|e| e == "java"))?;
                project.libraries.push(root);
            }
            None => project.without_sources.push(dependency.name.clone()),
        }
        let root = unpacked.join("resources");
        crate::jdk_sources::unpack(&dependency.jar, &root, |p| {
            !p.extension().is_some_and(|e| e == "class" || e == "java")
                && !p.starts_with("META-INF/versions")
                && !p.file_name().is_some_and(|f| f == "MANIFEST.MF" || f.to_string_lossy().ends_with(".SF"))
        })?;
        project.libraries.push(root);
    }
    Ok(project)
}

type Model = (Vec<PathBuf>, Vec<PathBuf>, Vec<Dependency>, Option<String>);

fn gradle(dir: &Path) -> Result<Model, String> {
    let script = crate::embedded::cache().join("model.init.gradle");
    let text = include_str!("../build-tools/model.init.gradle");
    if std::fs::read_to_string(&script).ok().as_deref() != Some(text) {
        std::fs::create_dir_all(crate::embedded::cache()).map_err(|e| e.to_string())?;
        std::fs::write(&script, text).map_err(|e| format!("{}: {e}", script.display()))?;
    }
    let out = std::env::temp_dir().join(format!("java-to-rust-gradle-model-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&out);
    let wrapper = dir.join("gradlew");
    let mut command = Command::new(if wrapper.is_file() { wrapper.as_os_str() } else { "gradle".as_ref() });
    command
        .current_dir(dir)
        .args(["-q", "--no-configuration-cache", "-I"])
        .arg(&script)
        .arg(format!("-PjavaToRustOut={}", out.display()))
        .arg("javaToRustModel");
    run("gradle", &mut command)?;
    let text = std::fs::read_to_string(&out).unwrap_or_default();
    let _ = std::fs::remove_file(&out);
    let mut sources = Vec::new();
    let mut resources = Vec::new();
    let mut jars: Vec<(String, PathBuf)> = Vec::new();
    let mut sources_jars = std::collections::BTreeMap::new();
    let mut main = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        match fields[..] {
            ["source", path] => push_new(&mut sources, PathBuf::from(path)),
            ["resources", path] => push_new(&mut resources, PathBuf::from(path)),
            ["jar", name, path] => jars.push((name.to_string(), PathBuf::from(path))),
            ["sources", name, path] => {
                sources_jars.insert(name.to_string(), PathBuf::from(path));
            }
            ["main", class] => main = main.or(Some(class.to_string())),
            _ => {}
        }
    }
    let dependencies = jars
        .into_iter()
        .map(|(name, jar)| Dependency { sources: sources_jars.get(&name).cloned(), name, jar })
        .collect();
    Ok((sources, resources, dependencies, main))
}

fn maven(dir: &Path) -> Result<Model, String> {
    let wrapper = dir.join("mvnw");
    let mut command = Command::new(if wrapper.is_file() { wrapper.as_os_str() } else { "mvn".as_ref() });
    command.current_dir(dir).args([
        "-q",
        "-B",
        "compile",
        "dependency:sources",
        "dependency:build-classpath",
        "-Dmdep.outputFile=target/java-to-rust-classpath.txt",
        "-Dmdep.includeScope=runtime",
        "-DincludeScope=runtime",
    ]);
    run("mvn", &mut command)?;
    let mut modules = Vec::new();
    maven_modules(dir, &mut modules);
    let mut sources = Vec::new();
    let mut resources = Vec::new();
    let mut dependencies = Vec::new();
    for module in &modules {
        let java = module.join("src/main/java");
        if java.is_dir() {
            push_new(&mut sources, java);
        }
        let res = module.join("src/main/resources");
        if res.is_dir() {
            push_new(&mut resources, res);
        }
        let classpath =
            std::fs::read_to_string(module.join("target/java-to-rust-classpath.txt")).unwrap_or_default();
        for entry in std::env::split_paths(classpath.trim()) {
            // Another module of the build, by its classes: its sources are among the program's.
            if entry.as_os_str().is_empty() || entry.is_dir() {
                continue;
            }
            let name = entry.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let sources_jar =
                entry.with_file_name(name.trim_end_matches(".jar").to_string() + "-sources.jar");
            dependencies.push(Dependency {
                sources: sources_jar.is_file().then_some(sources_jar),
                name: name.trim_end_matches(".jar").to_string(),
                jar: entry,
            });
        }
    }
    let pom = std::fs::read_to_string(dir.join("pom.xml")).unwrap_or_default();
    let main = pom
        .split("<mainClass>")
        .nth(1)
        .and_then(|rest| rest.split("</mainClass>").next())
        .map(|class| class.trim().to_string())
        .filter(|class| !class.is_empty() && !class.contains("${"));
    Ok((sources, resources, dependencies, main))
}

/// The directories under `dir` (itself included) of the modules of a Maven build that the build
/// just ran: each with a `pom.xml` and the class path it wrote.
fn maven_modules(dir: &Path, out: &mut Vec<PathBuf>) {
    if dir.join("pom.xml").is_file() && dir.join("target/java-to-rust-classpath.txt").is_file() {
        out.push(dir.to_path_buf());
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    for sub in dirs {
        let name = sub.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !name.starts_with('.') && name != "target" && name != "src" && name != "node_modules" {
            maven_modules(&sub, out);
        }
    }
}

/// Runs the build tool, with its error output shown if it fails.
fn run(tool: &str, command: &mut Command) -> Result<(), String> {
    let output = command.output().map_err(|e| {
        format!(
            "--project: cannot run {:?} ({e}); install {tool} or add its wrapper to the project",
            command.get_program()
        )
    })?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let tail: Vec<&str> =
        stdout.lines().chain(stderr.lines()).filter(|l| !l.starts_with("WARNING:")).collect();
    let tail = tail[tail.len().saturating_sub(30)..].join("\n");
    Err(format!("--project: {tool} failed ({}):\n{tail}", output.status))
}

fn push_new(list: &mut Vec<PathBuf>, path: PathBuf) {
    if !list.contains(&path) {
        list.push(path);
    }
}

/// The directory name of a jar's unpacked files: a hash of its path, size and time, so a jar
/// that is replaced is unpacked anew.
fn key(jar: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(jar).map_err(|e| format!("{}: {e}", jar.display()))?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let stem = jar.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    for byte in jar
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .copied()
        .chain(meta.len().to_le_bytes())
        .chain(modified.to_le_bytes())
    {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
    }
    Ok(format!("{stem}-{hash:016x}"))
}
