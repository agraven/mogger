//! A simple blogging engine.

#![allow(clippy::new_without_default)]

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate serde;

pub mod article;
pub mod comment;
pub mod config;
pub mod date_format;
pub mod db;
pub mod document;
pub mod handler;
pub mod schema;
pub mod user;

use gotham::{
    hyper::{Body, Response, StatusCode},
    middleware::cookie::CookieParser,
    middleware::state::StateMiddleware,
    pipeline::new_pipeline,
    pipeline::single_pipeline,
    router::builder::{build_router, DefineSingleRoute, DrawRoutes},
    router::response::ResponseExtender,
    router::Router,
    state::State,
};

use std::{borrow::Cow, path::Path};

use crate::{config::Settings, db::DbConnection, user::SessionMiddleware};

/// Response extender for 404 errors
pub struct NotFound;

impl ResponseExtender<Body> for NotFound {
    fn extend(&self, _state: &mut State, res: &mut Response<Body>) {
        let body = res.body_mut();
        *body = "404 File not found".into();
    }
}

/// Builds the request router
fn router(settings: Settings) -> Router {
    // The directory static assets are served from. Is:
    // STATIC_DIR environment varible if defined, otherwise
    // STATIC_DIR compile-time environment variable if defined, otherwise
    // local directory 'static'
    let assets_dir: Cow<str> = if Path::new("/usr/share/mogger").is_dir() {
        "/usr/share/mogger".into()
    } else if let Some(compile_env) = option_env!("STATIC_DIR") {
        compile_env.into()
    } else {
        "static".into()
    };

    // Set up shared state
    let connection = DbConnection::from_url(&settings.database_url);
    let state_mw = StateMiddleware::new(connection);
    let settings_mw = StateMiddleware::new(settings);
    // Build pipeline
    let (chain, pipelines) = single_pipeline(
        new_pipeline()
            .add(state_mw)
            .add(settings_mw)
            .add(CookieParser)
            .add(SessionMiddleware)
            .build(),
    );

    build_router(chain, pipelines, |route| {
        use crate::handler::{articles, users};
        route.get("/").to(handler!(document::index::index));
        route
            .get("/page/:page")
            .with_path_extractor::<document::index::Page>()
            .to(handler!(document::index::index));

        route
            .get("/initial-setup")
            .to(handler!(document::index::init_setup));
        route
            .post("/initial-setup")
            .to(body_handler!(document::index::init_setup_post));

        route.get("/about").to(handler!(document::index::about));

        route
            .get("/article/:id")
            .with_path_extractor::<articles::ArticlePath>()
            .to(handler!(document::article::view));

        route
            .get("/user/:user")
            .with_path_extractor::<users::UserPath>()
            .to(handler!(document::user::view));
        route
            .get("/user/:user/edit")
            .with_path_extractor::<users::UserPath>()
            .to(handler!(document::user::edit));
        route
            .post("/user/:user/profile")
            .with_path_extractor::<users::UserPath>()
            .to(body_handler!(document::user::profile_post));
        route
            .post("/user/:user/password")
            .with_path_extractor::<users::UserPath>()
            .to(body_handler!(document::user::password_post));
        route
            .post("/user/:user/delete")
            .with_path_extractor::<users::UserPath>()
            .to(body_handler!(document::user::delete_post));

        route.get("/login").to(handler!(document::user::login));
        route
            .post("/login")
            .to(body_handler!(document::user::login_post));

        route.get("/logout").to(handler!(document::user::logout));

        route.get("/signup").to(handler!(document::user::signup));
        route
            .post("/signup")
            .to(body_handler!(document::user::signup_post));

        route.get("/edit").to(handler!(document::article::edit));
        route
            .post("/edit")
            .to(body_handler!(document::article::edit_post));
        route
            .get("/edit/:id")
            .with_path_extractor::<articles::ArticleIdPath>()
            .to(handler!(document::article::edit));
        route
            .post("/edit/:id")
            .with_path_extractor::<articles::ArticleIdPath>()
            .to(body_handler!(document::article::edit_post));

        route.scope("/api", |route| {
            route.scope("/articles", |route| {
                route.get("/list").to(handler!(articles::list));
                route
                    .get("/view/:id")
                    .with_path_extractor::<articles::ArticlePath>()
                    .to(handler!(articles::view));
                route.post("/submit").to(body_handler!(articles::submit));
                route
                    .post("/edit/:id")
                    .with_path_extractor::<articles::ArticlePath>()
                    .to(body_handler!(articles::edit));
            });

            route.scope("/comments", |route| {
                use crate::handler::comments;

                route
                    .get("/list/:id")
                    .with_path_extractor::<articles::ArticlePath>()
                    .to(handler!(comments::list));

                route
                    .get("/view/:id")
                    .with_path_extractor::<comments::CommentPath>()
                    .with_query_string_extractor::<comments::Context>()
                    .to(handler!(comments::view));

                route
                    .get("/single/:id")
                    .with_path_extractor::<comments::CommentPath>()
                    .to(handler!(comments::single));

                route
                    .get("/render-content/:id")
                    .with_path_extractor::<comments::CommentPath>()
                    .to(handler!(comments::render_content));
                route
                    .get("/render/:id")
                    .with_path_extractor::<comments::CommentPath>()
                    .to(handler!(comments::render));

                route.post("/submit").to(body_handler!(comments::submit));

                route
                    .post("/edit/:id")
                    .with_path_extractor::<comments::CommentPath>()
                    .to(body_handler!(comments::edit));

                route
                    .get("/delete/:id")
                    .with_path_extractor::<comments::CommentPath>()
                    .to(handler!(comments::delete));

                route
                    .get("/restore/:id")
                    .with_path_extractor::<comments::CommentPath>()
                    .to(handler!(comments::restore));

                route
                    .get("/purge/:id")
                    .with_path_extractor::<comments::CommentPath>()
                    .to(handler!(comments::purge))
            });

            route.scope("/users", |route| {
                route.post("/create").to(body_handler!(users::create));
                route.post("/login").to(body_handler!(users::login));
            });
        });

        route.get("/file/*").to_dir(&*assets_dir);

        route.get("/feed.rss").to(handler!(handler::rss::rss));

        // Error responders
        route.add_response_extender(StatusCode::NOT_FOUND, NotFound);
    })
}

fn main() -> Result<(), failure::Error> {
    // Read settings
    let path = if Path::new("/etc/mogger/mogger.toml").is_file() {
        Path::new("/etc/mogger/mogger.toml")
    } else {
        Path::new("mogger.toml")
    };
    let data = std::fs::read(path)?;
    let settings = Settings::from_slice(&data)?;
    let address = settings.host_address.clone();

    println!("Running at {}", &address);
    gotham::start(address, router(settings))?;
    Ok(())
}
