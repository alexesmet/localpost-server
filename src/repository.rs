use rusqlite::{self, Connection, Error, NO_PARAMS, params};
use itertools::Itertools;
use std::{iter::{FromIterator, IntoIterator}, time::UNIX_EPOCH};
use std::time::SystemTime;
use std::convert::TryInto;

use crate::model as m;


pub struct Repo {
    pub conn: Connection
}

struct MessageRow {
    id: u32,
    text: String,
    user_id: u32,
    user_name: String,
    timestamp: u32,
    sender_name: String,
    sender_id: u32
}

impl Repo {
    pub fn new(filename: &str) -> Result<Self, Error> {
        let conn = Connection::open(filename)?;
        conn.execute("
            CREATE TABLE IF NOT EXISTS users (
                token TEXT NOT NULL,
                name TEXT NOT NULL
            );
        ", rusqlite::NO_PARAMS)?;
        conn.execute("
            CREATE TABLE IF NOT EXISTS messages (
                text TEXT NOT NULL,
                user_id INTEGER,
                timestamp INTEGER,
                FOREIGN KEY (user_id) REFERENCES users(ROWID)
            )
        ", rusqlite::NO_PARAMS)?;
        conn.execute("
            CREATE TABLE IF NOT EXISTS message_recipients (
                user_id INTEGER,
                message_id INTEGER,
                FOREIGN KEY (user_id) REFERENCES users(ROWID),
                FOREIGN KEY (message_id) REFERENCES messages(ROWID)
            );
        ", rusqlite::NO_PARAMS)?;
        return Ok(Self { conn });
    }

    pub fn select_messages_by_token(&self, token: String) -> Result<Vec<m::MessageResponse>, Error> {
        let mut stmt = self.conn.prepare(" 
            SELECT 
                m.ROWID, m.text, m.timestamp, mr.user_id, ur.name, us.name, m.user_id
            FROM messages m
                JOIN message_recipients mr ON mr.message_id = m.ROWID
                JOIN users ur ON ur.ROWID = mr.user_id
                JOIN users us ON us.ROWID = m.user_id
            WHERE 
                m.ROWID IN ( 
                    SELECT message_id 
                    FROM message_recipients mr2 JOIN users u2 ON mr2.user_id = u2.ROWID
                    WHERE u2.token = ?1) 
            ORDER BY
                m.timestamp
        ")?;

        let row_array = stmt.query_map(params![ token ], |row| {
            Ok( MessageRow { 
                id: row.get(0)?,
                text: row.get(1)?,
                timestamp: row.get(2)?,
                user_id: row.get(3)?,
                user_name: row.get(4)?,
                sender_name: row.get(5)?,
                sender_id: row.get(6)?
            })
        })?.collect::<Result<Vec<_>,_>>()?;
        Ok(message_rows_to_message(row_array.into_iter()))
    }

    pub fn insert_message(&self, sender_id: u32, req: m::PostMessageRequest) -> Result<m::MessageResponse, Error> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Can not count time anymore")
            .as_millis()
            .try_into::<u32>()
            .expect("Can not count this much time");

        self.conn.execute(
            " INSERT INTO messages (text, user_id, timestamp) VALUES (?1, ?2, ?3) "
            &[req.text, sender_id, now]
        )?;

        let rowid = self.conn.last_insert_rowid()?;

        self.conn.execute(
            " INSERT INTO message_recipients () VALUES (?1, ?2, ?3) "
            &[req.text, sender_id, now]
        )?;


        return todo!();
    }
}

fn message_rows_to_message(row_array: impl Iterator<Item=MessageRow>) -> Vec<m::MessageResponse> {
    return row_array.group_by(|m| { (m.id, m.timestamp, m.sender_id, m.sender_name.clone(), m.text.clone()) })
        .into_iter()
        .map(|(key, g)| { m::MessageResponse {
            id: key.0,
            timestamp: key.1,
            sender_id: key.2,
            sender_name: key.3,
            text: key.4,
            recepients: g.map(|m| m::EmbeddedRecipient { 
                id: m.user_id, name: m.user_name
            }).collect()
        }})
        .collect();
}



