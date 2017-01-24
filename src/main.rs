extern crate x11;

#[link(name = "X11")]
extern "C" {}

use x11::xlib;
use std::ptr::null_mut;

fn main() {
    let display = unsafe { xlib::XOpenDisplay(null_mut()) };
    if display == null_mut() {
        println!("can't open display");
        return;
    }

    unsafe {
        xlib::XCloseDisplay(display);
    }
}
