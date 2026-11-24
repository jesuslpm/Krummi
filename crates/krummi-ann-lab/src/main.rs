//! The `krummi-ann-lab` command: prepares the data sets, computes the exact ground truth and measures the
//! approximate searches of design document 12. Its messages are in Spanish (repository rule).

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use krummi_ann_lab::kernels::Metric;
use krummi_ann_lab::run;

const USAGE: &str = "Uso: krummi-ann-lab <preparar|verdad|medir|grafo> --carpeta <carpeta> \
[--conjunto <sift|glove|cohere>] [--filas <n>] [--informe <archivo>] [--metrica <l2|dot|coseno>] [--sondeos <n,n,...>]";

fn parse(args: &[OsString]) -> Result<(String, run::Options), String> {
    let mut args = args.iter().map(|a| a.to_string_lossy().into_owned());
    let command = args.next().ok_or_else(|| USAGE.to_string())?;
    let mut options = run::Options::default();
    let mut folder = None;
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .filter(|v| !v.starts_with("--"))
                .ok_or_else(|| format!("Falta el valor de {arg}."))
        };
        match arg.as_str() {
            "--carpeta" => folder = Some(PathBuf::from(value()?)),
            "--conjunto" => options.set = Some(value()?.parse()?),
            "--filas" => {
                let text = value()?;
                options.rows = Some(
                    text.parse()
                        .map_err(|_| format!("Número de filas no válido: {text}."))?,
                );
            }
            "--informe" => options.report = Some(PathBuf::from(value()?)),
            "--metrica" => {
                let text = value()?;
                options.metric = Some(
                    [Metric::L2Squared, Metric::Dot, Metric::Cosine]
                        .into_iter()
                        .find(|m| m.name() == text)
                        .ok_or_else(|| {
                            format!("Métrica no válida: {text}. Son l2, dot y coseno.")
                        })?,
                );
            }
            "--sondeos" => {
                let text = value()?;
                let nprobes = text
                    .split(',')
                    .map(|n| n.parse::<usize>().ok().filter(|&n| n > 0))
                    .collect::<Option<Vec<usize>>>()
                    .ok_or_else(|| format!("Lista de sondeos no válida: {text}."))?;
                options.nprobes = Some(nprobes);
            }
            _ => return Err(format!("Opción no válida: {arg}.\n{USAGE}")),
        }
    }
    options.folder = folder.ok_or_else(|| format!("Falta --carpeta.\n{USAGE}"))?;
    Ok((command, options))
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let (command, options) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let result = match command.as_str() {
        "preparar" => run::prepare(&options),
        "verdad" => run::ground_truth(&options),
        "medir" => run::measure(&options),
        "grafo" => run::graph(&options),
        _ => {
            eprintln!("Orden no válida: {command}.\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
