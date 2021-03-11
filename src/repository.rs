use rusqlite::{self, Connection, Error, params};
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
                admin INTEGER NOT NULL DEFAULT 0
            );
        ", rusqlite::NO_PARAMS)?;
        conn.execute("
            CREATE TABLE IF NOT EXISTS messages (
                text TEXT NOT NULL,
                user_id INTEGER,
                timestamp INTEGER,
                FOREIGN KEY (user_id) REFERENCES users(ROWID) ON DELETE CASCADE
            )
        ", rusqlite::NO_PARAMS)?;
        return Ok(Self { conn });
    }

    pub fn get_authenticated_user(&self, cred: &m::UserCredentials) -> Result<Option<m::UserResponse>, Error> {
        let mut stmt = self.conn.prepare("
            SELECT u.ROWID, u.name, u.username, u.admin, u.password FROM users u 
            WHERE lower(u.username) = ?1 AND u.password = ?2
        ")?;

        match stmt.query_row(params![cred.username.clone().to_lowercase(), cred.password], |row| {
            return Ok(m::UserResponse {
                id: row.get(0)?,
                name: row.get(1)?,
                username: row.get(2)?,
                admin: row.get(3)?,
                password: row.get(4)?
            });
        }) {
            Ok(id) => { return Ok(Some(id)); }
            Err(Error::QueryReturnedNoRows) => { return Ok(None) }
            Err(n) => { return Err(n) }
        }
    }

    pub fn delete_user(&self, user_id: u32) -> Result<(), Error> {
        self.conn.execute(
            " DELETE FROM users WHERE ROWID = ?1",
            &[user_id]
        )?;
        return Ok(());
    }

    pub fn delete_message(&self, user_id: u32) -> Result<(), Error> {
        self.conn.execute(
            " DELETE FROM messages WHERE ROWID = ?1",
            &[user_id]
        )?;
        return Ok(());
    }

    pub fn update_user(&self, u: m::UserResponse) -> Result<(), Error> {
        self.conn.execute(
            "UPDATE users SET name = ?1, username = ?2, password = ?3, admin = ?4 WHERE ROWID = ?5",
            params![u.name, u.username, u.password, if u.admin {1} else {0}, u.id]
        )?;
        return Ok(());
    }

    pub fn create_user(&self, u: m::UserResponse) -> Result<(), Error> {
        self.conn.execute(
            "INSERT INTO users (name, username, password, admin) VALUES (?1, ?2, ?3, ?4)",
            params![u.name, u.username, u.password, if u.admin {1} else {0}]
        )?;
        return Ok(());
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

        let row_array = stmt.query_map(params![], |row| {
            Ok( m::MessageResponse { 
                id: row.get(0)?,
                text: row.get(1)?,
                timestamp: row.get(2)?,
                sender_name: row.get(3)?,
                sender_id: row.get(4)?
            })
        })?.collect::<Result<Vec<_>,_>>()?;
        return Ok(row_array);
    }

    pub fn select_users(&self) -> Result<Vec<m::UserResponse>, Error> {
        let mut stmt = self.conn.prepare(" 
            SELECT ROWID, name, username, admin, password FROM users ORDER BY name
        ")?;

        return Ok(stmt.query_map(params![], |row| {
            return Ok(m::UserResponse {
                id: row.get(0)?,
                name: row.get(1)?,
                username: row.get(2)?,
                admin: row.get(3)?,
                password: row.get(4)?
            });
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
        return Ok(());

    }
}

