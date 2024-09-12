use std::{env, io, process};

// fn match_pattern(input_line: &str, pattern: &str) -> bool {
//     if pattern.chars().count() == 1 {
//         input_line.contains(pattern)
//     } else if pattern == "\\d" {
//         input_line.contains(|c: char| c.is_ascii_digit())
//     } else if pattern == "\\w" {
//         input_line.contains(|c: char| c.is_ascii_alphanumeric())
//     } else if pattern.starts_with('[') && pattern.ends_with(']') {
//         let p = pattern.trim_start_matches('[').trim_end_matches(']');
//         if p.starts_with('^') {
//             let p = p.trim_start_matches('^');
//             !input_line.contains(|c: char| p.contains(c))
//         } else {
//             input_line.contains(|c| p.contains(c))
//         }
//     } else {
//         panic!("Unhandled pattern: {}", pattern)
//     }
// }

fn match_pattern(text: &str, re: &str) -> bool {
    let mut text_iter = text.chars();
    let mut re_iter = re.chars();

    if re_iter.clone().next() == Some('^') {
        re_iter.next(); // Consume
        match_here(text_iter, re_iter)
    } else {
        loop {
            if match_here(text_iter.clone(), re_iter.clone()) {
                return true;
            } else if text_iter.next().is_none() {
                return false;
            }
        }
    }
}

fn match_here<C>(mut text_iter: C, mut re_iter: C) -> bool
where
    C: Clone + Iterator<Item = char>,
{
    if let Some(r0) = re_iter.next() {
        if re_iter.clone().next() == Some('*') {
            re_iter.next(); // Consume
            match_star(text_iter, re_iter, r0)
        } else if let Some(t0) = text_iter.next() {
            if r0 == '.' || r0 == t0 {
                match_here(text_iter, re_iter)
            } else {
                false // No match
            }
        } else {
            r0 == '$' // No more input text, only works if at end
        }
    } else {
        true // regex is complete
    }
}

fn match_star<C>(mut text_iter: C, re_iter: C, c: char) -> bool
where
    C: Clone + Iterator<Item = char>,
{
    loop {
        if match_here(text_iter.clone(), re_iter.clone()) {
            return true; // Found match
        } else if let Some(t0) = text_iter.next() {
            if c == '.' || t0 == c {
                continue; // Expanding star, try again
            } else {
                return false; // Didn't match here and can't expand star, nothing else to try
            }
        } else {
            return false; // No more input text
        }
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
