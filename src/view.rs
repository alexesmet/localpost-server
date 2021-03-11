use tera;
use serde::Serialize;
use chrono::TimeZone;
use crate::model;
use chrono;

pub struct View {
    pub tera: tera::Tera
}

#[derive(Serialize)]
struct ViewMessage {
    id: u32,
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

impl View {
    pub fn render_index(
        &self, 
        msgs: Vec<model::MessageResponse>, 
        user: Option<model::UserResponse>
    ) -> tera::Result<String> {
        let view_messages: Vec<ViewMessage> = msgs.iter()
            .map(|m| ViewMessage {
                id: m.id,
                sender: ViewPerson { 
                    id: m.sender_id,
                    name: m.sender_name.clone(),
                },
                text: m.text.clone(),
                time: chrono::offset::Local.timestamp(m.timestamp,0).format("%Y-%m-%d").to_string(),
                time_full: chrono::offset::Local.timestamp(m.timestamp,0).format("%Y-%m-%d %H:%M:%S").to_string(),
            })
            .collect();

        let mut context = tera::Context::new();
        context.insert("messages", &view_messages);
        context.insert("account", &user);

        return self.tera.render("index.html", &context);
    }

    pub fn render_admin(
        &self, 
        users: Vec<model::UserResponse>, 
    ) -> tera::Result<String> {

        let mut context = tera::Context::new();
        context.insert("users", &users);

        return self.tera.render("admin.html", &context);
    }

    pub fn render_user(
        &self, 
        user: Option<&model::UserResponse>
    ) -> tera::Result<String> {

        let mut context = tera::Context::new();
        context.insert("user", &user);

        return self.tera.render("user.html", &context);
    }
}
