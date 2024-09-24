use std::{env, io, iter::Peekable, process};

type Phrase = Vec<ReItem>;

#[derive(Debug, Eq, PartialEq)]
enum ReItem {
    Char(char),
    Digit,
    Alphanum,
    CharClass(String),
    NegCharClass(String),
    AnchorStart,
    AnchorEnd,
    QuantZeroPlus,
    QuantOnePlus,
    QuantZeroOrOne,
    Wildcard,
    Alternative(Vec<Phrase>),
}

#[derive(Eq, PartialEq)]
enum CompileState {
    None,
    Beginning,
    Escaped,
    CharClassStart,
    CharClass(String),
    NegCharClass(String),
    Group,
}

fn compile_re(re: &str) -> Vec<Phrase> {
    let mut phrases = Vec::new();

    let mut re_iter = re.chars().peekable();
    while re_iter.peek().is_some() {
        let phrase = compile_phrase(&mut re_iter);
        phrases.push(phrase);
    }

    phrases
}

fn compile_phrase<R>(re_iter: &mut Peekable<R>) -> Phrase
where
    R: Iterator<Item = char>,
{
    let mut items = Vec::new();

    let mut state = CompileState::Beginning;
    while let Some(&c) = re_iter.peek() {
        match state {
            CompileState::None | CompileState::Beginning => match c {
                '\\' => state = CompileState::Escaped,
                '[' => state = CompileState::CharClassStart,
                ']' => panic!("Error: found ']' outside of character class"),
                '^' if state == CompileState::Beginning => {
                    items.push(ReItem::AnchorStart);
                    state = CompileState::None;
                }
                '$' => items.push(ReItem::AnchorEnd),
                '*' => items.push(ReItem::QuantZeroPlus),
                '+' => items.push(ReItem::QuantOnePlus),
                '?' => items.push(ReItem::QuantZeroOrOne),
                '.' => items.push(ReItem::Wildcard),
                '(' => state = CompileState::Group,
                '|' | ')' => break, // Let parent deal with it, don't consume
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
            CompileState::Group => {
                let mut grp = Vec::new();
                loop {
                    let phrase = compile_phrase(re_iter);
                    grp.push(phrase);

                    match re_iter.peek() {
                        Some('|') => {
                            re_iter.next(); // Consume
                        }
                        Some(')') => {
                            break;
                        }
                        Some(x) => panic!("Invalid group close: {x}"),
                        None => panic!("Group not closed"),
                    }
                }

                items.push(ReItem::Alternative(grp));
                state = CompileState::None;
            }
        }

        re_iter.next(); // Consume
    }

    items
}

fn match_pattern(text: &str, re: &str) -> bool {
    let re_compiled = compile_re(re);

    for phrase in re_compiled.iter() {
        let text_iter = text.chars();
        let re_iter = phrase.iter().peekable();
        if match_phrase(text_iter, re_iter).is_some() {
            return true;
        }
    }

    false
}

fn match_phrase<'a, C, R>(mut text_iter: C, mut re_iter: Peekable<R>) -> Option<C>
where
    C: Clone + Iterator<Item = char>,
    R: Clone + Iterator<Item = &'a ReItem>,
{
    if matches!(re_iter.peek(), Some(ReItem::AnchorStart)) {
        re_iter.next(); // Consume
        match_here(text_iter, re_iter)
    } else {
        loop {
            let result = match_here(text_iter.clone(), re_iter.clone());
            if result.is_some() {
                return result;
            } else if text_iter.next().is_none() {
                return None;
            }
        }
    }
}

fn match_here<'a, C, R>(mut text_iter: C, mut re_iter: Peekable<R>) -> Option<C>
where
    C: Clone + Iterator<Item = char>,
    R: Clone + Iterator<Item = &'a ReItem>,
{
    if let Some(r0) = re_iter.next() {
        if matches!(re_iter.peek(), Some(ReItem::QuantZeroPlus)) {
            re_iter.next(); // Consume
            match_quant(r0, 0, usize::MAX, text_iter, re_iter)
        } else if matches!(re_iter.peek(), Some(ReItem::QuantOnePlus)) {
            re_iter.next(); // Consume
            match_quant(r0, 1, usize::MAX, text_iter, re_iter)
        } else if matches!(re_iter.peek(), Some(ReItem::QuantZeroOrOne)) {
            re_iter.next(); // Consume
            match_quant(r0, 0, 1, text_iter, re_iter)
        } else if let ReItem::Alternative(alts) = r0 {
            match_alts(alts, text_iter, re_iter)
        } else if let Some(t0) = text_iter.next() {
            if match_char(t0, r0) {
                match_here(text_iter, re_iter)
            } else {
                None // No match
            }
        } else if r0 == &ReItem::AnchorEnd {
            Some(text_iter) // No more input text, but at end so it's a match
        } else {
            None // No more input text, no match
        }
    } else {
        Some(text_iter) // regex is complete
    }
}

fn match_char(text_char: char, re_item: &ReItem) -> bool {
    match re_item {
        ReItem::Char(c) => *c == text_char,
        ReItem::Digit => text_char.is_ascii_digit(),
        ReItem::Alphanum => text_char.is_ascii_alphanumeric(),
        ReItem::CharClass(s) => s.contains(text_char),
        ReItem::NegCharClass(s) => !s.contains(text_char),
        ReItem::AnchorStart => panic!("Invalid: start anchor not at start"),
        ReItem::AnchorEnd => false, // Never matches a character
        ReItem::QuantZeroPlus => panic!("Invalid: quant 0+ not matchable"),
        ReItem::QuantOnePlus => panic!("Invalid: quant 1+ not matchable"),
        ReItem::QuantZeroOrOne => panic!("Invalid: quant 0-1 not matchable"),
        ReItem::Wildcard => true,
        ReItem::Alternative(_) => panic!("Invalid: alts not matchable"),
    }
}

fn match_quant<'a, C, R>(
    item: &ReItem,
    min: usize,
    max: usize,
    mut text_iter: C,
    re_iter: Peekable<R>,
) -> Option<C>
where
    C: Clone + Iterator<Item = char>,
    R: Clone + Iterator<Item = &'a ReItem>,
{
    let mut count = 0;
    while count <= max {
        if count >= min {
            let result = match_here(text_iter.clone(), re_iter.clone());
            if result.is_some() {
                return result; // Found match
            }
        }

        if let Some(t0) = text_iter.next() {
            if match_char(t0, item) {
                count += 1;
                continue; // Continue to expand, try again
            } else {
                return None; // Didn't match here and can't expand further, nothing else to try
            }
        } else {
            return None; // No more input text
        }
    }
    None
}

fn match_alts<'a, C, R>(alts: &'a [Phrase], text_iter: C, re_iter: Peekable<R>) -> Option<C>
where
    C: Clone + Iterator<Item = char>,
    R: Clone + Iterator<Item = &'a ReItem>,
{
    for phrase in alts {
        if let Some(text_remainder) = match_here(text_iter.clone(), phrase.iter().peekable()) {
            let result = match_here(text_remainder, re_iter.clone());
            if result.is_some() {
                return result;
            }
        }
    }
    None
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
