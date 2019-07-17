extern crate env_logger;
extern crate rwm;

use std::io::{stderr, Write};

fn main() {
    env_logger::init();
    if let Err(mesg) = rwm::run() {
        let _ = writeln!(&mut stderr(), "error: {}", mesg);
    }
}
