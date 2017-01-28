#[macro_use]
extern crate log;
extern crate env_logger;
extern crate libc;
extern crate x11;

#[link(name = "X11")]
extern "C" {}

mod rwm;

fn main() {
  env_logger::init().unwrap();
  match rwm::run() {
    Ok(()) => (),
    Err(mesg) => error!("error: {}", mesg),
  }
}
