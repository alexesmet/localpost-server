use tera;
use serde::{Deserialize, Serialize};
use crate::model;
use regex;

pub struct View {
    pub tera: tera::Tera
}

#[derive(Serialize)]
struct ViewMessage {
    sender: ViewPerson,
    text: String,
    time: String,
    recipients: Vec<ViewPerson>
}

#[derive(Serialize)]
struct ViewPerson {
    name: String,
    acronym: String,
    color: String
}

fn to_acronym(name: &str) -> String {
    return regex::Regex::new("[[:upper:]]").unwrap().find_iter(name).fold(String::new(),|a,b| a+b.as_str());
}

impl View {
    pub fn render_index(&self, msgs: Vec<model::MessageResponse>) -> tera::Result<String> {

        let view_messages: Vec<ViewMessage> = msgs.iter()
            .map(|m| ViewMessage {
                sender: ViewPerson { 
                    name: m.sender_name.clone(),
                    acronym: to_acronym(&m.sender_name),
                    color: "#AA0000".to_string() 
                },
                text: m.text.clone(),
                time: "today".to_string(),
                recipients: m.recepients.iter().map(|r| ViewPerson {
                    name: r.name.clone(),
                    acronym: to_acronym(&r.name),
                    color: "#00BB00".to_string(),
                }).collect(), 
                
            })
            .collect();

        let mut context = tera::Context::new();
        context.insert("messages", &view_messages);


        return self.tera.render("index.html", &context);
    }
}
