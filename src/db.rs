use diesel::prelude::*;

pub fn connect() -> PgConnection {
    let url = "postgresql://postgres@localhost/amandag";
    PgConnection::establish(url).expect("Error opening database")
}
