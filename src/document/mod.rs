//! Modules for generating HTML documents.

use gotham::{
    helpers::http::response::create_response,
    hyper::{Body, Response, StatusCode},
    state::State,
    mime,
};

pub mod article;
pub mod index;
pub mod user;

pub type DocumentResult = Result<Response<Body>, failure::Error>;

pub trait TemplateExt {
    fn to_response(&self, state: &State) -> Response<Body>;
}

impl<T: askama::Template> TemplateExt for T {
    fn to_response(&self, state: &State) -> Response<Body> {
        match self.render() {
            Ok(string) => create_response(state, StatusCode::OK, mime::TEXT_HTML, string),
            Err(e) => create_response(
                state,
                StatusCode::INTERNAL_SERVER_ERROR,
                mime::TEXT_PLAIN,
                format!("Template error: {}", e),
            ),
        }
    }
}
