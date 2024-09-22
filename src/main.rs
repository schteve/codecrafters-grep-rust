use std::{env, io, process};

#[derive(Debug, Eq, PartialEq)]
enum ReItem {
    Char(char),
    Digit,
    Alphanum,
    CharClass(String),
    NegCharClass(String),
    AnchorStart,
}

#[derive(Eq, PartialEq)]
enum CompileState {
    None,
    Beginning,
    Escaped,
    CharClassStart,
    CharClass(String),
    NegCharClass(String),
}

fn compile_re(re: &str) -> Vec<ReItem> {
    let mut items = Vec::new();

    let mut state = CompileState::Beginning;
    for c in re.chars() {
        match state {
            CompileState::None | CompileState::Beginning => match c {
                '\\' => state = CompileState::Escaped,
                '[' => state = CompileState::CharClassStart,
                ']' => panic!("Error: found ']' outside of character class"),
                '^' if state == CompileState::Beginning => {
                    items.push(ReItem::AnchorStart);
                    state = CompileState::None;
                }
                _ => items.push(ReItem::Char(c)),
            },
            CompileState::Escaped => match c {
                'd' => {
                    items.push(ReItem::Digit);
                    state = CompileState::None;
                }
                'w' => {
                    items.push(ReItem::Alphanum);
                    state = CompileState::None;
                }
                '\\' => {
                    items.push(ReItem::Char(c));
                    state = CompileState::None;
                }
                _ => panic!("Invalid escape: {c}"),
            },
            CompileState::CharClassStart => match c {
                ']' => state = CompileState::None,
                '^' => state = CompileState::NegCharClass(String::new()),
                _ => state = CompileState::CharClass(String::from(c)),
            },
            CompileState::CharClass(ref mut s) => match c {
                ']' => {
                    let cs = std::mem::replace(&mut state, CompileState::None);
                    let CompileState::CharClass(cc) = cs else {
                        unreachable!()
                    };
                    items.push(ReItem::CharClass(cc));
                }
                '\\' => panic!("Not supported in character class: {c}"),
                _ => s.push(c),
            },
            CompileState::NegCharClass(ref mut s) => match c {
                ']' => {
                    let cs = std::mem::replace(&mut state, CompileState::None);
                    let CompileState::NegCharClass(cc) = cs else {
                        unreachable!()
                    };
                    items.push(ReItem::NegCharClass(cc));
                }
                '\\' => panic!("Not supported in character class: {c}"),
                _ => s.push(c),
            },
        }
    }

    items
}

fn match_pattern(text: &str, re: &str) -> bool {
    let mut text_iter = text.chars();
    let re_compiled = compile_re(re);
    let mut re_iter = re_compiled.iter();

    if re_iter.clone().next() == Some(&ReItem::AnchorStart) {
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

fn match_here<'a, C, R>(mut text_iter: C, mut re_iter: R) -> bool
where
    C: Clone + Iterator<Item = char>,
    R: Clone + Iterator<Item = &'a ReItem>,
{
    if let Some(r0) = re_iter.next() {
        if let Some(t0) = text_iter.next() {
            let matched = match r0 {
                ReItem::Char(c) => *c == t0,
                ReItem::Digit => t0.is_ascii_digit(),
                ReItem::Alphanum => t0.is_ascii_alphanumeric(),
                ReItem::CharClass(s) => s.contains(t0),
                ReItem::NegCharClass(s) => !s.contains(t0),
                ReItem::AnchorStart => panic!("Invalid: start anchor not at start"),
            };

            if matched {
                match_here(text_iter, re_iter)
            } else {
                false // No match
            }
        } else {
            false // No match, text ran out but regex not matched
        }
    } else {
        true // regex is complete
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
