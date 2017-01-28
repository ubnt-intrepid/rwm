use std::os::raw::{c_void, c_int, c_uint, c_ulong};
use std::ffi::CString;
use std::ptr::null_mut;
use std::mem::transmute_copy;
use std::mem::zeroed;
use x11::xlib;

const TITLE_HEIGHT: c_uint = 32;


enum Event {
  ButtonPress(xlib::XButtonPressedEvent),
  Expose(xlib::XExposeEvent),
  MapRequest(xlib::XMapRequestEvent),
  Unmap(xlib::XUnmapEvent),
  Destroy(xlib::XDestroyWindowEvent),
  ConfigureRequest(xlib::XConfigureRequestEvent),
  Unknown,
}


struct Entry {
  window: xlib::Window,
  frame: xlib::Window,
  ignore_unmap: bool,
}


struct Env {
  display: *mut xlib::Display,
  root: xlib::Window,
  black_pixel: c_ulong,
  white_pixel: c_ulong,
  gc: xlib::GC,
  clients: Vec<Entry>,
}

impl Drop for Env {
  fn drop(&mut self) {
    unsafe {
      xlib::XFreeGC(self.display, self.gc);
      xlib::XCloseDisplay(self.display);
    }
    self.gc = null_mut();
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
    let black_pixel = unsafe { xlib::XBlackPixelOfScreen(screen) };
    let white_pixel = unsafe { xlib::XWhitePixelOfScreen(screen) };

    let gc = unsafe {
      let mut gv = zeroed::<xlib::XGCValues>();
      xlib::XCreateGC(display, root, 0, &mut gv as *mut xlib::XGCValues)
    };

    unsafe {
      let mut attr = zeroed::<xlib::XSetWindowAttributes>();
      attr.event_mask = xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask;
      xlib::XChangeWindowAttributes(display, root, xlib::CWEventMask, &mut attr);
    }

    unsafe { xlib::XSync(display, xlib::False) };

    Ok(Env {
      clients: Vec::new(),
      root: root,
      black_pixel: black_pixel,
      white_pixel: white_pixel,
      gc: gc,
      display: display,
    })
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
        let mut attr = zeroed::<xlib::XWindowAttributes>();
        xlib::XGetWindowAttributes(self.display, win, &mut attr as *mut xlib::XWindowAttributes);
        if attr.override_redirect != 0 {
          continue;
        }
        self.manage(win, true);
      }

      xlib::XFree(wins as *mut c_void);
    }

    info!("scan_wins: number of entries = {}", self.clients.len());
    Ok(())
  }

  fn next_event(&mut self) -> Event {
    unsafe {
      let mut ev = zeroed::<xlib::XEvent>();
      xlib::XNextEvent(self.display, &mut ev);
      match ev.get_type() {
        xlib::ButtonPress => Event::ButtonPress(transmute_copy(&ev)),
        xlib::Expose => Event::Expose(transmute_copy(&ev)),
        xlib::MapRequest => Event::MapRequest(transmute_copy(&ev)),
        xlib::UnmapNotify => Event::Unmap(transmute_copy(&ev)),
        xlib::DestroyNotify => Event::Destroy(transmute_copy(&ev)),
        xlib::ConfigureRequest => Event::ConfigureRequest(transmute_copy(&ev)),
        _ => Event::Unknown,
      }
    }
  }

  fn manage(&mut self, win: xlib::Window, ignore_unmap: bool) {
    if self.clients.iter().find(|&cli| cli.window == win).is_some() {
      return;
    }

    let mut root: xlib::Window = 0;
    let (mut x, mut y, mut width, mut height, mut border_width, mut depth) = (0, 0, 0, 0, 0, 0);
    unsafe {
      xlib::XGetGeometry(self.display,
                         win,
                         &mut root,
                         &mut x,
                         &mut y,
                         &mut width,
                         &mut height,
                         &mut border_width,
                         &mut depth);
    }

    let width = ::std::cmp::max(width, 100);
    let height = ::std::cmp::max(height, 100);

    let frame = unsafe {
      let mut attr = zeroed::<xlib::XSetWindowAttributes>();
      attr.override_redirect = xlib::True;
      attr.background_pixel = self.white_pixel;
      attr.border_pixel = self.black_pixel;
      attr.event_mask = xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask |
                        xlib::ButtonPressMask | xlib::ButtonReleaseMask |
                        xlib::ButtonMotionMask | xlib::ExposureMask;
      let mask = xlib::CWEventMask | xlib::CWBackPixel | xlib::CWBorderPixel |
                 xlib::CWOverrideRedirect;
      xlib::XCreateWindow(self.display,
                          self.root,
                          x,
                          y,
                          width,
                          height + TITLE_HEIGHT,
                          1,
                          0,
                          0,
                          null_mut(),
                          mask,
                          &mut attr)
    };

    unsafe {
      xlib::XReparentWindow(self.display, win, frame, x, y + TITLE_HEIGHT as c_int);
      xlib::XResizeWindow(self.display, win, width, height);
      xlib::XMapWindow(self.display, win);
      xlib::XMapWindow(self.display, frame);
      xlib::XChangeSaveSet(self.display, win, xlib::SetModeInsert);
    }

    self.clients.push(Entry {
      window: win,
      frame: frame,
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
      unsafe {
        xlib::XDestroyWindow(self.display, self.clients[pos].frame);
      }
      self.clients.remove(pos);
    }
  }
}


pub fn run() -> Result<(), &'static str> {
  let mut env = Env::new("")?;
  env.scan_wins()?;

  info!("starting main loop...");
  loop {
    match env.next_event() {
      Event::ButtonPress(_) => {
        info!("event: ButtonPress");
      }
      Event::Expose(_) => {
        info!("event: Expose");
      }
      Event::MapRequest(ev) => {
        info!("event: MapRequest");
        env.manage(ev.window, false);
      }
      Event::Unmap(ev) => {
        info!("event: Unmap");
        env.unmanage(ev.window)
      }
      Event::Destroy(ev) => {
        info!("event: Destroy");
        env.unmanage(ev.window)
      }
      Event::ConfigureRequest(_) => {
        info!("event: ConfigureRequest");
      }
      Event::Unknown => info!("event: Unknown"),
    }
  }
}
