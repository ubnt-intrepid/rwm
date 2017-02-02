use x11::xlib;
use backend::{WindowSystem, Event};
use std::rc::Rc;

/// represents a window to manage.
struct Client {
  ws: Rc<WindowSystem>,
  client: xlib::Window,
  frame: xlib::Window,
  ignore_unmap: bool,
  title_height: u32,
}

impl Client {
  fn new(ws: Rc<WindowSystem>,
         client: xlib::Window,
         title_height: u32,
         ignore_unmap: bool)
         -> Client {
    // get size information from client window.
    let (_, x, y, width, height, ..) = ws.get_geometry(client);
    let width = ::std::cmp::max(width, 600);
    let height = ::std::cmp::max(height, 400);

    // create frame window
    let frame = ws.create_window(x, y, width, height + title_height);

    ws.reparent_window(client, frame, x + 5, y + 5 + title_height as i32);
    ws.resize_window(client, width - 10, height - 10);

    ws.map_window(client);
    ws.map_window(frame);

    ws.add_to_saveset(client);

    Client {
      client: client,
      frame: frame,
      ignore_unmap: ignore_unmap,
      title_height: title_height,
      ws: ws,
    }
  }

  fn resize(&self, width: u32, height: u32) {
    self.ws.resize_window(self.frame, width, height);
    self.ws.resize_window(self.client, width - 10, height - 10 - self.title_height);
  }

  fn draw(&self) {
    let text = self.ws.fetch_name(self.client);
    let y = self.ws.font().map(|ref font| font.ascent).unwrap_or(14);
    self.ws.draw_string(self.frame, text, 5, y);
  }

  fn configure(&self) {
    let (_, curx, cury, curwid, curht, ..) = self.ws.get_geometry(self.client);
    self.ws.move_window(self.frame, curx, cury - self.title_height as i32);
    self.resize(curwid, curht + self.title_height);
  }

  fn kill(&self) {
    self.ws.kill_client(self.client);
  }

  fn move_in_drag(&self) {
    self.ws.raise_window(self.frame);
    let (_, _, _, _, x, y, ..) = self.ws.query_pointer(self.frame);

    loop {
      match self.ws.next_event() {
        Event::ButtonRelease(ev) => {
          self.ws.move_window(self.frame, ev.x_root - x, ev.y_root - y);
          return;
        }
        Event::MotionNotify(ev) => {
          self.ws.move_window(self.frame, ev.x_root - x, ev.y_root - y);
        }
        _ => (),
      }
    }
  }

  fn resize_in_drag(&self) {
    self.ws.raise_window(self.frame);
    let (_, x, y, width, height, ..) = self.ws.get_geometry(self.frame);
    self.ws.warp_pointer(self.frame, x, y, width, height);

    loop {
      match self.ws.next_event() {
        Event::ButtonRelease(ev) => {
          self.resize((ev.x_root - x).abs() as u32, (ev.y_root - y).abs() as u32);
          return;
        }
        Event::MotionNotify(ev) => {
          self.resize((ev.x_root - x).abs() as u32, (ev.y_root - y).abs() as u32)
        }
        _ => (),
      }
    }
  }
}


pub struct Env {
  ws: Rc<WindowSystem>,
  clients: Vec<Client>,
}

impl Env {
  pub fn new(displayname: &str) -> Result<Env, &'static str> {
    let ws = WindowSystem::new(displayname)?;
    Ok(Env {
      ws: Rc::new(ws),
      clients: Vec::new(),
    })
  }

  pub fn scan_wins(&mut self) -> Result<(), &'static str> {
    for win in self.ws.query_tree() {
      let attr = self.ws.get_attributes(win);
      if attr.override_redirect != 0 {
        continue;
      }
      self.manage(win, true);
    }

    trace!("scan_wins: number of entries = {}", self.clients.len());
    Ok(())
  }

  pub fn handle_event(mut self) -> Result<(), &'static str> {
    loop {
      match self.ws.next_event() {
        Event::ButtonPress(xlib::XButtonPressedEvent { button, window, .. }) => {
          info!("event: ButtonPress");
          if let Some(ref client) = self.find_by_frame(window) {
            match button {
              xlib::Button1 => {
                trace!("move_frame");
                client.move_in_drag();
              }
              xlib::Button2 => {
                trace!("destroy_client");
                client.kill();
              }
              xlib::Button3 => {
                trace!("resize_frame");
                client.resize_in_drag();
              }
              _ => (),
            }
          }
        }
        Event::Expose(xlib::XExposeEvent { count, window, .. }) => {
          info!("event: Expose");
          if count == 0 {
            if let Some(ref client) = self.find_by_frame(window) {
              client.draw();
            }
          }
        }
        Event::MapRequest(xlib::XMapRequestEvent { window, .. }) => {
          info!("event: MapRequest");
          self.manage(window, false);
        }
        Event::Unmap(xlib::XUnmapEvent { window, .. }) => {
          info!("event: Unmap");
          self.unmanage(window)
        }
        Event::Destroy(xlib::XDestroyWindowEvent { window, .. }) => {
          info!("event: Destroy");
          self.unmanage(window)
        }
        Event::ConfigureRequest(xlib::XConfigureRequestEvent { window, .. }) => {
          info!("event: ConfigureRequest");
          if let Some(ref client) = self.find_by_client(window) {
            client.configure();
          }
        }
        _ => {
          info!("event: Unhandled");
        }
      }
    }
  }

  fn find_by_client(&self, client: xlib::Window) -> Option<&Client> {
    self.clients
      .iter()
      .find(|&ent| ent.client == client)
  }

  fn find_by_frame(&self, frame: xlib::Window) -> Option<&Client> {
    self.clients
      .iter()
      .find(|&ent| ent.frame == frame)
  }

  fn manage(&mut self, client: xlib::Window, ignore_unmap: bool) {
    if self.find_by_client(client).is_some() {
      return;
    }
    let client = Client::new(self.ws.clone(), client, self.title_height(), ignore_unmap);
    self.clients.push(client);
  }

  fn unmanage(&mut self, client: xlib::Window) {
    let pos = match self.clients.iter().position(|ref entry| entry.client == client) {
      Some(pos) => pos,
      None => return,
    };

    if self.clients[pos].ignore_unmap {
      self.clients[pos].ignore_unmap = false;
    } else {
      self.ws.destroy_window(self.clients[pos].frame);
      self.clients.remove(pos);
    }
  }

  fn title_height(&self) -> u32 {
    self.ws.font().map(|ref font| (font.ascent + font.descent) as u32).unwrap_or(18)
  }
}
