use clap::Parser;
use seekr::{dispatch, Cli};

fn main() {
    match dispatch(Cli::parse()) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
