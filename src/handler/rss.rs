//! Handler for serving an rss feed
use chrono::{DateTime, NaiveDateTime, Utc};
use gotham::{
    helpers::http::response::create_response,
    hyper::{Body, Response, StatusCode},
    state::{FromState, State},
    mime,
};
use rss::{ChannelBuilder, Item, ItemBuilder};

use crate::{article, article::Article, DbConnection};

impl From<Article> for Item {
    fn from(a: Article) -> Self {
        let guid = rss::GuidBuilder::default()
            .value(format!("https://amandag.net/article/view/{}", a.id))
            .permalink(true)
            .build()
            .unwrap();

        ItemBuilder::default()
            .title(a.title)
            .link(format!("https://amandag.net/article/view/{}", a.url))
            .guid(guid)
            .pub_date(date_format(a.date))
            .content(a.content)
            .build()
            .unwrap()
    }
}

/// Encodes a date in [RFC822](https://www.w3.org/Protocols/rfc822/#z28) format.
fn date_format(date: NaiveDateTime) -> String {
    DateTime::<Utc>::from_utc(date, Utc)
        .format("%a, %d %b %Y %H:%M:%S %z")
        .to_string()
}

/// Serves an RSS encoded feed of articles
pub fn rss(state: &State) -> Result<Response<Body>, failure::Error> {
    let connection = &DbConnection::borrow_from(state).lock()?;

    let articles = article::list(connection)?;
    let last_change = articles.get(0).map(|art| date_format(art.date));
    let items: Vec<Item> = articles.into_iter().map(Into::into).collect();

    let mut buf = Vec::new();
    let channel = ChannelBuilder::default()
        .title(env!("CARGO_PKG_NAME"))
        .link(env!("CARGO_PKG_HOMEPAGE"))
        .description(env!("CARGO_PKG_DESCRIPTION"))
        .last_build_date(last_change.clone())
        .pub_date(last_change)
        .items(items)
        .build()
        .unwrap();
    channel.pretty_write_to(&mut buf, b' ', 4)?;

    let media_type: mime::Mime = "application/rss+xml".parse().unwrap();
    Ok(create_response(state, StatusCode::OK, media_type, buf))
}
