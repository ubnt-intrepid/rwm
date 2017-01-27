#[macro_use]
extern crate log;
extern crate env_logger;
extern crate x11;

#[link(name = "X11")]
extern "C" {}

use x11::xlib;
use std::ptr::null_mut;

enum Event {
  MapRequest(xlib::XMapRequestEvent),
  Unmap(xlib::XUnmapEvent),
  Destroy(xlib::XDestroyWindowEvent),
  Unknown,
}

#[allow(dead_code)]
struct Entry {
  window: xlib::Window,
  ignore_unmap: bool,
}

struct Env {
  display: *mut xlib::Display,
  root: xlib::Window,
  clients: Vec<Entry>,
}

impl Drop for Env {
  fn drop(&mut self) {
    unsafe {
      xlib::XCloseDisplay(self.display);
    }
    self.display = null_mut();
  }
}

impl Env {
  fn new(displayname: &str) -> Result<Env, &'static str> {
    let displayname = std::ffi::CString::new(displayname).map_err(|_| "invalid displayname")?;
    let display = unsafe { xlib::XOpenDisplay(displayname.as_ptr()) };
    if display == null_mut() {
      return Err("can't open display");
    }
    info!("success: XOpenDisplay");

    let screen = unsafe { xlib::XDefaultScreenOfDisplay(display) };
    let root = unsafe { xlib::XRootWindowOfScreen(screen) };

    Ok(Env {
      clients: Vec::new(),
      root: root,
      display: display,
    })
  }

  fn mask_events(&mut self) -> Result<(), &'static str> {
    // let black = unsafe { xlib::XBlackPixel(self.raw, 0) };
    // let white = unsafe { xlib::XWhitePixel(self.raw, 0) };
    info!("success: XRootWindow");

    let mut attr = xlib::XSetWindowAttributes {
      background_pixel: 0,
      background_pixmap: 0,
      backing_pixel: 0,
      backing_planes: 0,
      backing_store: 0,
      bit_gravity: 0,
      border_pixel: 0,
      border_pixmap: 0,
      colormap: 0,
      cursor: 0,
      do_not_propagate_mask: 0,
      event_mask: xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask,
      override_redirect: 0,
      save_under: 0,
      win_gravity: 0,
    };
    unsafe {
      xlib::XChangeWindowAttributes(self.display,
                                    self.root,
                                    xlib::CWCursor | xlib::CWEventMask,
                                    &mut attr)
    };
    info!("success: XChangeWindowAttributes");

    unsafe { xlib::XSync(self.display, xlib::False) };
    Ok(())
  }

  fn next_event(&mut self) -> Event {
    let mut ev = xlib::XEvent { pad: [0; 24] };
    unsafe {
      xlib::XNextEvent(self.display, &mut ev);
      match ev.get_type() {
        xlib::MapRequest => Event::MapRequest(std::mem::transmute_copy(&ev)),
        xlib::UnmapNotify => Event::Unmap(std::mem::transmute_copy(&ev)),
        xlib::DestroyNotify => Event::Destroy(std::mem::transmute_copy(&ev)),
        _ => Event::Unknown,
      }
    }
  }

  fn scan_wins(&mut self) -> Result<(), &'static str> {
    unsafe {
      let mut wins: *mut xlib::Window = null_mut();
      let mut nwins = 0;
      let mut w1: xlib::Window = 0;
      let mut w2: xlib::Window = 0;
      xlib::XQueryTree(self.display,
                       self.root,
                       &mut w1,
                       &mut w2,
                       &mut wins,
                       &mut nwins);

      for &win in std::slice::from_raw_parts(wins, nwins as usize) {
        self.manage(win, true);
      }

      xlib::XFree(std::mem::transmute(wins));
    }
    Ok(())
  }

  fn get_attributes(&self, win: xlib::Window) -> xlib::XWindowAttributes {
    let mut attr = xlib::XWindowAttributes {
      all_event_masks: 0,
      backing_pixel: 0,
      backing_planes: 0,
      backing_store: 0,
      bit_gravity: 0,
      border_width: 0,
      class: 0,
      colormap: 0,
      depth: 0,
      do_not_propagate_mask: 0,
      height: 0,
      map_installed: 0,
      map_state: 0,
      override_redirect: 0,
      root: 0,
      save_under: 0,
      screen: null_mut(),
      visual: null_mut(),
      width: 0,
      win_gravity: 0,
      x: 0,
      y: 0,
      your_event_mask: 0,
    };
    unsafe {
      xlib::XGetWindowAttributes(self.display, win, &mut attr as *mut xlib::XWindowAttributes);
    }
    attr
  }

  fn manage(&mut self, win: xlib::Window, ignore_unmap: bool) {
    if self.clients.iter().find(|&cli| cli.window == win).is_some() {
      return;
    }

    let attr = self.get_attributes(win);
    if attr.override_redirect != 0 {
      return;
    }

    unsafe {
      xlib::XReparentWindow(self.display, win, self.root, 0, 0);
      xlib::XResizeWindow(self.display, win, 300, 200);
      xlib::XMapWindow(self.display, win);
    }
    self.clients.push(Entry {
      window: win,
      ignore_unmap: ignore_unmap,
    });
  }

  fn unmanage(&mut self, client: xlib::Window) {
    if let Some(pos) = self.clients.iter().position(|ref ent| ent.window == client) {
      if self.clients[pos].ignore_unmap {
        self.clients[pos].ignore_unmap = false;
      } else {
        self.clients.remove(pos);
      }
    }
  }
}

fn run() -> Result<(), &'static str> {
  let mut env = Env::new("")?;
  env.mask_events()?;
  info!("mask_events");
  env.scan_wins()?;
  info!("scan_wins: num_entries = {}", env.clients.len());

  info!("now starting main loop...");
  loop {
    let event = env.next_event();
    match event {
      Event::MapRequest(ev) => {
        info!("MapRequest");
        env.manage(ev.window, false);
      }
      Event::Unmap(ev) => {
        info!("Unmap");
        env.unmanage(ev.window)
      }
      Event::Destroy(ev) => {
        info!("Destroy");
        env.unmanage(ev.window)
      }
      Event::Unknown => info!("Unknown"),
    }
  }
}

fn main() {
  env_logger::init().unwrap();
  info!("info");
  let status = run();
  match status {
    Ok(()) => (),
    Err(mesg) => error!("error: {}", mesg),
  }
}
