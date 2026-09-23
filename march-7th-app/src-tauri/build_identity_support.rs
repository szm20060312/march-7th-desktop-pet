// Keep this list aligned with G7-BUILD-IDENTITY.md. The same scope drives Git
// status and Cargo watches; never watch the app root (it contains build caches).
use std::{
    env,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

// Project convention: keep both automatic desktop overrides present, even if
// empty. Missing optional paths cannot be watched without perpetual rebuilds.
const PLATFORM_CONFIGS: &[(&str, &str)] = &[
    ("src-tauri/tauri.windows.conf.json", "windows-config"),
    ("src-tauri/tauri.macos.conf.json", "macos-config"),
];

// Labels are fixed public vocabulary, never derived from filesystem names.
// Directory pathspecs require a trailing slash: ignored matching must not
// treat a similarly prefixed sibling (e.g. the native target cache) as input.
const INPUTS: &[(&str, &str)] = &[
    ("src/", "frontend-source"),
    ("public/", "public-assets"),
    ("index.html", "main-entry"),
    ("settings.html", "settings-entry"),
    ("reminder.html", "reminder-entry"),
    ("package.json", "frontend-manifest"),
    ("pnpm-lock.yaml", "frontend-lock"),
    ("tsconfig.json", "typescript-config"),
    ("vite.config.ts", "vite-config"),
    (".gitignore", "app-ignore"),
    ("src-tauri/.gitignore", "native-ignore"),
    ("src-tauri/src/", "native-source"),
    ("src-tauri/icons/", "native-icons"),
    ("src-tauri/capabilities/", "native-capabilities"),
    ("src-tauri/Cargo.toml", "native-manifest"),
    ("src-tauri/Cargo.lock", "native-lock"),
    ("src-tauri/tauri.conf.json", "tauri-config"),
    ("src-tauri/build.rs", "native-build-script"),
    (
        "src-tauri/build_identity_support.rs",
        "identity-build-support",
    ),
];

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        // Ambient Git redirection must not assign another checkout's identity.
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(args)
        .output()
        .ok()?;
    output.status.success().then_some(())?;
    String::from_utf8(output.stdout).ok()
}

fn watch(path: &Path) {
    // Watching a nonexistent packed ref would force every Cargo run to rebuild.
    if path.exists() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

fn git_watch(root: &Path, name: &str) {
    if let Some(path) = git(
        root,
        &["rev-parse", "--path-format=absolute", "--git-path", name],
    ) {
        watch(Path::new(path.trim()));
    }
}

fn diagnose_scopes(root: &Path, scopes: &[(String, &str)]) {
    for (input, label) in scopes {
        // Quoted porcelain keeps filenames (including newline names) inside one
        // record. Inspect only its status prefix; never print any Git output.
        let status = git(
            root,
            &[
                "-c",
                "core.quotePath=true",
                "status",
                "--porcelain=v1",
                "--untracked-files=all",
                "--ignored=matching",
                "--",
                input,
            ],
        );
        let Some(status) = status else {
            println!("cargo:warning=build-input-diagnostic scope={label} unavailable");
            continue;
        };
        let mut changed = [false; 3];
        for line in status.lines() {
            match line.get(..2) {
                Some("??") => changed[1] = true,
                Some("!!") => changed[2] = true,
                Some(_) => changed[0] = true,
                None => {}
            }
        }
        let categories = ["tracked", "untracked", "ignored"]
            .into_iter()
            .zip(changed)
            .filter_map(|(category, present)| present.then_some(category))
            .collect::<Vec<_>>();
        if !categories.is_empty() {
            println!(
                "cargo:warning=build-input-diagnostic scope={label} categories={}",
                categories.join(",")
            );
        }
    }
}

fn safe_ignored_summary(line: &str) -> bool {
    let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
    if line.len() > 1024
        || line.contains('\n')
        || line.contains('\r')
        || !(6..=7).contains(&fields.len())
        || fields[0] != "ignored-input"
        || fields[1] != "phase=identity-sample"
    {
        return false;
    }
    if fields[2] == "state=unavailable" {
        return fields.len() == 7
            && fields[3..6] == ["total=unknown", "groups=unknown", "overflow=no"]
            && [
                "failureStage=phase",
                "failureStage=root",
                "failureStage=status-query",
                "failureStage=status-format",
                "failureStage=limit",
                "failureStage=ignore-query",
                "failureStage=ignore-format",
                "failureStage=path",
                "failureStage=metadata",
                "failureStage=helper",
            ]
            .contains(&fields[6]);
    }
    if fields.len() != 6
        || fields[2] != "state=ok"
        || !["total=zero", "total=one", "total=few", "total=many"].contains(&fields[3])
        || !["overflow=no", "overflow=yes"].contains(&fields[5])
    {
        return false;
    }
    if fields[4] == "groups=none" {
        return fields[3] == "total=zero" && fields[5] == "overflow=no";
    }
    let Some(groups) = fields[4].strip_prefix("groups=") else {
        return false;
    };
    let groups = groups.split(',').collect::<Vec<_>>();
    groups.len() <= 8
        && groups.iter().all(|group| {
            let parts = group.split(':').collect::<Vec<_>>();
            parts.len() == 4
                && [
                    "root-rules",
                    "app-rules",
                    "nested-rules",
                    "internal-external-rules",
                ]
                .contains(&parts[0])
                && [
                    "dependency-directory",
                    "build-output",
                    "logs",
                    "local-config",
                    "editor-metadata",
                    "other",
                ]
                .contains(&parts[1])
                && ["file", "directory", "link", "other"].contains(&parts[2])
                && ["one", "few", "many"].contains(&parts[3])
        })
}

fn diagnose_ignored_frontend(root: &Path) {
    let script = root.join("scripts/diagnose-frontend-ignore.mjs");
    watch(&script);
    let summary = bounded_ignored_query(root, &script);
    let line = summary.as_deref().map(str::trim).filter(|line| safe_ignored_summary(line))
        .unwrap_or("ignored-input phase=identity-sample state=unavailable total=unknown groups=unknown overflow=no failureStage=helper");
    println!("cargo:warning={line}");
}

fn bounded_ignored_query(root: &Path, script: &Path) -> Option<String> {
    let mut child = Command::new("node")
        .current_dir(root)
        .arg(script)
        .arg("identity-sample")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let (sender, receiver) = mpsc::channel();
    // Keep at most one bounded line even if a broken process prints forever.
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.take(1025).read_to_end(&mut bytes).map(|_| bytes);
        let _ = sender.send(result);
    });
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                let bytes = receiver
                    .recv_timeout(Duration::from_millis(500))
                    .ok()?
                    .ok()?;
                return (bytes.len() <= 1024)
                    .then(|| String::from_utf8(bytes).ok())
                    .flatten();
            }
            Ok(Some(_)) => return None,
            Ok(None) if started.elapsed() < Duration::from_secs(20) => {
                thread::sleep(Duration::from_millis(20))
            }
            _ => {
                let _ = child.kill();
                // Even a failed termination must not turn an optional diagnostic
                // into an unbounded wait. The build process will exit normally.
                for _ in 0..25 {
                    if !matches!(child.try_wait(), Ok(None)) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                return None;
            }
        }
    }
}

