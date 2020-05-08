use diesel_migrations::embed_migrations;
use gotham::state::FromState;
use gotham_derive::StateData;

use std::sync::{Arc, Mutex, MutexGuard};

pub use diesel::pg::PgConnection as Connection;

pub type DieselResult<T> = Result<T, diesel::result::Error>;

embed_migrations!();

/// The wrapper for a database connection that can shared via gotham's state data
#[derive(Clone, StateData)]
pub struct DbConnection {
    connection: Arc<Mutex<Connection>>,
}

impl DbConnection {
    pub fn from_url(url: &str) -> Self {
        Self {
            connection: Arc::new(Mutex::new(connect(url).expect("database error"))),
        }
    }

    pub fn from_state(
        state: &gotham::state::State,
    ) -> Result<MutexGuard<Connection>, failure::Error> {
        Self::borrow_from(state).lock()
    }

    pub fn get(&self) -> Arc<Mutex<Connection>> {
        self.connection.clone()
    }

    pub fn lock(&self) -> Result<MutexGuard<Connection>, failure::Error> {
        match self.connection.lock() {
            Ok(lock) => Ok(lock),
            Err(_) => Err(failure::err_msg("failed to get lock")),
        }
    }
}

pub fn connect(url: &str) -> Result<Connection, failure::Error> {
    let connection = diesel::Connection::establish(url)?;

    // Run migrations.
    embedded_migrations::run_with_output(&connection, &mut std::io::stdout())?;

    Ok(connection)
}
