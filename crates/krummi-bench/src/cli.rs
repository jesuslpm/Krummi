//! The command's options and the checks before measuring, in the order of *Validaciones y mensajes* (K10).

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

/// A failure the command reports: its message, one line on standard error, and its exit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub message: String,
    pub code: i32,
}

impl Failure {
    pub fn new(message: impl Into<String>, code: i32) -> Failure {
        Failure {
            message: message.into(),
            code,
        }
    }
}

/// The options of the command. Paths keep the text the maintainer wrote, which the messages show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub ravendb: PathBuf,
    pub work: Option<PathBuf>,
    pub report: Option<PathBuf>,
    pub reduced: bool,
}

/// Parses the arguments after the program's name. Arguments are read left to right and the first invalid one
/// wins: one that is none of the four options (also `--option=value` or a loose argument), the second occurrence
/// of a repeated option, or an option whose value is missing or starts with `--` (named by the option).
pub fn parse(args: &[OsString]) -> Result<Options, Failure> {
    let invalid = |shown: &str| Failure::new(format!("Opción no válida: {shown}."), 2);
    let mut ravendb = None;
    let mut work = None;
    let mut report = None;
    let mut reduced = false;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].to_string_lossy().into_owned();
        let slot = match arg.as_str() {
            "--reducido" => {
                if reduced {
                    return Err(invalid(&arg));
                }
                reduced = true;
                i += 1;
                continue;
            }
            "--ravendb" => &mut ravendb,
            "--carpeta" => &mut work,
            "--informe" => &mut report,
            _ => return Err(invalid(&arg)),
        };
        if slot.is_some() {
            return Err(invalid(&arg));
        }
        match args.get(i + 1) {
            Some(value) if !value.to_string_lossy().starts_with("--") => {
                *slot = Some(PathBuf::from(value));
            }
            _ => return Err(invalid(&arg)),
        }
        i += 2;
    }
    let ravendb = ravendb.ok_or_else(|| {
        Failure::new(
            "Falta la opción --ravendb con la carpeta del código de RavenDB 7.2.7.",
            2,
        )
    })?;
    Ok(Options {
        ravendb,
        work,
        report,
        reduced,
    })
}

/// A path as the messages show it.
pub fn shown(path: &Path) -> String {
    path.display().to_string()
}

fn cannot_use(folder: &Path, cause: &str) -> Failure {
    Failure::new(
        format!("No se puede usar la carpeta {}: {cause}.", shown(folder)),
        1,
    )
}

/// The report's file must not exist, also as a folder, and its folder must exist and be a folder.
pub fn check_report(report: &Path) -> Result<(), Failure> {
    // A path that ends with a separator, or has no file name, names a folder: it can never be the report's file,
    // so it fails now rather than after the whole run.
    let text = report.as_os_str().to_string_lossy();
    let names_a_folder = text.ends_with('/')
        || text.ends_with(std::path::MAIN_SEPARATOR)
        || report.file_name().is_none();
    if names_a_folder && fs::symlink_metadata(report).is_err() {
        return Err(cannot_use(report, "no existe"));
    }
    if fs::symlink_metadata(report).is_ok() {
        return Err(Failure::new(
            format!("El destino {} ya existe.", shown(report)),
            1,
        ));
    }
    let parent = match report.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    match fs::metadata(parent) {
        Ok(m) if m.is_dir() => can_create_in(parent),
        Ok(_) => Err(cannot_use(parent, "no es una carpeta")),
        Err(_) => Err(cannot_use(parent, "no existe")),
    }
}

/// Whether a new file can be created in `folder`: it creates one with a name of its own and removes it, so that a
/// folder without write permission, or on a read-only file system, fails before the run and not when the report is
/// written at its end.
fn can_create_in(folder: &Path) -> Result<(), Failure> {
    let probe = folder.join(format!(".krummi-bench-{}.tmp", std::process::id()));
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(file) => {
            drop(file);
            let _ = fs::remove_file(&probe);
            Ok(())
        }
        Err(e) => Err(cannot_use(folder, &e.to_string())),
    }
}

/// The `--ravendb` folder holds RavenDB 7.2.7 (design D3).
pub fn check_ravendb(folder: &Path) -> Result<(), Failure> {
    let version = fs::read_to_string(folder.join("src/CommonAssemblyInfo.cs")).unwrap_or_default();
    if folder.join("src/Voron/Voron.csproj").is_file()
        && version.contains("AssemblyInformationalVersion(\"7.2.7\")")
    {
        Ok(())
    } else {
        Err(Failure::new(
            format!(
                "La carpeta {} no contiene el código de RavenDB 7.2.7.",
                shown(folder)
            ),
            1,
        ))
    }
}

