use rusqlite::{self, Connection, Error, params};
use itertools::Itertools;
use std::iter::{IntoIterator};
use std::time::{SystemTime, UNIX_EPOCH};
use std::convert::TryInto;

use crate::model as m;


pub struct Repo {
    pub conn: Connection
}

impl Repo {
    pub fn new(filename: &str) -> Result<Self, Error> {
        let conn = Connection::open(filename)?;
        conn.execute("
            CREATE TABLE IF NOT EXISTS users (
                username TEXT NOT NULL,
                password TEXT NOT NULL,
                name TEXT NOT NULL,
                admin INTEGER NOT NULL DEFAUT 0
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
        return Ok(Self { conn });
    }

    pub fn get_authenticated_user_id(&self, cred: &m::UserCredentials) -> Result<Option<u32>, Error> {
        let mut stmt = self.conn.prepare("
            SELECT u.ROWID FROM users u 
            WHERE lower(u.username) = ?1 AND u.password = ?2
        ")?;

        match stmt.query_row(params![cred.username.clone().to_lowercase(), cred.password], |row| {
            Ok( row.get(0)? )
        }) {
            Ok(id) => { return Ok(Some(id)); }
            Err(Error::QueryReturnedNoRows) => { return Ok(None) }
            Err(n) => { return Err(n) }
        }
    }

    pub fn select_messages(&self) -> Result<Vec<m::MessageResponse>, Error> {
        let mut stmt = self.conn.prepare(" 
            SELECT 
                m.ROWID, m.text, m.timestamp, us.name, m.user_id
            FROM messages m
                JOIN users us ON us.ROWID = m.user_id
            ORDER BY
                m.timestamp
        ")?;

        let row_array = stmt.query_map(params![ user_id ], |row| {
            Ok( m::MessageResponse { 
                id: row.get(0)?,
                text: row.get(1)?,
                timestamp: row.get(2)?,
                sender_name: row.get(7)?,
                sender_id: row.get(8)?
            })
        })?.collect::<Result<Vec<_>,_>>()?;
        return Ok(row_array);
    }

    pub fn select_users(&self) -> Result<Vec<m::UserResponse>, Error> {
        let mut stmt = self.conn.prepare(" 
            SELECT ROWID, name FROM users ORDER BY name
        ")?;

        return Ok(stmt.query_map(params![], |row| {
            Ok(m::UserResponse {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })?.collect::<Result<Vec<_>,_>>()?);
    }

    pub fn insert_message(&self, sender_id: u32, req: m::PostMessageRequest) -> Result<(), Error> {
        let now: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Can not count time anymore")
            .as_secs()
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
        return Ok(());

    }
}

