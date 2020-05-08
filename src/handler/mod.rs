use askama::Template;
use futures::{future, Future, Stream};
use gotham::{
    handler::{HandlerFuture, IntoHandlerError},
    helpers::http::response::create_response,
    state::{FromState, State},
};
use http::{Response, StatusCode};
use hyper::Body;

use crate::{
    db::{Connection, DbConnection},
    document::TemplateExt,
    user::{Permission, Session},
};

pub mod articles;
pub mod comments;
pub mod index;
pub mod rss;
pub mod users;

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTemplate<'a> {
    session: Option<&'a Session>,
    connection: &'a Connection,
    error: String,
}

/// Creates a `HandlerFuture` that runs the given function
pub fn body_handler<F>(mut state: State, op: F) -> Box<HandlerFuture>
where
    F: FnOnce(&State, Vec<u8>) -> Response<Body> + Send + 'static,
{
    let f = Body::take_from(&mut state)
        .concat2()
        .then(|result| match result {
            Ok(body) => {
                let response = op(&state, body.to_vec());
                future::ok((state, response))
            }
            Err(e) => future::err((state, e.into_handler_error())),
        });

    Box::new(f)
}

pub fn error_response(state: &State, error: impl std::fmt::Display) -> Response<Body> {
    if let Ok(ref connection) = DbConnection::borrow_from(state).lock() {
        let template = ErrorTemplate {
            session: Session::try_borrow_from(state),
            connection,
            error: error.to_string(),
        };
        template.to_response(state)
    } else {
        create_response(
            state,
            StatusCode::INTERNAL_SERVER_ERROR,
            mime::TEXT_PLAIN,
            format!("{}", error),
        )
    }
}

pub fn response(state: &State, result: Result<Response<Body>, failure::Error>) -> Response<Body> {
    match result {
        Ok(response) => response,
        Err(error) => error_response(state, error),
    }
}

#[macro_export]
macro_rules! handler {
    ($handler_fn:path) => {
        |state| {
            let r = crate::handler::response(&state, $handler_fn(&state));
            (state, r)
        }
    };
}

#[macro_export]
macro_rules! body_handler {
    ($handler_fn:path) => {
        |state| {
            crate::handler::body_handler(state, |state, post| {
                crate::handler::response(&state, $handler_fn(state, post))
            })
        }
    };
}
