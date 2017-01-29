#[macro_use]
extern crate log;
extern crate env_logger;
extern crate libc;
extern crate x11;

#[link(name = "X11")]
extern "C" {}

mod rwm;

fn run() -> Result<(), &'static str> {
  let mut env = rwm::Env::new("")?;
  env.scan_wins()?;

  trace!("starting main loop...");
  env.handle_event()
}

fn main() {
  env_logger::init().unwrap();

  if let Err(mesg) = run() {
    error!("error: {}", mesg);
  }
}
