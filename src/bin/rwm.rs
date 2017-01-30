extern crate rwm;
extern crate env_logger;

use std::io::{stderr, Write};

fn main() {
  env_logger::init().unwrap();
  if let Err(mesg) = rwm::run() {
    let _ = writeln!(&mut stderr(), "error: {}", mesg);
  }
}
