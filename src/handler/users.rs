use gotham::{
    helpers::http::response::{create_empty_response, create_response},
    hyper::{Body, Response, StatusCode},
    state::{FromState, State},
};
use gotham_derive::{StateData, StaticResponseExtender};
use mime::APPLICATION_JSON as JSON;

use crate::{
    config::Settings,
    user::{self, Login, NewUser, Session},
    DbConnection,
};

#[derive(Deserialize, StateData, StaticResponseExtender)]
pub struct UserPath {
    pub user: String,
}

pub fn create(state: &State, post: Vec<u8>) -> Result<Response<Body>, failure::Error> {
    let session = Session::try_borrow_from(state);
    if session.is_none() && !Settings::borrow_from(state).features.signups {
        return Err(failure::err_msg("Permission denied"));
    }
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
