use diesel::prelude::*;
use diesel_migrations::embed_migrations;

embed_migrations!();

pub fn connect(url: &str) -> Result<PgConnection, failure::Error> {
    let connection = PgConnection::establish(url)?;

    // Run migrations.
    embedded_migrations::run_with_output(&connection, &mut std::io::stdout())?;

    Ok(connection)
}
