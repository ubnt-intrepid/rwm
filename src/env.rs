use x11::xlib;
use backend::{WindowSystem, Event};

struct Client {
  client: xlib::Window,
  frame: xlib::Window,
  ignore_unmap: bool,
}

pub struct Env {
  ws: WindowSystem,
  clients: Vec<Client>,
}

impl Env {
  pub fn new(displayname: &str) -> Result<Env, &'static str> {
    let ws = WindowSystem::new(displayname)?;
    Ok(Env {
      ws: ws,
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
        Event::ButtonPress(ev) => {
          info!("event: ButtonPress");
          if let Some(ref client) = self.find_by_frame(ev.window) {
            match ev.button {
              xlib::Button1 => {
                trace!("move_frame");
                self.ws.raise_window(client.frame);
                let (_, _, _, _, win_x, win_y, ..) = self.ws.query_pointer(client.frame);
                let (x, y) = self.move_in_drag(client, win_x, win_y);
                self.ws.move_window(client.frame, x, y);
              }
              xlib::Button2 => {
                trace!("destroy_client");
                self.ws.kill_client(client.client);
              }
              xlib::Button3 => {
                trace!("resize_frame");
                let (_, x, y, width, height, ..) = self.ws.get_geometry(client.frame);
                self.ws.warp_pointer(client.frame, x, y, width, height);
                let (newx, newy) = self.resize_in_drag(client, x, y);
                let (newwidth, newheight) = ((newx - x).abs() as u32, (newy - y).abs() as u32);
                self.resize_client(client, newwidth, newheight);
              }
              _ => (),
            }
          }
        }
        Event::Expose(ev) => {
          info!("event: Expose");
          if ev.count == 0 {
            if let Some(ref client) = self.find_by_frame(ev.window) {
              self.paint_frame(client);
            }
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
          if let Some(ref client) = self.find_by_client(ev.window) {
            self.configure(client);
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
    if self.clients.iter().find(|&entry| entry.client == client).is_some() {
      return;
    }

    let (_, x, y, width, height, ..) = self.ws.get_geometry(client);
    let width = ::std::cmp::max(width, 600);
    let height = ::std::cmp::max(height, 400);

    let frame = self.ws.create_window(x, y, width, height + self.title_height() as u32);
    self.ws.reparent_window(client, frame, x, y + self.title_height());
    self.ws.resize_window(client, width, height);
    self.ws.map_window(client);
    self.ws.map_window(frame);
    self.ws.add_to_saveset(client);

    self.clients.push(Client {
      client: client,
      frame: frame,
      ignore_unmap: ignore_unmap,
    });
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

  fn configure(&self, client: &Client) {
    let (_, curx, cury, curwid, curht, ..) = self.ws.get_geometry(client.client);
    self.ws.move_window(client.frame, curx, cury - self.title_height());
    self.ws.resize_window(client.frame, curwid, curht + self.title_height() as u32);
    self.ws.resize_window(client.client, curwid, curht);
  }

  fn title_height(&self) -> i32 {
    self.ws.font().map(|ref font| font.ascent + font.descent).unwrap_or(18)
  }

  fn paint_frame(&self, client: &Client) {
    let text = self.ws.fetch_name(client.client);
    let y = self.ws.font().map(|ref font| font.ascent).unwrap_or(14);
    self.ws.draw_string(client.frame, text, 5, y);
  }

  fn move_in_drag(&self, client: &Client, x: i32, y: i32) -> (i32, i32) {
    loop {
      match self.ws.next_event() {
        Event::ButtonRelease(ev) => return (ev.x_root - x, ev.y_root - y),
        Event::MotionNotify(ev) => self.ws.move_window(client.frame, ev.x_root - x, ev.y_root - y),
        _ => (),
      }
    }
  }

  fn resize_in_drag(&self, client: &Client, x: i32, y: i32) -> (i32, i32) {
    loop {
      match self.ws.next_event() {
        Event::ButtonRelease(ev) => return (ev.x_root, ev.y_root),
        Event::MotionNotify(ev) => {
          self.resize_client(client,
                             (ev.x_root - x).abs() as u32,
                             (ev.y_root - y).abs() as u32)
        }
        _ => (),
      }
    }
  }

  fn resize_client(&self, client: &Client, width: u32, height: u32) {
    self.ws.resize_window(client.frame, width, height);
    self.ws.resize_window(client.client, width, height - self.title_height() as u32);
  }
}
