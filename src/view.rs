use tera;
use serde::Serialize;
use chrono::TimeZone;
use crate::model;
use regex;
use chrono;

pub struct View {
    pub tera: tera::Tera
}

#[derive(Serialize)]
struct ViewMessage {
    sender: ViewPerson,
    text: String,
    time: String,
    time_full: String,
}

#[derive(Serialize)]
struct ViewPerson {
    id: u32,
    name: String,
}

fn to_acronym(name: &str) -> String {
    return regex::Regex::new("[[:upper:]]").unwrap().find_iter(name).fold(String::new(),|a,b| a+b.as_str());
}

impl View {
    pub fn render_index(
        &self, 
        msgs: Vec<model::MessageResponse>, 
        users: Vec<model::UserResponse>
    ) -> tera::Result<String> {
        let view_messages: Vec<ViewMessage> = msgs.iter()
            .map(|m| ViewMessage {
                sender: ViewPerson { 
                    id: m.sender_id,
                    name: m.sender_name.clone(),
                },
                text: m.text.clone(),
                time: chrono::offset::Local.timestamp(m.timestamp,0).format("%H:%M").to_string(),
                time_full: chrono::offset::Local.timestamp(m.timestamp,0).format("%Y-%m-%d %H:%M:%S").to_string(),
            })
            .collect();

        let view_users: Vec<ViewPerson> = users.into_iter()
            .map(|u| ViewPerson {
                id: u.id, name: u.name.clone()
            })
            .collect();

        let mut context = tera::Context::new();
        context.insert("messages", &view_messages);
        context.insert("users", &view_users);

        return self.tera.render("index.html", &context);
    }
}
