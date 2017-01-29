use std::os::raw::{c_void, c_int, c_ulong, c_char};
use std::ffi::CString;
use std::ptr::null_mut;
use std::mem::transmute_copy;
use std::mem::zeroed;
use x11::xlib;
use libc;

enum Event {
  ButtonPress(xlib::XButtonPressedEvent),
  ButtonRelease(xlib::XButtonReleasedEvent),
  MotionNotify(xlib::XMotionEvent),
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


pub struct Env {
  display: *mut xlib::Display,
  root: xlib::Window,
  black_pixel: c_ulong,
  white_pixel: c_ulong,
  gc: xlib::GC,
  font: *mut xlib::XFontStruct,
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
  pub fn new(displayname: &str) -> Result<Env, &'static str> {
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

    let font = unsafe { xlib::XQueryFont(display, ::std::mem::transmute(gc)) };

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
      font: font,
      display: display,
    })
  }

  pub fn scan_wins(&mut self) -> Result<(), &'static str> {
    for win in self.query_tree(self.root) {
      let attr = self.get_attributes(win);
      if attr.override_redirect != 0 {
        continue;
      }
      self.manage(win, true);
    }

    trace!("scan_wins: number of entries = {}", self.clients.len());
    Ok(())
  }

  fn frame_of(&self, client: xlib::Window) -> Option<xlib::Window> {
    self.clients
      .iter()
      .find(|&ent| ent.window == client)
      .map(|ref ent| ent.frame)
  }

  fn client_of(&self, frame: xlib::Window) -> Option<xlib::Window> {
    self.clients
      .iter()
      .find(|&ent| ent.frame == frame)
      .map(|ref ent| ent.window)
  }

  fn next_event(&mut self) -> Event {
    unsafe {
      let mut ev = zeroed::<xlib::XEvent>();
      xlib::XNextEvent(self.display, &mut ev);
      match ev.get_type() {
        xlib::ButtonPress => Event::ButtonPress(transmute_copy(&ev)),
        xlib::ButtonRelease => Event::ButtonRelease(transmute_copy(&ev)),
        xlib::MotionNotify => Event::MotionNotify(transmute_copy(&ev)),
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

    let (_, x, y, width, height, ..) = self.get_geometry(win);

    let width = ::std::cmp::max(width, 600);
    let height = ::std::cmp::max(height, 400);

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
                          height + self.title_height() as u32,
                          1,
                          0,
                          0,
                          null_mut(),
                          mask,
                          &mut attr)
    };

    unsafe {
      xlib::XReparentWindow(self.display, win, frame, x, y + self.title_height());
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

  fn configure(&mut self, ev: xlib::XConfigureRequestEvent) {
    if let Some(frame) = self.frame_of(ev.window) {
      let (_, curx, cury, curwid, curht, ..) = self.get_geometry(ev.window);
      unsafe {
        xlib::XMoveResizeWindow(self.display,
                                frame,
                                curx,
                                cury - self.title_height(),
                                curwid,
                                curht + self.title_height() as u32);
        xlib::XResizeWindow(self.display, ev.window, curwid, curht);
      }
    }
  }

  fn title_height(&self) -> i32 {
    if self.font != null_mut() {
      unsafe { (*self.font).ascent + (*self.font).descent }
    } else {
      18
    }
  }

  fn paint_frame(&mut self, frame: xlib::Window) {
    let text = if let Some(client) = self.client_of(frame) {
      unsafe {
        let mut name: *mut c_char = null_mut();
        xlib::XFetchName(self.display, client, &mut name);
        CString::from_raw(name)
      }
    } else {
      CString::new("<unknown>").unwrap()
    };
    unsafe {
      xlib::XDrawString(self.display,
                        frame,
                        self.gc,
                        5,
                        if self.font != null_mut() {
                          (*self.font).ascent
                        } else {
                          14
                        },
                        text.as_ptr(),
                        libc::strlen(text.as_ptr()) as c_int);
    }
  }

  fn handle_buttonpress(&mut self, ev: xlib::XButtonPressedEvent) {
    let frame = ev.window;
    if let Some(client) = self.client_of(frame) {
      match ev.button {
        xlib::Button1 => self.move_frame(frame),
        xlib::Button2 => self.resize_frame(frame, client),
        xlib::Button3 => self.destroy_client(frame),
        _ => (),
      }
    }
  }

  fn query_pointer(&self,
                   win: xlib::Window)
                   -> (xlib::Window, xlib::Window, i32, i32, i32, i32, u32) {
    let (mut root, mut child): (xlib::Window, xlib::Window) = (0, 0);
    let (mut root_x, mut root_y, mut win_x, mut win_y, mut mask) = (0, 0, 0, 0, 0);
    unsafe {
      xlib::XQueryPointer(self.display,
                          win,
                          &mut root,
                          &mut child,
                          &mut root_x,
                          &mut root_y,
                          &mut win_x,
                          &mut win_y,
                          &mut mask);
    }
    (root, child, root_x, root_y, win_x, win_y, mask)
  }

  fn move_in_drag(&mut self, win: xlib::Window, x: i32, y: i32) -> (i32, i32) {
    loop {
      match self.next_event() {
        Event::ButtonRelease(ev) => return (ev.x_root - x, ev.y_root - y),
        Event::MotionNotify(ev) => unsafe {
          xlib::XMoveWindow(self.display, win, ev.x_root - x, ev.y_root - y);
        },
        _ => (),
      }
    }
  }

  fn move_frame(&mut self, frame: xlib::Window) {
    trace!("move_frame");
    unsafe {
      xlib::XRaiseWindow(self.display, frame);
    }
    let (_, _, _, _, win_x, win_y, ..) = self.query_pointer(frame);
    let (x, y) = self.move_in_drag(frame, win_x, win_y);
    unsafe {
      xlib::XMoveWindow(self.display, frame, x, y);
    }
  }

  fn resize_frame(&mut self, frame: xlib::Window, client: xlib::Window) {
    trace!("resize_frame");
    drop(frame);
    drop(client);
  }

  fn destroy_client(&mut self, frame: xlib::Window) {
    trace!("destroy_client");
    if let Some(client) = self.client_of(frame) {
      unsafe {
        xlib::XKillClient(self.display, client);
      }
    }
  }

  /// get all window IDs in current screen.
  fn query_tree(&self, win: xlib::Window) -> Vec<xlib::Window> {
    unsafe {
      let mut wins_ptr: *mut xlib::Window = null_mut();
      let mut nwins = 0;
      let mut w1: xlib::Window = 0;
      let mut w2: xlib::Window = 0;
      xlib::XQueryTree(self.display,
                       win,
                       &mut w1,
                       &mut w2,
                       &mut wins_ptr,
                       &mut nwins);
      let wins = Vec::from_raw_parts(wins_ptr, nwins as usize, nwins as usize);
      xlib::XFree(wins_ptr as *mut c_void);
      wins
    }
  }

  fn get_attributes(&self, win: xlib::Window) -> xlib::XWindowAttributes {
    unsafe {
      let mut attr = zeroed::<xlib::XWindowAttributes>();
      xlib::XGetWindowAttributes(self.display, win, &mut attr as *mut xlib::XWindowAttributes);
      attr
    }
  }

  fn get_geometry(&self, win: xlib::Window) -> (xlib::Window, i32, i32, u32, u32, u32, u32) {
    unsafe {
      let mut root: xlib::Window = 0;
      let (mut x, mut y, mut width, mut height, mut border_width, mut depth) = (0, 0, 0, 0, 0, 0);
      xlib::XGetGeometry(self.display,
                         win,
                         &mut root,
                         &mut x,
                         &mut y,
                         &mut width,
                         &mut height,
                         &mut border_width,
                         &mut depth);
      (root, x, y, width, height, border_width, depth)
    }
  }

  pub fn handle_event(mut self) -> Result<(), &'static str> {
    loop {
      match self.next_event() {
        Event::ButtonPress(ev) => {
          info!("event: ButtonPress");
          self.handle_buttonpress(ev);
        }
        Event::Expose(ev) => {
          info!("event: Expose");
          if ev.count == 0 {
            self.paint_frame(ev.window);
          }
        }
        Event::MapRequest(ev) => {
          info!("event: MapRequest");
          self.manage(ev.window, false);
        }
        Event::Unmap(ev) => {
          info!("event: Unmap");
          self.unmanage(ev.window)
        }
        Event::Destroy(ev) => {
          info!("event: Destroy");
          self.unmanage(ev.window)
        }
        Event::ConfigureRequest(ev) => {
          info!("event: ConfigureRequest");
          self.configure(ev);
        }
        _ => {
          info!("event: Unhandled");
        }
      }
    }
  }
}
