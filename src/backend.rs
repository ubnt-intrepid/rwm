use std::os::raw::{c_void, c_int, c_char};
use std::ptr::null_mut;
use std::mem::zeroed;
use std::ffi::CString;
use std::mem::transmute_copy;
use x11::xlib;
use libc;

pub enum Event {
  ButtonPress(xlib::XButtonPressedEvent),
  ButtonRelease(xlib::XButtonReleasedEvent),
  MotionNotify(xlib::XMotionEvent),
  Expose(xlib::XExposeEvent),
  MapRequest(xlib::XMapRequestEvent),
  Unmap(xlib::XUnmapEvent),
  Destroy(xlib::XDestroyWindowEvent),
  ConfigureRequest(xlib::XConfigureRequestEvent),
  PropertyNotify(xlib::XPropertyEvent),
  Unknown,
}

pub struct WindowSystem {
  display: *mut xlib::Display,
  root: xlib::Window,
  gc: xlib::GC,
  font: *mut xlib::XFontStruct,
  // fg: xlib::XColor,
  bg: xlib::XColor,
  bd: xlib::XColor,
}

impl Drop for WindowSystem {
  fn drop(&mut self) {
    unsafe {
      xlib::XFreeFont(self.display, self.font);
      xlib::XFreeGC(self.display, self.gc);
      xlib::XCloseDisplay(self.display);
    }
    self.font = null_mut();
    self.gc = null_mut();
    self.display = null_mut();
  }
}

extern "C" fn error_handler(_: *mut xlib::Display, _: *mut xlib::XErrorEvent) -> c_int {
  0
}

impl WindowSystem {
  pub fn new(displayname: &str) -> Result<WindowSystem, &'static str> {
    let display = Self::open_display(displayname)?;
    unsafe {
      xlib::XSetErrorHandler(Some(error_handler));
    }

    let screen = unsafe { xlib::XDefaultScreenOfDisplay(display) };

    let root = unsafe {
      let root = xlib::XRootWindowOfScreen(screen);

      let mut attr = zeroed::<xlib::XSetWindowAttributes>();
      attr.event_mask = xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask;
      xlib::XChangeWindowAttributes(display, root, xlib::CWEventMask, &mut attr);

      root
    };

    let gc = unsafe {
      let mut gv = zeroed::<xlib::XGCValues>();
      xlib::XCreateGC(display, root, 0, &mut gv as *mut xlib::XGCValues)
    };
    let font = unsafe { xlib::XQueryFont(display, ::std::mem::transmute(gc)) };

    let def_cmap = unsafe { xlib::XDefaultColormap(display, xlib::XDefaultScreen(display)) };
    let (mut fg, mut bg, mut bd): (xlib::XColor, xlib::XColor, xlib::XColor) =
      unsafe { (zeroed(), zeroed(), zeroed()) };
    let mut exact: xlib::XColor = unsafe { zeroed() };
    unsafe {
      xlib::XAllocNamedColor(display,
                             def_cmap,
                             CString::new("black").unwrap().as_ptr(),
                             &mut fg,
                             &mut exact);
      xlib::XAllocNamedColor(display,
                             def_cmap,
                             CString::new("yellow").unwrap().as_ptr(),
                             &mut bg,
                             &mut exact);
      xlib::XAllocNamedColor(display,
                             def_cmap,
                             CString::new("red").unwrap().as_ptr(),
                             &mut bd,
                             &mut exact);
    }