/// A .NET SDK of version 10 is installed: `list_sdks` is the output of `dotnet --list-sdks`, or `None` when the
/// command could not run.
pub fn check_sdk(list_sdks: Option<&str>) -> Result<(), Failure> {
    if list_sdks.is_some_and(|out| out.lines().any(|l| l.trim_start().starts_with("10."))) {
        Ok(())
    } else {
        Err(Failure::new(
            "No se encuentra el SDK de .NET 10, necesario para medir Voron.",
            1,
        ))
    }
}

/// The work folder must exist; then a new subfolder of its own is created in it, which the run removes at the end.
pub fn create_work_subfolder(work: &Path, name: &str) -> Result<PathBuf, Failure> {
    match fs::metadata(work) {
        Err(_) => {
            return Err(Failure::new(
                format!("La carpeta {} no existe.", shown(work)),
                1,
            ));
        }
        Ok(m) if !m.is_dir() => return Err(cannot_use(work, "no es una carpeta")),
        Ok(_) => {}
    }
    let sub = work.join(name);
    fs::create_dir(&sub).map_err(|e| cannot_use(work, &e.to_string()))?;
    Ok(sub)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    fn fails(list: &[&str]) -> Failure {
        parse(&args(list)).unwrap_err()
    }

    #[test]
    fn options_go_in_any_order() {
        let o = parse(&args(&[
            "--reducido",
            "--informe",
            "r.md",
            "--ravendb",
            "rv",
            "--carpeta",
            "w",
        ]))
        .unwrap();
        assert_eq!(
            o,
            Options {
                ravendb: "rv".into(),
                work: Some("w".into()),
                report: Some("r.md".into()),
                reduced: true
            }
        );
        let o = parse(&args(&["--ravendb", "rv"])).unwrap();
        assert_eq!((o.work, o.report, o.reduced), (None, None, false));
    }

    // AC-K9, first rows of the table: the first invalid argument from the left wins.
    #[test]
    fn invalid_arguments_name_the_first_one() {
        let usage = |m: &str| Failure::new(format!("Opción no válida: {m}."), 2);
        assert_eq!(fails(&["--ravendb", "rv", "--otra"]), usage("--otra"));
        assert_eq!(fails(&["--ravendb=rv"]), usage("--ravendb=rv"));
        assert_eq!(fails(&["suelto", "--x"]), usage("suelto"));
        assert_eq!(
            fails(&["--ravendb", "a", "--ravendb", "b"]),
            usage("--ravendb")
        );
        assert_eq!(fails(&["--reducido", "--reducido"]), usage("--reducido"));
        assert_eq!(fails(&["--ravendb", "rv", "--carpeta"]), usage("--carpeta"));
        assert_eq!(fails(&["--carpeta", "--informe", "x"]), usage("--carpeta"));
        assert_eq!(
            fails(&["--ravendb", "rv", "--informe", "--x"]),
            usage("--informe")
        );
        // An invalid argument wins over the missing --ravendb.
        assert_eq!(fails(&["--x"]), usage("--x"));
    }

    #[test]
    fn ravendb_is_required() {
        let missing = Failure::new(
            "Falta la opción --ravendb con la carpeta del código de RavenDB 7.2.7.",
            2,
        );
        assert_eq!(fails(&[]), missing);
        assert_eq!(fails(&["--reducido", "--carpeta", "w"]), missing);
    }

    #[test]
    fn the_report_must_not_exist_and_its_folder_must() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("r.md");
        fs::write(&existing, "x").unwrap();
        assert_eq!(
            check_report(&existing).unwrap_err(),
            Failure::new(format!("El destino {} ya existe.", existing.display()), 1)
        );
        assert_eq!(check_report(dir.path()).unwrap_err().code, 1);
        assert!(
            check_report(dir.path())
                .unwrap_err()
                .message
                .ends_with("ya existe.")
        );
        let no_folder = dir.path().join("nada").join("r.md");
        assert_eq!(
            check_report(&no_folder).unwrap_err(),
            Failure::new(
                format!(
                    "No se puede usar la carpeta {}: no existe.",
                    dir.path().join("nada").display()
                ),
                1
            )
        );
        let file_parent = existing.join("r.md");
        assert!(
            check_report(&file_parent)
                .unwrap_err()
                .message
                .ends_with(": no es una carpeta.")
        );
        // A path that ends with a separator names a folder, which can never be the report's file.
        let trailing = format!(
            "{}{}",
            dir.path().join("nada").display(),
            std::path::MAIN_SEPARATOR
        );
        assert_eq!(
            check_report(Path::new(&trailing)).unwrap_err(),
            Failure::new(
                format!("No se puede usar la carpeta {trailing}: no existe."),
                1
            )
        );
        let trailing_existing = format!("{}/", dir.path().display());
        assert_eq!(
            check_report(Path::new(&trailing_existing)).unwrap_err(),
            Failure::new(format!("El destino {trailing_existing} ya existe."), 1)
        );
        assert_eq!(check_report(&dir.path().join("nuevo.md")), Ok(()));
        assert_eq!(check_report(Path::new("nuevo-que-no-existe.md")), Ok(()));
    }

    // R2 of the Codex code review: the report's folder must accept a new file before the run, and checking it leaves
    // the folder as it was.
    #[test]
    fn the_report_folder_must_accept_a_new_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(check_report(&dir.path().join("nuevo.md")), Ok(()));
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            0,
            "the check left a file"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let locked = dir.path().join("cerrada");
            fs::create_dir(&locked).unwrap();
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
            // A superuser writes anyway: then there is nothing to check.
            let writable = fs::write(locked.join("probe"), "x").is_ok();
            if !writable {
                let failure = check_report(&locked.join("r.md")).unwrap_err();
                let prefix = format!("No se puede usar la carpeta {}: ", locked.display());
                assert!(failure.message.starts_with(&prefix), "{}", failure.message);
                assert!(failure.message.ends_with('.'));
                assert_eq!(failure.code, 1);
            }
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn ravendb_is_recognized_by_its_files() {
        let dir = tempfile::tempdir().unwrap();
        let expected = Failure::new(
            format!(
                "La carpeta {} no contiene el código de RavenDB 7.2.7.",
                dir.path().display()
            ),
            1,
        );
        assert_eq!(check_ravendb(dir.path()).unwrap_err(), expected);
        assert!(check_ravendb(&dir.path().join("no-existe")).is_err());
        fs::create_dir_all(dir.path().join("src/Voron")).unwrap();
        fs::write(dir.path().join("src/Voron/Voron.csproj"), "").unwrap();
        fs::write(
            dir.path().join("src/CommonAssemblyInfo.cs"),
            "[assembly: AssemblyInformationalVersion(\"7.2.6\")]",
        )
        .unwrap();
        assert!(check_ravendb(dir.path()).is_err());
        fs::write(
            dir.path().join("src/CommonAssemblyInfo.cs"),
            "[assembly: AssemblyInformationalVersion(\"7.2.7\")]",
        )
        .unwrap();
        assert_eq!(check_ravendb(dir.path()), Ok(()));
    }

    #[test]
    fn the_sdk_must_be_of_version_10() {
        let missing = Failure::new(
            "No se encuentra el SDK de .NET 10, necesario para medir Voron.",
            1,
        );
        assert_eq!(check_sdk(None), Err(missing.clone()));
        assert_eq!(check_sdk(Some("8.0.427 [C:\\sdk]\n")), Err(missing));
        assert_eq!(
            check_sdk(Some("8.0.427 [C:\\sdk]\n10.0.201 [C:\\sdk]\n")),
            Ok(())
        );
    }

    #[test]
    fn the_work_folder_must_exist_and_be_usable() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no");
        assert_eq!(
            create_work_subfolder(&missing, "s").unwrap_err(),
            Failure::new(format!("La carpeta {} no existe.", missing.display()), 1)
        );
        let file = dir.path().join("f");
        fs::write(&file, "").unwrap();
        assert_eq!(
            create_work_subfolder(&file, "s").unwrap_err(),
            Failure::new(
                format!(
                    "No se puede usar la carpeta {}: no es una carpeta.",
                    file.display()
                ),
                1
            )
        );
        let sub = create_work_subfolder(dir.path(), "s").unwrap();
        assert!(sub.is_dir());
        assert!(
            create_work_subfolder(dir.path(), "s")
                .unwrap_err()
                .message
                .starts_with("No se puede usar la carpeta")
        );
    }
}
