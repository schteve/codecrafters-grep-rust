use std::env;
use std::io;
use std::process;

fn match_pattern(input_line: &str, pattern: &str) -> bool {
    if pattern.chars().count() == 1 {
        input_line.contains(pattern)
    } else if pattern == "\\d" {
        input_line.contains(|c: char| c.is_ascii_digit())
    } else if pattern == "\\w" {
        input_line.contains(|c: char| c.is_ascii_alphanumeric())
    } else if pattern.starts_with('[') && pattern.ends_with(']') {
        let p = pattern.trim_start_matches('[').trim_end_matches(']');
        if p.starts_with('^') {
            let p = p.trim_start_matches('^');
            !input_line.contains(|c: char| p.contains(c))
        } else {
            input_line.contains(|c| p.contains(c))
        }
    } else {
        panic!("Unhandled pattern: {}", pattern)
    }
}

// Usage: echo <input_text> | your_program.sh -E <pattern>
fn main() {
    if env::args().nth(1).unwrap() != "-E" {
        println!("Expected first argument to be '-E'");
        process::exit(1);
    }

    let pattern = env::args().nth(2).unwrap();
    let mut input_line = String::new();

    io::stdin().read_line(&mut input_line).unwrap();

    if match_pattern(&input_line, &pattern) {
        process::exit(0)
    } else {
        process::exit(1)
    }
}