    Ok(WindowSystem {
      display: display,
      root: root,
      gc: gc,
      font: font,
      // fg: fg,
      bg: bg,
      bd: bd,
    })
  }

  fn open_display(displayname: &str) -> Result<*mut xlib::Display, &'static str> {
    let displayname = CString::new(displayname).unwrap();
    let display = unsafe { xlib::XOpenDisplay(displayname.as_ptr()) };
    if display == null_mut() {
      return Err("can't open display");
    }
    Ok(display)
  }

  /// get all window IDs in current screen.
  pub fn query_tree(&self) -> Vec<xlib::Window> {
    unsafe {
      let mut wins_ptr: *mut xlib::Window = null_mut();
      let mut nwins = 0;
      let mut w1: xlib::Window = 0;
      let mut w2: xlib::Window = 0;
      xlib::XQueryTree(self.display,
                       self.root,
                       &mut w1,
                       &mut w2,
                       &mut wins_ptr,
                       &mut nwins);
      let wins = Vec::from_raw_parts(wins_ptr, nwins as usize, nwins as usize);
      xlib::XFree(wins_ptr as *mut c_void);
      wins
    }
  }

  pub fn get_attributes(&self, win: xlib::Window) -> xlib::XWindowAttributes {
    unsafe {
      let mut attr = zeroed::<xlib::XWindowAttributes>();
      xlib::XGetWindowAttributes(self.display, win, &mut attr as *mut xlib::XWindowAttributes);
      attr
    }
  }

  pub fn get_geometry(&self, win: xlib::Window) -> (xlib::Window, i32, i32, u32, u32, u32, u32) {
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

  pub fn query_pointer(&self,
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

  pub fn next_event(&self) -> Event {
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
        xlib::PropertyNotify => Event::PropertyNotify(transmute_copy(&ev)),
        _ => Event::Unknown,
      }
    }
  }

  pub fn create_window(&self, x: i32, y: i32, width: u32, height: u32) -> xlib::Window {
    unsafe {
      let mut attr = zeroed::<xlib::XSetWindowAttributes>();
      attr.override_redirect = xlib::True;
      attr.background_pixel = self.bg.pixel;
      attr.border_pixel = self.bd.pixel;
      attr.event_mask = xlib::SubstructureRedirectMask | xlib::SubstructureNotifyMask |
                        xlib::ButtonPressMask | xlib::ButtonReleaseMask |
                        xlib::ButtonMotionMask | xlib::ExposureMask |
                        xlib::EnterWindowMask;
      let mask = xlib::CWEventMask | xlib::CWBackPixel | xlib::CWBorderPixel |
                 xlib::CWOverrideRedirect;
      xlib::XCreateWindow(self.display,
                          self.root,
                          x,
                          y,
                          width,
                          height,
                          1,
                          0,
                          0,
                          null_mut(),
                          mask,
                          &mut attr)
    }
  }

  pub fn move_window(&self, win: xlib::Window, x: i32, y: i32) {
    unsafe {
      xlib::XMoveWindow(self.display, win, x, y);
    }
  }

  pub fn resize_window(&self, win: xlib::Window, width: u32, height: u32) {
    unsafe {
      xlib::XResizeWindow(self.display, win, width, height);
    }
  }

  pub fn add_to_saveset(&self, win: xlib::Window) {
    unsafe {
      xlib::XChangeSaveSet(self.display, win, xlib::SetModeInsert);
    }
  }

  pub fn reparent_window(&self, win: xlib::Window, parent: xlib::Window, x: i32, y: i32) {
    unsafe {
      xlib::XReparentWindow(self.display, win, parent, x, y);
    }
  }

  pub fn map_window(&self, win: xlib::Window) {
    unsafe {
      xlib::XMapWindow(self.display, win);
    }
  }

  pub fn destroy_window(&self, win: xlib::Window) {
    unsafe {
      xlib::XDestroyWindow(self.display, win);
    }
  }

  pub fn clear_window(&self, win: xlib::Window) {
    unsafe {
      xlib::XClearWindow(self.display, win);
    }
  }

  pub fn fetch_name(&self, win: xlib::Window) -> String {
    use std::ffi::CStr;
    unsafe {
      let mut name: *mut c_char = null_mut();
      xlib::XFetchName(self.display, win, &mut name);
      CStr::from_ptr(name).to_string_lossy().into_owned()
    }
  }

  pub fn draw_string(&self, win: xlib::Window, text: String, x: i32, y: i32) {
    let ctext = CString::new(text).unwrap();
    unsafe {
      xlib::XDrawString(self.display,
                        win,
                        self.gc,
                        x,
                        y,
                        ctext.as_ptr(),
                        libc::strlen(ctext.as_ptr()) as c_int);
    }
  }

  pub fn raise_window(&self, win: xlib::Window) {
    unsafe {
      xlib::XRaiseWindow(self.display, win);
    }
  }

  pub fn kill_client(&self, win: xlib::Window) {
    unsafe {
      xlib::XKillClient(self.display, win);
    }
  }

  pub fn warp_pointer(&self, win: xlib::Window, x: i32, y: i32, width: u32, height: u32) {
    unsafe {
      xlib::XWarpPointer(self.display,
                         0,
                         win,
                         x,
                         y,
                         width,
                         height,
                         width as i32,
                         height as i32);
    }
  }

  pub fn font(&self) -> Option<&xlib::XFontStruct> {
    if self.font != null_mut() {
      Some(unsafe { ::std::mem::transmute(self.font) })
    } else {
      None
    }
  }
}