pub fn embed() {
    let manifest = env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory");
    let root = Path::new(&manifest)
        .parent()
        .expect("application directory");
    for (config, _) in PLATFORM_CONFIGS {
        assert!(
            root.join(config).is_file(),
            "Missing required project build file: {config}; restore the tracked platform config (an empty object is valid)"
        );
    }
    for (input, _) in INPUTS.iter().chain(PLATFORM_CONFIGS) {
        watch(&root.join(input));
    }
    println!("cargo:rerun-if-env-changed=PATH");
    println!("cargo:rerun-if-env-changed=MARCH_BUILD_INPUT_DIAGNOSTICS");
    for control in ["HEAD", "index", "packed-refs", "config", "info/exclude"] {
        git_watch(root, control);
    }
    if let Some(reference) = git(root, &["symbolic-ref", "-q", "HEAD"]) {
        git_watch(root, reference.trim());
        // Packed branches can acquire a new loose ref without touching HEAD or
        // packed-refs. Watch the existing ref directory, never the Git root.
        if let Some(path) = git(
            root,
            &[
                "rev-parse",
                "--path-format=absolute",
                "--git-path",
                reference.trim(),
            ],
        ) {
            if let Some(parent) = Path::new(path.trim())
                .ancestors()
                .skip(1)
                .find(|parent| parent.is_dir())
            {
                watch(parent);
            }
        }
    }
    // A linked worktree's .git is a file, not a directory.
    for ancestor in root.ancestors() {
        let marker = ancestor.join(".git");
        if marker.is_file() {
            watch(&marker);
            break;
        }
        if marker.is_dir() {
            break;
        }
    }
    let mut scope = INPUTS
        .iter()
        .chain(PLATFORM_CONFIGS)
        .map(|(input, label)| ((*input).to_owned(), *label))
        .collect::<Vec<_>>();
    if let Some(repo) = git(root, &["rev-parse", "--show-toplevel"]) {
        // Repository attributes affect the bytes Git compares (e.g. CRLF).
        for (name, label) in [
            (".gitattributes", "repository-attributes"),
            (".gitignore", "repository-ignore"),
        ] {
            let file = Path::new(repo.trim()).join(name);
            watch(&file);
            scope.push((file.to_string_lossy().into_owned(), label));
        }
    }
    let source = (|| {
        // An untracked export inside an unrelated repository must stay unknown.
        git(
            root,
            &["ls-files", "--error-unmatch", "--", "src-tauri/Cargo.toml"],
        )?;
        let commit = git(root, &["rev-parse", "--verify", "HEAD^{commit}"])?;
        let commit = commit.trim();
        if commit.len() != 40
            || !commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return None;
        }
        let mut args = vec![
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--ignored=matching",
            "--",
        ];
        args.extend(scope.iter().map(|(input, _)| input.as_str()));
        let changed = git(root, &args)?;
        Some((
            commit.to_owned(),
            if changed.is_empty() {
                "clean"
            } else {
                "modified"
            },
        ))
    })();
    let (commit, state) = source.unwrap_or_else(|| (String::new(), "unknown"));
    if env::var("MARCH_BUILD_INPUT_DIAGNOSTICS").as_deref() == Ok("1") {
        if state == "modified" {
            diagnose_scopes(root, &scope);
        }
        diagnose_ignored_frontend(root);
    }
    println!(
        "cargo:rustc-env=MARCH_BUILD_TARGET={}",
        env::var("TARGET").expect("Cargo target")
    );
    println!("cargo:rustc-env=MARCH_SOURCE_COMMIT={commit}");
    println!("cargo:rustc-env=MARCH_SOURCE_STATE={state}");
}
