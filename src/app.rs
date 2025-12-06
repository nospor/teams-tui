use crate::api::{Chat, Message};

pub struct App {
    pub chats: Vec<Chat>,
    pub status: String,
    pub selected_index: usize,
    pub current_user_name: Option<String>,
    pub messages: Vec<Message>,
    pub loading_messages: bool,
    pub input_mode: bool,
    pub input_buffer: String,
    pub scroll_offset: u16,
    pub max_scroll: u16,
    pub snap_to_bottom: bool,
}

impl App {
    pub fn new() -> App {
        App {
            chats: Vec::new(),
            status: "Loading...".to_string(),
            selected_index: 0,
            current_user_name: None,
            messages: Vec::new(),
            loading_messages: false,
            input_mode: false,
            input_buffer: String::new(),
            scroll_offset: 0,
            max_scroll: 0,
            snap_to_bottom: true,
        }
    }

    pub fn set_chats(&mut self, chats: Vec<Chat>) {
        self.chats = chats;
        self.status = format!("Loaded {} chats", self.chats.len());
    }

    pub fn set_current_user(&mut self, name: String) {
        self.current_user_name = Some(name);
    }

    pub fn set_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
        self.loading_messages = false;
    }

    pub fn set_loading_messages(&mut self, loading: bool) {
        self.loading_messages = loading;
    }

    pub fn get_selected_chat(&self) -> Option<&Chat> {
        self.chats.get(self.selected_index)
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
