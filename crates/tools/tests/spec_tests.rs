use super::*;
use serde_json::Value;

#[test]
fn specs_cover_every_tool_in_order() {
    let names: Vec<&str> = specs().iter().map(|s| s.name).collect();
    assert_eq!(names, TOOL_NAMES);
}

#[test]
fn every_spec_is_a_closed_object_schema() {
    for spec in specs() {
        let p = &spec.parameters;
        assert_eq!(p["type"], "object", "{}", spec.name);
        assert_eq!(p["additionalProperties"], false, "{}", spec.name);
        assert!(!spec.description.trim().is_empty(), "{}", spec.name);

        let props = p["properties"].as_object().unwrap_or_else(|| panic!("{}: no properties", spec.name));
        for (key, prop) in props {
            assert!(prop["type"].is_string(), "{}.{key}: no type", spec.name);
            assert!(prop["description"].is_string(), "{}.{key}: no description", spec.name);
        }
        for req in p["required"].as_array().unwrap() {
            let req = req.as_str().unwrap();
            assert!(props.contains_key(req), "{}: required {req} is not a property", spec.name);
        }
    }
}

// ----- run: the loop's entry point -----

fn sb() -> Sandbox {
    Sandbox::new(&std::env::current_dir().unwrap()).unwrap()
}

fn yes(_: &str) -> Result<(), String> {
    Ok(())
}

fn no(_: &str) -> Result<(), String> {
    Err("user said no".into())
}

#[test]
fn run_returns_tool_output_on_success() {
    let out = run(&sb(), &yes, "read_file", r#"{"path": "Cargo.toml", "start_line": 1, "end_line": 1}"#);
    assert!(!out.starts_with("ERROR:"), "{out}");
    assert!(out.contains("[package]"), "{out}");
}

#[test]
fn run_turns_every_failure_into_error_text() {
    let cases = [
        ("nope", "{}", "ERROR: unknown tool: nope"),
        ("read_file", "{not json", "ERROR: invalid arguments:"),
        ("read_file", r#"{"start_line": 1}"#, "ERROR: invalid arguments:"),
        ("read_file", r#"{"path": "../outside.txt"}"#, "ERROR:"),
        ("run_command", r#"{"command": "echo hi"}"#, "ERROR:"),
    ];
    for (name, args, prefix) in cases {
        let out = run(&sb(), &no, name, args);
        assert!(out.starts_with(prefix), "{name} {args}: {out}");
    }
}

#[test]
fn run_matches_dispatch() {
    let args = r#"{"pattern": "fn specs", "glob": "tools.rs"}"#;
    assert_eq!(run(&sb(), &yes, "search", args), dispatch(&sb(), &yes, "search", args).unwrap());
}

/// Guards against the schema drifting from the args struct: the sample args
/// (which `dispatch` accepts, see the CLI tests) must use only declared
/// properties and must include every required one.
#[test]
fn sample_args_fit_each_schema() {
    for spec in specs() {
        let sample: Value = serde_json::from_str(sample_args(spec.name).unwrap()).unwrap();
        let sample = sample.as_object().unwrap();
        let props = spec.parameters["properties"].as_object().unwrap();

        for key in sample.keys() {
            assert!(props.contains_key(key), "{}: sample uses undeclared {key}", spec.name);
        }
        for req in spec.parameters["required"].as_array().unwrap() {
            let req = req.as_str().unwrap();
            assert!(sample.contains_key(req), "{}: sample is missing required {req}", spec.name);
        }
    }
}
