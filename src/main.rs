use clap::StructOpt;
use std::io::stdin;
use rustypex::config::RustypexConfig;
use rustypex::Rustypex;
use rustypex::RustypexError;

fn main() -> Result<(), RustypexError> {
    let config = RustypexConfig::parse();

    let mut rustypex = Rustypex::new(config)?;

    let stdin = stdin();

    loop {
        let stdin = stdin.lock();
        if let Ok((true, _)) = rustypex.test(stdin) {
            rustypex.restart()?;
        } else {
            break;
        }
    }
    Ok(())
}
