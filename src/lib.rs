#[macro_use]
extern crate log;
extern crate libc;
extern crate x11;

#[link(name = "X11")]
extern "C" {}

mod backend;
mod env;

pub fn run() -> Result<(), &'static str> {
    let mut env = env::Env::new("")?;
    env.scan_wins()?;

    trace!("starting main loop...");
    env.handle_event()
}
