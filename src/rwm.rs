use std::os::raw::{c_void, c_int};
use std::ffi::CString;
use std::ptr::null_mut;

use x11::xlib;


enum Event {
  MapRequest(xlib::XMapRequestEvent),
  Unmap(xlib::XUnmapEvent),
  Destroy(xlib::XDestroyWindowEvent),
  Unknown,
}


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

extern "C" fn error_handler(_: *mut xlib::Display, _: *mut xlib::XErrorEvent) -> c_int {
  0
}

impl Env {
  fn new(displayname: &str) -> Result<Env, &'static str> {
    let displayname = CString::new(displayname).unwrap();
    let display = unsafe { xlib::XOpenDisplay(displayname.as_ptr()) };
    if display == null_mut() {
      return Err("can't open display");
    }
    unsafe {
      xlib::XSetErrorHandler(Some(error_handler));
    }

    let screen = unsafe { xlib::XDefaultScreenOfDisplay(display) };
    let root = unsafe { xlib::XRootWindowOfScreen(screen) };

    Ok(Env {
      clients: Vec::new(),
      root: root,
      display: display,
    })
  }

  fn mask_events(&mut self) -> Result<(), &'static str> {
    let mut attr = unsafe { ::std::mem::zeroed::<xlib::XSetWindowAttributes>() };
    attr.event_mask = xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask;
    unsafe {
      xlib::XChangeWindowAttributes(self.display,
                                    self.root,
                                    xlib::CWCursor | xlib::CWEventMask,
                                    &mut attr)
    };

    unsafe { xlib::XSync(self.display, xlib::False) };
    Ok(())
  }

  fn next_event(&mut self) -> Event {
    unsafe {
      let mut ev = ::std::mem::zeroed::<xlib::XEvent>();
      xlib::XNextEvent(self.display, &mut ev);
      match ev.get_type() {
        xlib::MapRequest => Event::MapRequest(::std::mem::transmute_copy(&ev)),
        xlib::UnmapNotify => Event::Unmap(::std::mem::transmute_copy(&ev)),
        xlib::DestroyNotify => Event::Destroy(::std::mem::transmute_copy(&ev)),
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

      for &win in ::std::slice::from_raw_parts(wins, nwins as usize) {
        self.manage(win, true);
      }

      xlib::XFree(wins as *mut c_void);
    }

    info!("scan_wins: number of entries = {}", self.clients.len());
    Ok(())
  }

  fn get_attributes(&self, win: xlib::Window) -> xlib::XWindowAttributes {
    unsafe {
      let mut attr = ::std::mem::zeroed::<xlib::XWindowAttributes>();
      xlib::XGetWindowAttributes(self.display, win, &mut attr as *mut xlib::XWindowAttributes);
      attr
    }
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
    let pos = match self.clients.iter().position(|ref ent| ent.window == client) {
      Some(pos) => pos,
      None => return,
    };

    if self.clients[pos].ignore_unmap {
      self.clients[pos].ignore_unmap = false;
    } else {
      self.clients.remove(pos);
    }
  }
}


pub fn run() -> Result<(), &'static str> {
  let mut env = Env::new("")?;
  env.mask_events()?;
  info!("mask_events");
  env.scan_wins()?;

  info!("now starting main loop...");
  loop {
    match env.next_event() {
      Event::MapRequest(ev) => {
        info!("event: MapRequest");
        env.manage(ev.window, false);
      }
      Event::Unmap(ev) => {
        info!("event: Unmap");
        env.unmanage(ev.window)
      }
      Event::Destroy(ev) => {
        info!("event Destroy");
        env.unmanage(ev.window)
      }
      Event::Unknown => info!("Unknown event"),
    }
  }
}
