extern crate clap;
extern crate either;
extern crate rand;
#[macro_use]
extern crate log;
extern crate simple_logger;

use std::{ops::Range, process::Stdio, str::FromStr};

use clap::{Parser, Subcommand, ValueEnum};
use either::Either;

use rand::seq::SliceRandom;

#[derive(Subcommand)]
enum Command {
    /// Select the letter portion of the password by sampling words from a dictionary
    Dict {
        #[clap(long, env, default_value = "en")]
        language: String,
    },
    /// Select the letter portion of the password by just randomly selecting
    /// (a-z) letters.
    Chars,
}

#[derive(Clone, ValueEnum, PartialEq, Copy, Debug)]
enum Constraint {
    LowerCaseLetter,
    UpperCaseLetter,
    Number,
    Symbol,
}

impl Constraint {
    fn verify(self, opts: &Opts, s: &str) -> bool {
        s.chars().any(|c| match self {
            Constraint::LowerCaseLetter => LOWER_CASE_LETTERS.contains(&c),
            Constraint::UpperCaseLetter => UPPER_CASE_LETTERS.contains(&c),
            Constraint::Number => NUMBERS.contains(&c),
            Constraint::Symbol => opts.symbols.contains(c),
        })
    }
}

impl FromStr for Constraint {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <Self as ValueEnum>::from_str(s, true)
    }
}

const LOWER_CASE_LETTERS: Range<char> = 'a'..'z';
const UPPER_CASE_LETTERS: Range<char> = 'A'..'Z';
const NUMBERS: Range<char> = '0'..'9';
const SYMBOLS: &'static str = "-_/[]{}()*&^%$#@.!?=+:;|~";

/// Generate strong passwords by sampling words form an aspell dictionary or
/// just letters.
#[derive(Parser)]
#[clap(author, version, about)]
struct Opts {
    /// Minimal length of the password
    #[clap(long, default_value = "10")]
    min: usize,
    /// Maximal length of the password
    #[clap(long, default_value = "20")]
    max: usize,
    /// Require this constraint be fulfilled, if left empty all constraints are required.
    #[clap(long)]
    require: Vec<Constraint>,
    /// Exclude this constraint. Overwrites both default and elements in `require`
    #[clap(long)]
    exclude: Vec<Constraint>,
    /// Characters to use as the valid symbols
    #[clap(long, default_value = SYMBOLS)]
    symbols: String,
    #[clap(long, default_value = "1000")]
    tries: usize,
    #[clap(long)]
    debug: bool,
    #[clap(subcommand)]
    command: Command,
}

fn do_gen<F: Fn(&mut String, &mut rand::rngs::ThreadRng, bool) -> Option<()>>(
    opts: &Opts,
    required: &[Constraint],
    pick_letters: F,
) -> (String, u32) {
    let ref mut rng = rand::thread_rng();
    std::iter::repeat_with(|| {
        let mut s = String::new();
        while s.len() < opts.min {
            match required.choose(rng).unwrap() {
                Constraint::LowerCaseLetter => pick_letters(&mut s, rng, true)?,
                Constraint::UpperCaseLetter => pick_letters(&mut s, rng, false)?,
                Constraint::Symbol => s.push(
                    <std::str::Chars as rand::seq::IteratorRandom>::choose(
                        opts.symbols.chars(),
                        rng,
                    )
                    .unwrap(),
                ),
                Constraint::Number => s.push(
                    <std::ops::Range<char> as rand::seq::IteratorRandom>::choose(
                        NUMBERS.clone(),
                        rng,
                    )
                    .unwrap(),
                ),
            }
        }
        let judgement = s.len() <= opts.max && !required.iter().any(|c| !c.verify(opts, &s));
        if !judgement {
            debug!(
                "Rejecting {s}: {} {:?} {} and {:?}",
                s.len(),
                s.len().cmp(&opts.max),
                opts.max,
                required
                    .iter()
                    .map(|c| (c, c.verify(opts, &s)))
                    .collect::<Vec<_>>()
            );
        }
        judgement.then_some(s)
    })
    .take(opts.tries)
    .find_map({
        let mut actual_tries = 0;
        move |s| {
            actual_tries += 1;
            s.map(|s| (s, actual_tries))
        }
    })
    .expect(&format!(
        "Could not find a satisfactory string in {} tries",
        opts.tries
    ))
}

fn main() {
    let ref opts = Opts::parse();

    let mut logger = simple_logger::SimpleLogger::default().with_level(log::LevelFilter::Info);
    if opts.debug {
        info!("Running with debug");
        logger = logger.with_level(log::LevelFilter::Debug);
    } 
    logger.init().unwrap();

    let ref required = if opts.require.is_empty() {
        Either::Left(Constraint::value_variants().iter())
    } else {
        Either::Right(opts.require.iter())
    }
    .cloned()
    .into_iter()
    .filter(|i| !opts.exclude.contains(i))
    .collect::<Vec<_>>();
    assert!(!required.is_empty());
    assert!(opts.min <= opts.max);
    assert!(opts.min > required.len());

    let (result, tries) = match &opts.command {
        Command::Chars => do_gen(opts, required, |s, rng, is_lowercase| {
            s.push(
                <std::ops::Range<char> as rand::seq::IteratorRandom>::choose(
                    if is_lowercase {
                        LOWER_CASE_LETTERS
                    } else {
                        UPPER_CASE_LETTERS
                    },
                    rng,
                )
                .unwrap(),
            );
            Some(())
        }),
        Command::Dict { language } => {
            let dump = std::process::Command::new("aspell")
                .args(&["-d", language.as_str(), "dump", "master"])
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            let expanded = std::process::Command::new("aspell")
                .args(&["-l", language.as_str(), "expand"])
                .stdin(dump.stdout.unwrap())
                .output()
                .unwrap();
            let s = String::from_utf8(expanded.stdout).unwrap();
            let words = s
                .split(char::is_whitespace)
                .filter(|s| !s.contains('\'') && !s.is_empty() && !s.len() > opts.max)
                .collect::<Vec<_>>();

            assert!(words
                .iter()
                .any(|w| w.len() >= opts.min && w.len() <= opts.max));

            assert!(words.len() > 0);

            do_gen(opts, &required, |s, rng, is_lowercase| {
                let word = *std::iter::from_fn(|| words.choose(rng))
                    .filter(|w| w.len() + s.len() <= opts.max)
                    .next()?;
                let mut chars = word.chars();
                let first = chars.next().unwrap();

                s.extend(
                    if is_lowercase {
                        Either::Left(first.to_lowercase())
                    } else {
                        Either::Right(first.to_uppercase())
                    }
                    .into_iter()
                    .chain(chars),
                );
                Some(())
            })
        }
    };
    info!("Needed {tries} tries");
    println!("{result}");
}
