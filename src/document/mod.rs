//use gotham::handler::IntoResponse;
use gotham::helpers::http::response::create_response;
use gotham::state::State;
use http::StatusCode;
use hyper::{Body, Response};

pub mod index;

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
