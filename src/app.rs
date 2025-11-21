use crate::api::Chat;

pub struct App {
    pub should_quit: bool,
    pub chats: Vec<Chat>,
    pub status: String,
    pub selected_index: usize,
}

impl App {
    pub fn new() -> App {
        App {
            should_quit: false,
            chats: Vec::new(),
            status: "Loading...".to_string(),
            selected_index: 0,
        }
    }

    pub fn set_chats(&mut self, chats: Vec<Chat>) {
        self.chats = chats;
        self.status = format!("Loaded {} chats", self.chats.len());
    }

    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    pub fn next_chat(&mut self) {
        if !self.chats.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.chats.len();
        }
    }

    pub fn previous_chat(&mut self) {
        if !self.chats.is_empty() {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = self.chats.len() - 1;
            }
        }
    }
}
