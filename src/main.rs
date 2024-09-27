use std::{
    env, io,
    iter::{self, Peekable},
    process,
};

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
    Group(usize, Vec<Phrase>),
    Backreference(usize),
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

struct CompileResult {
    phrases: Vec<Phrase>,
    groups: usize,
}

struct ReCompiler {
    groups: usize,
}

impl ReCompiler {
    fn compile(re: &str) -> CompileResult {
        let mut compiler = Self { groups: 0 };

        let mut phrases = Vec::new();

        let mut re_iter = re.chars().peekable();
        while re_iter.peek().is_some() {
            let phrase = compiler.compile_phrase(&mut re_iter);
            phrases.push(phrase);
        }

        CompileResult {
            phrases,
            groups: compiler.groups,
        }
    }

    fn compile_phrase<R>(&mut self, re_iter: &mut Peekable<R>) -> Phrase
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
                    d if ('1'..='9').contains(&d) => {
                        items.push(ReItem::Backreference(d.to_digit(10).unwrap() as usize - 1));
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
                    let group_n = self.groups;
                    self.groups += 1;

                    let mut grp = Vec::new();
                    loop {
                        let phrase = self.compile_phrase(re_iter);
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

                    items.push(ReItem::Group(group_n, grp));
                    state = CompileState::None;
                }
            }

            re_iter.next(); // Consume
        }

        items
    }
}

fn match_pattern(text: &str, re: &str) -> Option<String> {
    let compile_result = ReCompiler::compile(re);

    for phrase in compile_result.phrases.iter() {
        let text_iter = text.chars();
        let re_iter = phrase.iter().peekable();
        let matcher = Matcher {
            text_iter,
            re_iter,
            backreferences: vec![String::new(); compile_result.groups],
            matched: String::new(),
        };

        if let Some(result) = matcher.match_phrase() {
            return Some(result.matched);
        }
    }

    None
}

struct MatchResult<T>
where
    T: Clone + Iterator<Item = char>,
{
    matched: String,
    backreferences: Vec<String>,
    remainder: T,
}

#[derive(Clone)]
struct Matcher<'a, T, R>
where
    T: Clone + Iterator<Item = char>,
    R: Clone + Iterator<Item = &'a ReItem>,
{
    text_iter: T,
    re_iter: Peekable<R>,
    backreferences: Vec<String>,
    matched: String,
}

