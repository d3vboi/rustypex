pub mod config;
pub mod results;
pub mod textgen;
pub mod tui;
pub mod wordlists;

use std::io::StdinLock;
use std::path::PathBuf;
use std::time::Instant;

use config::RustypexConfig;
use results::RustypexResults;
use termion::input::Keys;
use termion::{color, event::Key, input::TermRead};
use textgen::{RawWordSelector, WordSelector};
use tui::{Text, RustypexTui};
use wordlists::{BuiltInWordlist, OS_WORDLIST_PATH};

/// Terminal UI and logic.
pub struct Rustypex {
    tui: RustypexTui,
    text: Vec<Text>,
    words: Vec<String>,
    word_selector: Box<dyn WordSelector>,
    config: RustypexConfig,
}
/// Errors
pub struct RustypexError {
    pub msg: String,
}

impl From<std::io::Error> for RustypexError {
    fn from(error: std::io::Error) -> Self {
        RustypexError {
            msg: error.to_string(),
        }
    }
}

impl From<String> for RustypexError {
    fn from(error: String) -> Self {
        RustypexError { msg: error }
    }
}

impl std::fmt::Debug for RustypexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(format!("RustypexError: {}", self.msg).as_str())
    }
}

impl<'a> Rustypex {
    pub fn new(config: RustypexConfig) -> Result<Self, RustypexError> {
        let word_selector: Box<dyn WordSelector> =
            if let Some(wordlist_path) = config.wordlist_file.clone() {
                Box::new(RawWordSelector::from_path(PathBuf::from(wordlist_path))?)
            } else if let Some(word_list) = config.wordlist.contents() {
                Box::new(RawWordSelector::from_string(word_list.to_string())?)
            } else if let BuiltInWordlist::OS = config.wordlist {
                Box::new(RawWordSelector::from_path(PathBuf::from(OS_WORDLIST_PATH))?)
            } else {
                return Err(RustypexError {
                    msg: "Undefined word list or path.".to_string(),
                });
            };

        let mut Rustypex = Rustypex {
            tui: RustypexTui::new(),
            words: Vec::new(),
            text: Vec::new(),
            word_selector,
            config,
        };

        Rustypex.restart()?;

        Ok(Rustypex)
    }
    pub fn restart(&mut self) -> Result<(), RustypexError> {
        self.tui.reset_screen()?;

        self.words = self.word_selector.new_words(self.config.num_words)?;

        self.tui.display_lines_bottom(&[&[
            Text::from("ctrl-r").with_color(color::Blue),
            Text::from(" to restart, ").with_faint(),
            Text::from("ctrl-c").with_color(color::Blue),
            Text::from(" to quit ").with_faint(),
        ]])?;

        self.show_words()?;

        Ok(())
    }

    fn show_words(&mut self) -> Result<(), RustypexError> {
        self.text = self.tui.display_words(&self.words)?;
        Ok(())
    }

    pub fn test(&mut self, stdin: StdinLock<'a>) -> Result<(bool, RustypexResults), RustypexError> {
        let mut input = Vec::<char>::new();
        let original_text = self
            .text
            .iter()
            .fold(Vec::<char>::new(), |mut chars, text| {
                chars.extend(text.text().chars());
                chars
            });
        let mut num_errors = 0;
        let mut num_chars_typed = 0;

        enum TestStatus {
            NotDone,
            Done,
            Quit,
            Restart,
        }

        impl TestStatus {
            fn to_process_more_keys(&self) -> bool {
                matches!(self, TestStatus::NotDone)
            }

            fn to_display_results(&self) -> bool {
                matches!(self, TestStatus::Done)
            }

            fn to_restart(&self) -> bool {
                matches!(self, TestStatus::Restart)
            }
        }

        let mut process_key = |key: Key| -> Result<TestStatus, RustypexError> {
            match key {
                Key::Ctrl('c') => {
                    return Ok(TestStatus::Quit);
                }
                Key::Ctrl('r') => {
                    return Ok(TestStatus::Restart);
                }
                Key::Ctrl('w') => {
                    while !matches!(input.last(), Some(' ') | None) {
                        if input.pop().is_some() {
                            self.tui.replace_text(
                                Text::from(original_text[input.len()]).with_faint(),
                            )?;
                        }
                    }
                }
                Key::Char(c) => {
                    input.push(c);

                    if input.len() >= original_text.len() {
                        return Ok(TestStatus::Done);
                    }

                    num_chars_typed += 1;

                    if original_text[input.len() - 1] == c {
                        self.tui
                            .display_raw_text(&Text::from(c).with_color(color::LightGreen))?;
                        self.tui.move_to_next_char()?;
                    } else {
                        self.tui.display_raw_text(
                            &Text::from(original_text[input.len() - 1])
                                .with_underline()
                                .with_color(color::Red),
                        )?;
                        self.tui.move_to_next_char()?;
                        num_errors += 1;
                    }
                }
                Key::Backspace => {
                    if input.pop().is_some() {
                        self.tui
                            .replace_text(Text::from(original_text[input.len()]).with_faint())?;
                    }
                }
                _ => {}
            }

            self.tui.flush()?;

            Ok(TestStatus::NotDone)
        };

        let mut keys = stdin.keys();

        let key = keys.next().unwrap()?;
        let started_at = Instant::now();
        let mut status = process_key(key)?;

        if status.to_process_more_keys() {
            for key in &mut keys {
                status = process_key(key?)?;
                if !status.to_process_more_keys() {
                    break;
                }
            }
        }

        let ended_at = Instant::now();

        let (final_chars_typed_correctly, final_uncorrected_errors) =
            input.iter().zip(original_text.iter()).fold(
                (0, 0),
                |(total_chars_typed_correctly, total_uncorrected_errors),
                 (typed_char, orig_char)| {
                    if typed_char == orig_char {
                        (total_chars_typed_correctly + 1, total_uncorrected_errors)
                    } else {
                        (total_chars_typed_correctly, total_uncorrected_errors + 1)
                    }
                },
            );

        let results = RustypexResults {
            total_words: self.words.len(),
            total_chars_typed: num_chars_typed,
            total_chars_in_text: input.len(),
            total_char_errors: num_errors,
            final_chars_typed_correctly,
            final_uncorrected_errors,
            started_at,
            ended_at,
        };

        let to_restart = if status.to_display_results() {
            self.display_results(results.clone(), keys)?
        } else {
            status.to_restart()
        };

        Ok((to_restart, results))
    }

    // TODO: Randomize messages for each speed range.
    fn classify_results(&self, results: &RustypexResults) -> String {
        let wpm = results.wpm();

        if wpm < 10.0 {
            "A turtle could type faster.".to_string()
        } else if wpm < 20.0 {
            "Not bad.".to_string()
        } else if wpm < 30.0 {
            "Just a tad below average.".to_string()
        } else if wpm < 40.0 {
            "You're right at the average speed.".to_string()
        } else if wpm < 50.0 {
            "Great job, you're above average!".to_string()
        } else if wpm < 70.0 {
            "You type like a pro!".to_string()
        } else {
            "You're a typing god!".to_string()
        }
    }

    fn display_results(
        &mut self,
        results: RustypexResults,
        mut keys: Keys<StdinLock>,
    ) -> Result<bool, RustypexError> {
        self.tui.reset_screen()?;

        self.tui.display_lines::<&[Text], _>(&[
            &[Text::from(format!(
                "Took {}s for {} words of {}",
                results.duration().as_secs(),
                results.total_words,
                self.config.text_name(),
            ))],
            &[
                Text::from(format!("Accuracy: {:.1}%", results.accuracy() * 100.0))
                    .with_color(color::Blue),
            ],
            &[Text::from(format!(
                "Mistakes: {} out of {} characters",
                results.total_char_errors, results.total_chars_in_text
            ))],
            &[
                Text::from("Speed: "),
                Text::from(format!("{:.1} wpm", results.wpm())).with_color(color::Green),
                Text::from(" (words per minute)"),
            ],
            &[
                Text::from(format!("{}", self.classify_results(&results))),
            ],
        ])?;
        self.tui.display_lines_bottom(&[&[
            Text::from("ctrl-r").with_color(color::Blue),
            Text::from(" to restart, ").with_faint(),
            Text::from("ctrl-c").with_color(color::Blue),
            Text::from(" to quit ").with_faint(),
        ]])?;
        self.tui.hide_cursor()?;

        let mut to_restart: Option<bool> = None;
        while to_restart.is_none() {
            match keys.next().unwrap()? {
                Key::Ctrl('r') => to_restart = Some(true),
                Key::Ctrl('c') => to_restart = Some(false),
                _ => {}
            }
        }

        self.tui.show_cursor()?;

        Ok(to_restart.unwrap_or(false))
    }
}
