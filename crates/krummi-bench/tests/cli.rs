//! The command's checks before the Voron build (AC-K9): every row of *Validaciones y mensajes* up to the work
//! folder. Each case shows its exact message as the only line of standard error, ends with its code, prints nothing
//! on standard output, writes no report and leaves the current and the work folders as they were.
//!
//! The rows after the .NET check (the work folder) need a .NET 10 SDK on the `PATH`, so they are ignored by default
//! and run in the CI job that has it (`--include-ignored`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

/// A folder that passes the RavenDB check (design D3) without the real source.
fn fixture_ravendb(parent: &Path) -> PathBuf {
    let root = parent.join("ravendb");
    fs::create_dir_all(root.join("src/Voron")).unwrap();
    fs::write(root.join("src/Voron/Voron.csproj"), "<Project />").unwrap();
    fs::write(
        root.join("src/CommonAssemblyInfo.cs"),
        "[assembly: AssemblyInformationalVersion(\"7.2.7\")]",
    )
    .unwrap();
    root
}

/// Every path under `dir`, relative and sorted, folders included.
fn listing(dir: &Path) -> Vec<PathBuf> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            out.push(path.strip_prefix(root).unwrap().to_path_buf());
            if path.is_dir() {
                walk(root, &path, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// A run of the command in a fresh current folder, next to a fresh work folder and a RavenDB fixture.
struct Case {
    /// The current folder of the command, where the default report would go.
    cwd: TempDir,
    /// A folder with the RavenDB fixture (`ravendb`) and room for other paths of the case.
    side: TempDir,
    /// The work folder, passed with `--carpeta` when the case asks for it.
    work: TempDir,
}

impl Case {
    fn new() -> Case {
        let case = Case {
            cwd: tempfile::tempdir().unwrap(),
            side: tempfile::tempdir().unwrap(),
            work: tempfile::tempdir().unwrap(),
        };
        fixture_ravendb(case.side.path());
        case
    }

    fn ravendb(&self) -> String {
        self.side.path().join("ravendb").display().to_string()
    }

    fn work(&self) -> String {
        self.work.path().display().to_string()
    }

    /// Runs the command with `args` and checks the failure: exactly `message` on standard error, `code`, nothing
    /// on standard output, and the current, side and work folders unchanged.
    fn fails(&self, args: &[&str], path: Option<&Path>, message: &str, code: i32) {
        let before = self.listings();
        let mut command = Command::new(env!("CARGO_BIN_EXE_krummi-bench"));
        command
            .args(args)
            .current_dir(self.cwd.path())
            .env_remove("KRUMMI_BENCH_ROLE")
            .env_remove("KRUMMI_BENCH_FAIL");
        if let Some(path) = path {
            command.env("PATH", path);
        }
        let out: Output = command.output().expect("the command starts");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(stderr.trim_end_matches(['\r', '\n']), message, "{args:?}");
        assert_eq!(stderr.lines().count(), 1, "{args:?}: {stderr}");
        assert_eq!(out.status.code(), Some(code), "{args:?}: {stderr}");
        assert!(
            out.stdout.is_empty(),
            "{args:?}: stdout {}",
            String::from_utf8_lossy(&out.stdout)
        );
        assert_eq!(self.listings(), before, "{args:?}: a folder changed");
    }

    fn listings(&self) -> [Vec<PathBuf>; 3] {
        [
            listing(self.cwd.path()),
            listing(self.side.path()),
            listing(self.work.path()),
        ]
    }
}

#[test]
fn an_invalid_argument_is_named_and_ends_with_code_2() {
    let case = Case::new();
    let rv = case.ravendb();
    let work = case.work();
    let invalid = |o: &str| format!("Opción no válida: {o}.");
    let cases: [(Vec<&str>, &str); 7] = [
        (vec!["--ravendb", &rv, "--otra"], "--otra"),
        (vec!["--ravendb=x"], "--ravendb=x"),
        (vec!["suelto", "--ravendb", &rv], "suelto"),
        (
            vec!["--ravendb", &rv, "--reducido", "--reducido"],
            "--reducido",
        ),
        (
            vec!["--ravendb", &rv, "--carpeta", &work, "--carpeta", &work],
            "--carpeta",
        ),
        (vec!["--ravendb", &rv, "--informe"], "--informe"),
        (
            vec!["--carpeta", "--informe", "x.md", "--ravendb", &rv],
            "--carpeta",
        ),
    ];
    for (args, shown) in cases {
        case.fails(&args, None, &invalid(shown), 2);
    }
}

#[test]
fn ravendb_is_required() {
    let case = Case::new();
    case.fails(
        &["--reducido", "--carpeta", &case.work()],
        None,
        "Falta la opción --ravendb con la carpeta del código de RavenDB 7.2.7.",
        2,
    );
}

#[test]
fn the_report_must_not_exist_also_as_a_folder() {
    let case = Case::new();
    let file = case.side.path().join("existe.md");
    fs::write(&file, "informe anterior").unwrap();
    let folder = case.side.path().join("carpeta.md");
    fs::create_dir(&folder).unwrap();
    for target in [&file, &folder] {
        let shown = target.display().to_string();
        case.fails(
            &[
                "--ravendb",
                &case.ravendb(),
                "--carpeta",
                &case.work(),
                "--informe",
                &shown,
            ],
            None,
            &format!("El destino {shown} ya existe."),
            1,
        );
    }
    assert_eq!(fs::read_to_string(&file).unwrap(), "informe anterior");
    // The report wins over a bad --ravendb: the checks go in the order of the table.
    let shown = file.display().to_string();
    case.fails(
        &["--informe", &shown, "--ravendb", "no-existe"],
        None,
        &format!("El destino {shown} ya existe."),
        1,
    );
}

#[test]
fn the_folder_of_the_report_must_exist() {
    let case = Case::new();
    let folder = case.side.path().join("nada");
    let report = folder.join("r.md");
    case.fails(
        &[
            "--ravendb",
            &case.ravendb(),
            "--informe",
            &report.display().to_string(),
        ],
        None,
        &format!(
            "No se puede usar la carpeta {}: no existe.",
            folder.display()
        ),
        1,
    );
}

#[test]
fn ravendb_must_hold_the_source_of_7_2_7() {
    let case = Case::new();
    let missing = case.side.path().join("no-existe").display().to_string();
    let empty = case.side.path().join("vacia");
    fs::create_dir(&empty).unwrap();
    let empty = empty.display().to_string();
    for folder in [&missing, &empty] {
        case.fails(
            &["--ravendb", folder, "--carpeta", &case.work()],
            None,
            &format!("La carpeta {folder} no contiene el código de RavenDB 7.2.7."),
            1,
        );
    }
    // A bad --ravendb wins over a missing .NET SDK.
    let no_dotnet = tempfile::tempdir().unwrap();
    case.fails(
        &["--ravendb", &empty],
        Some(no_dotnet.path()),
        &format!("La carpeta {empty} no contiene el código de RavenDB 7.2.7."),
        1,
    );
}

#[test]
fn the_dotnet_10_sdk_is_required() {
    let case = Case::new();
    // `dotnet` is not found in an empty PATH.
    let no_dotnet = tempfile::tempdir().unwrap();
    case.fails(
        &["--ravendb", &case.ravendb(), "--carpeta", &case.work()],
        Some(no_dotnet.path()),
        "No se encuentra el SDK de .NET 10, necesario para medir Voron.",
        1,
    );
}

#[test]
#[ignore = "needs .NET 10"]
fn the_work_folder_must_exist() {
    let case = Case::new();
    let missing = case.side.path().join("no").display().to_string();
    case.fails(
        &["--ravendb", &case.ravendb(), "--carpeta", &missing],
        None,
        &format!("La carpeta {missing} no existe."),
        1,
    );
}

#[test]
#[ignore = "needs .NET 10"]
fn the_work_folder_must_be_a_folder() {
    let case = Case::new();
    let file = case.side.path().join("archivo");
    fs::write(&file, "").unwrap();
    let file = file.display().to_string();
    case.fails(
        &["--ravendb", &case.ravendb(), "--carpeta", &file],
        None,
        &format!("No se puede usar la carpeta {file}: no es una carpeta."),
        1,
    );
}