impl<'a, T, R> Matcher<'a, T, R>
where
    T: Clone + Iterator<Item = char>,
    R: Clone + Iterator<Item = &'a ReItem>,
{
    fn into_result(self) -> MatchResult<T> {
        MatchResult {
            matched: self.matched,
            backreferences: self.backreferences,
            remainder: self.text_iter,
        }
    }

    fn match_phrase(mut self) -> Option<MatchResult<T>> {
        if matches!(self.re_iter.peek(), Some(ReItem::AnchorStart)) {
            self.re_iter.next(); // Consume
            self.match_here()
        } else {
            loop {
                let result = self.clone().match_here();
                if result.is_some() {
                    return result;
                } else if self.text_iter.next().is_none() {
                    return None;
                }
            }
        }
    }

    fn match_here(mut self) -> Option<MatchResult<T>> {
        if let Some(r0) = self.re_iter.next() {
            if matches!(self.re_iter.peek(), Some(ReItem::QuantZeroPlus)) {
                self.re_iter.next(); // Consume
                self.match_quant_greedy(r0, 0, usize::MAX)
            } else if matches!(self.re_iter.peek(), Some(ReItem::QuantOnePlus)) {
                self.re_iter.next(); // Consume
                self.match_quant_greedy(r0, 1, usize::MAX)
            } else if matches!(self.re_iter.peek(), Some(ReItem::QuantZeroOrOne)) {
                self.re_iter.next(); // Consume
                self.match_quant_greedy(r0, 0, 1)
            } else if let ReItem::Group(n, alts) = r0 {
                self.match_group(*n, alts)
            } else if let ReItem::Backreference(backref) = r0 {
                self.match_backref(*backref)
            } else if let Some(t0) = self.text_iter.next() {
                if match_char(t0, r0) {
                    self.matched.push(t0);
                    self.match_here()
                } else {
                    None // No match
                }
            } else if r0 == &ReItem::AnchorEnd {
                // No more input text, but at end so it's a match
                Some(self.into_result())
            } else {
                None // No more input text, no match
            }
        } else {
            // regex is complete
            Some(self.into_result())
        }
    }

    #[allow(dead_code)]
    fn match_quant_lazy(mut self, item: &ReItem, min: usize, max: usize) -> Option<MatchResult<T>> {
        let mut count = 0;
        while count <= max {
            if count >= min {
                let result = self.clone().match_here();
                if result.is_some() {
                    return result; // Found match
                }
            }

            if let Some(t0) = self.text_iter.next() {
                if match_char(t0, item) {
                    self.matched.push(t0);
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

    fn match_quant_greedy(self, item: &ReItem, min: usize, max: usize) -> Option<MatchResult<T>> {
        if max == 0 {
            return None;
        }

        let item_matcher = Matcher {
            text_iter: self.text_iter.clone(),
            re_iter: iter::once(item).peekable(),
            backreferences: self.backreferences.clone(),
            matched: String::new(),
        };
        if let Some(result) = item_matcher.match_here() {
            let mut matched = self.matched.clone();
            matched.push_str(&result.matched);

            let quant_matcher = Matcher {
                text_iter: result.remainder.clone(),
                re_iter: self.re_iter.clone(),
                backreferences: result.backreferences.clone(),
                matched: matched.clone(),
            };
            let quant_result =
                quant_matcher.match_quant_greedy(item, min.saturating_sub(1), max - 1);
            if quant_result.is_some() {
                quant_result
            } else {
                let remainder_matcher = Matcher {
                    text_iter: result.remainder,
                    re_iter: self.re_iter.clone(),
                    backreferences: result.backreferences,
                    matched,
                };
                remainder_matcher.match_here()
            }
        } else if min == 0 {
            self.match_here()
        } else {
            None
        }
    }

    fn match_group(self, n: usize, alts: &'a [Phrase]) -> Option<MatchResult<T>> {
        for phrase in alts {
            let phrase_matcher = Matcher {
                text_iter: self.text_iter.clone(),
                re_iter: phrase.iter().peekable(),
                backreferences: self.backreferences.clone(),
                matched: String::new(),
            };
            if let Some(result) = phrase_matcher.match_here() {
                let mut matched = self.matched.clone();
                matched.push_str(&result.matched);

                // The result has the latest backreferences - update it and use
                // it for future matching
                let mut backreferences = result.backreferences;
                backreferences[n] = result.matched;

                let remainder_matcher = Matcher {
                    text_iter: result.remainder,
                    re_iter: self.re_iter.clone(),
                    backreferences,
                    matched,
                };
                let result = remainder_matcher.match_here();
                if result.is_some() {
                    return result;
                }
            }
        }

        None
    }

    fn match_backref(self, backref: usize) -> Option<MatchResult<T>> {
        if let Some(s) = self.backreferences.get(backref) {
            let re: Vec<_> = s.chars().map(ReItem::Char).collect(); // Match the exact text
            let backref_matcher = Matcher {
                text_iter: self.text_iter.clone(),
                re_iter: re.iter().peekable(),
                backreferences: self.backreferences.clone(),
                matched: String::new(),
            };
            if let Some(result) = backref_matcher.match_here() {
                let mut matched = self.matched.clone();
                matched.push_str(&result.matched);

                let remainder_matcher = Matcher {
                    text_iter: result.remainder,
                    re_iter: self.re_iter.clone(),
                    backreferences: self.backreferences,
                    matched,
                };
                remainder_matcher.match_here()
            } else {
                None
            }
        } else {
            None
        }
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
        ReItem::Group(_, _) => panic!("Invalid: alts not matchable"),
        ReItem::Backreference(_) => panic!("Invalid: backreferences not matchable"),
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

    if let Some(matched) = match_pattern(&input_line, &pattern) {
        println!("Matched: \"{matched}\"");
        process::exit(0)
    } else {
        process::exit(1)
    }
}
