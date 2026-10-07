//! The CLI on every golden case of the core repository, checked out next to this one, at
//! each version the CLI accepts; then the paths around it: gzip, standard input, Extended
//! and the exit codes.

#![expect(clippy::unwrap_used, reason = "a broken case should fail the test")]

use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const CORE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../hpt-validator-rs/tests");

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn cli(args: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hpt-validator"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}

/// The JSON output, after checking it is laid out as `JSON.stringify(result, null, 2)`.
fn json(out: &Output) -> Value {
    let text = std::str::from_utf8(&out.stdout).unwrap();
    let value: Value = serde_json::from_str(text).unwrap();
    assert_eq!(text, serde_json::to_string_pretty(&value).unwrap() + "\n");
    value
}

/// Errors and alerts with only upstream's keys, as compact JSON so key order counts.
fn upstream(value: &Value) -> String {
    let mut value = value.clone();
    for list in ["errors", "alerts"] {
        for finding in value[list].as_array_mut().unwrap() {
            let finding = finding.as_object_mut().unwrap();
            finding.retain(|k, _| !matches!(k.as_str(), "code" | "keyword" | "line"));
        }
    }
    serde_json::to_string(&[&value["valid"], &value["errors"], &value["alerts"]]).unwrap()
}

/// Runs the CLI on one golden result; a description of the difference if there is one.
fn check((case, key, want): &(String, String, Value)) -> Option<String> {
    let (version, limit) = key.split_once(" maxErrors=").unwrap();
    let out = cli(&[case, version, "-o", "json", "-e", limit], &[]);
    let got = json(&out);
    let valid = want["valid"].as_bool().unwrap();
    let ok = upstream(&got) == upstream(want)
        && got["dataDictionaryVersion"] == version[1..]
        && got["errorCount"] == want["errors"].as_array().unwrap().len()
        && got["alertCount"] == want["alerts"].as_array().unwrap().len()
        && out.status.code() == Some(i32::from(!valid));
    (!ok).then(|| format!("{case} {key}\n  got  {got}\n  want {want}"))
}

#[test]
fn golden() {
    let tests = Path::new(CORE);
    let mut goldens = Vec::new();
    files(&tests.join("golden"), &mut goldens);
    let mut runs = Vec::new();
    for golden in goldens {
        let case = tests
            .join("cases")
            .join(golden.strip_prefix(tests.join("golden")).unwrap())
            .with_extension("");
        let results: Value = serde_json::from_str(&fs::read_to_string(&golden).unwrap()).unwrap();
        for (key, want) in results.as_object().unwrap() {
            let version = key.split_once(' ').unwrap().0;
            if ["v2.1.0", "v2.2.0", "v3.0.0"].contains(&version) {
                let case = case.to_str().unwrap().to_string();
                runs.push((case, key.clone(), want.clone()));
            }
        }
    }
    // A process per run; spread them over the cores.
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let failed: Vec<String> = std::thread::scope(|scope| {
        let chunks = runs.chunks(runs.len().div_ceil(threads));
        let handles: Vec<_> = (chunks
            .map(|chunk| scope.spawn(move || chunk.iter().filter_map(check).collect::<Vec<_>>())))
        .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let checked = runs.len();
    assert!(failed.is_empty(), "{} of {checked} failed:\n{}", failed.len(), failed.join("\n"));
    assert!(checked > 1500, "only {checked} results checked");
}

#[test]
fn paths() {
    let case = format!("{CORE}/cases/findings/invalid-quotes.csv");
    let text = fs::read(&case).unwrap();
    let plain = json(&cli(&[&case, "v2.1", "-o", "json"], &[]));

    // gzip, picked by the .gz suffix
    let gz = Path::new(env!("CARGO_TARGET_TMPDIR")).join("invalid-quotes.csv.gz");
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&text).unwrap();
    fs::write(&gz, encoder.finish().unwrap()).unwrap();
    let out = cli(&[gz.to_str().unwrap(), "2.1", "-o", "json"], &[]);
    assert_eq!(upstream(&json(&out)), upstream(&plain));

    // standard input needs -f
    let out = cli(&["-", "2.1", "-f", "csv", "-o", "json"], &text);
    assert_eq!(upstream(&json(&out)), upstream(&plain));
    assert_eq!(cli(&["-", "2.1"], &text).status.code(), Some(2));

    // Extended adds the file type and the quote finding upstream ignores
    let out = json(&cli(&[&case, "2.1", "-o", "json", "--extended"], &[]));
    assert_eq!(upstream(&out), upstream(&plain));
    assert_eq!(out["fileType"], "tall");
    assert_eq!(out["extended"][0]["code"], "HPT_X_CSV_INVALID_QUOTES");

    // exit codes: invalid 1, usage 2, unreadable input 3
    assert_eq!(cli(&[&case, "3"], &[]).status.code(), Some(1));
    for usage in [
        &[&case, "2.0"][..],
        &[&case, "03"],
        &[&case, "2.01"],
        &[&case, "3.0.01"],
        &[&case],
        &[&case, "3", "-e", "x"],
        &[&case, "3", "--x"],
    ] {
        assert_eq!(cli(usage, &[]).status.code(), Some(2), "{usage:?}");
    }
    assert_eq!(cli(&["missing.csv", "3"], &[]).status.code(), Some(3));
}
