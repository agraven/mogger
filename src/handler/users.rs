use gotham::{
    helpers::http::response::{create_empty_response, create_response},
    state::{FromState, State},
};
use hyper::{Body, Response, StatusCode};
use mime::APPLICATION_JSON as JSON;

use crate::{
    user::{self, Login, NewUser},
    DbConnection,
};

pub fn create(state: &State, post: Vec<u8>) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(state).lock()?;

    let user: NewUser = serde_json::from_slice(&post)?;

    user::create(connection, user)?;
    Ok(create_empty_response(state, StatusCode::OK))
}

pub fn login(state: &State, post: Vec<u8>) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(state).lock()?;

    let login: Login = serde_json::from_slice(&post)?;
    let response = match login.login(&connection)? {
        Some(session) => {
            // Create response
            create_response(
                state,
                StatusCode::OK,
                JSON,
                serde_json::to_string(&session)?,
            )
        }
        None => create_empty_response(state, StatusCode::FORBIDDEN),
    };
    Ok(response)
}
