use diesel::prelude::*;

pub fn connect() -> PgConnection {
    let url = "postgresql://postgres@localhost/mock_blog";
    PgConnection::establish(url).expect("Error opening database")
}
