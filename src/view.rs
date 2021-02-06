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
    recipients: Vec<ViewPerson>
}

#[derive(Serialize)]
struct ViewPerson {
    id: u32,
    name: String,
    acronym: String,
    color: String
}

fn to_acronym(name: &str) -> String {
    return regex::Regex::new("[[:upper:]]").unwrap().find_iter(name).fold(String::new(),|a,b| a+b.as_str());
}

impl View {
    pub fn render_index(
        &self, 
        msgs: Vec<model::MessageResponse>, 
        users: Vec<model::EmbeddedRecipient>
    ) -> tera::Result<String> {
        let view_messages: Vec<ViewMessage> = msgs.iter()
            .map(|m| ViewMessage {
                sender: ViewPerson { 
                    id: m.sender_id,
                    name: m.sender_name.clone(),
                    acronym: to_acronym(&m.sender_name),
                    color: m.sender_color.clone()
                },
                text: m.text.clone(),
                time: chrono::offset::Local.timestamp(m.timestamp,0).format("%H:%M").to_string(),
                time_full: chrono::offset::Local.timestamp(m.timestamp,0).format("%Y-%m-%d %H:%M:%S").to_string(),
                recipients: m.recipients.iter().map(|r| ViewPerson {
                    id: r.id,
                    name: r.name.clone(),
                    acronym: to_acronym(&r.name),
                    color: r.color.clone(),
                }).collect(), 
                
            })
            .collect();

        let view_users: Vec<ViewPerson> = users.into_iter()
            .map(|u| ViewPerson {
                id: u.id, color: u.color, name: u.name.clone(), acronym: to_acronym(&u.name)
            })
            .collect();

        let mut context = tera::Context::new();
        context.insert("messages", &view_messages);
        context.insert("users", &view_users);

        return self.tera.render("index.html", &context);
    }
}
