use rusqlite::{self, Connection, Error, params};
use itertools::Itertools;
use std::iter::{IntoIterator};
use std::time::{SystemTime, UNIX_EPOCH};
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
                username TEXT NOT NULL,
                password TEXT NOT NULL,
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

    pub fn get_authenticated_user_id(&self, cred: &m::UserCredentials) -> Result<Option<u32>, Error> {
        let mut stmt = self.conn.prepare("
            SELECT u.ROWID FROM users u 
            WHERE u.username = ?1 AND u.password = ?2
        ")?;

        match stmt.query_row(params![cred.username, cred.password], |row| {
            Ok( row.get(0)? )
        }) {
            Ok(id) => { return Ok(Some(id)); }
            Err(Error::QueryReturnedNoRows) => { return Ok(None) }
            Err(n) => { return Err(n) }
        }
    }

    pub fn register_user(&self, cred: &m::UserCredentials) -> Result<Option<u32>, Error> {
        let updated_rows = self.conn.execute("
            UPDATE users
            SET password = ?1
            WHERE username = :2 AND password = ''
        ", params![ cred.password, cred.username ])?;

        if updated_rows > 0 {
            return self.get_authenticated_user_id(cred);
        } else {
            return Ok(None);
        }
        
    }

    pub fn select_messages_for_user(&self, user_id: u32) -> Result<Vec<m::MessageResponse>, Error> {
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
                    WHERE u2.ROWID = ?1) 
            ORDER BY
                m.timestamp
        ")?;

        let row_array = stmt.query_map(params![ user_id ], |row| {
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

    pub fn select_message_by_id(&self, rowid: u32) -> Result<m::MessageResponse, Error> {
        let mut stmt = self.conn.prepare(" 
            SELECT 
                m.ROWID, m.text, m.timestamp, mr.user_id, ur.name, us.name, m.user_id
            FROM messages m
                JOIN message_recipients mr ON mr.message_id = m.ROWID
                JOIN users ur ON ur.ROWID = mr.user_id
                JOIN users us ON us.ROWID = m.user_id
            WHERE 
                m.ROWID = ?1
        ")?;

        let row_array = stmt.query_map(params![ rowid ], |row| {
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
        return message_rows_to_message(row_array.into_iter())
            .into_iter()
            .next()
            .ok_or(Error::QueryReturnedNoRows);
    }

    pub fn insert_message(&self, sender_id: u32, req: m::PostMessageRequest) -> Result<m::MessageResponse, Error> {
        let now: u32 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Can not count time anymore")
            .as_millis()
            .try_into()
            .expect("Can not count this much time");

        self.conn.execute(
            " INSERT INTO messages (text, user_id, timestamp) VALUES (?1, ?2, ?3) ",
            &[req.text.clone(), sender_id.to_string(), now.to_string()]
        )?;

        let rowid = self.conn.last_insert_rowid();
        // TODO: Wrap in transaction 
        for recp in req.recipients.iter() {
            self.conn.execute(
                "INSERT INTO message_recipients (user_id, message_id) VALUES (?1, ?2)",
                params![ recp, rowid ]
            )?;
        }
        return self.select_message_by_id(rowid.try_into().unwrap());

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



