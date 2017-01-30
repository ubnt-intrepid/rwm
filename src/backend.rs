use x11::xlib;

pub enum Event {
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
