static SHADER_DIR: &'static str = "src/_shaders";
static GLSL_VALIDATOR: &'static str = "/usr/local/bin/glslangValidator";

use std::fs;
use std::process::Command;

fn main() {
    check_glsl();
}

fn check_glsl() {
    let shader_directory = fs::read_dir(SHADER_DIR)
        .expect(&format!("Could not read shader directory: {}", SHADER_DIR));

    // Iterate through directories and accumulate scripts to check
    let mut scripts = Vec::new();
    for entry_result in shader_directory {
        let path = match entry_result {
            Ok(entry) => entry.path(),
            Err(err) => {
                eprintln!("WARN: there was an error a directory entry: {}", err);
                continue;
            }
        };
        
        match path.extension().map(|e| e.to_str().expect("Unable to convert extension to str")) {
            Some("vert") | Some("frag") => {
                scripts.push(path);
            }
            _ => {
                eprintln!("WARN: {} does not appear to be a glsl script", path.display());
            }
        }
    } 

    // Iterate through scripts and validate them
    let mut bad_scripts = 0;
    for path in scripts {
        let output = Command::new(GLSL_VALIDATOR)
            .arg(&path)
            .output()
            .expect(&format!("Unable to run {} on {}", GLSL_VALIDATOR, path.display()));

        if ! output.status.success() {
            let stdout = String::from_utf8(output.stdout)
                .expect(&format!("Unable to make string stdout for {}", path.display()));
            let stderr = String::from_utf8(output.stderr)
                .expect(&format!("Unable to make string stdout for {}", path.display()));


            eprintln!(
                "ERROR: failed to validate {}\nEXIT STATUS: {}",
                path.display(),
                output.status
            );

            if ! stdout.is_empty() {
                eprintln!("STDOUT:\n{}", stdout);
            }
            
            if ! stderr.is_empty() {
                eprintln!("STDERR:\n{}", stderr);
            }

            bad_scripts += 1;
        }
    }

    // We need to panic if there was an issue
    if bad_scripts > 0 {
        panic!(format!("There were {} bad scripts", bad_scripts));
    }
}
