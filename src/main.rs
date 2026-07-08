use clap::Parser;
use seekr::{dispatch, Cli};

fn main() {
    println!("{}", dispatch(Cli::parse()));
}
