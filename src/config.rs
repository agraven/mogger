use comrak::ComrakOptions;
use gotham_derive::StateData;

/// Application wide settings defined in configuration file.
#[derive(Deserialize, StateData, Clone)]
pub struct Settings {
    /// Postgres database url
    pub database_url: String,
    /// IP address to bind to
    pub host_address: String,
    /// Cookie settings
    pub cookie: Cookie,
}

impl Settings {
    pub fn from_slice(data: &[u8]) -> Result<Self, toml::de::Error> {
        toml::from_slice(data)
    }
}

/// Cookie related settings
#[derive(Deserialize, Clone)]
pub struct Cookie {
    /// Require HTTPS for cookies
    pub secure: bool,
    /// Restrict cookies to given domain if set
    pub domain: Option<String>,
}

/// Options for markdown formatting using comrak
pub const COMRAK_OPTS: ComrakOptions = ComrakOptions {
    hardbreaks: false,
    smart: false,
    github_pre_lang: true,
    width: 0,
    default_info_string: None,
    unsafe_: false,
    ext_strikethrough: true,
    ext_tagfilter: false,
    ext_table: true,
    ext_autolink: true,
    ext_tasklist: false,
    ext_superscript: false,
    ext_header_ids: None,
    ext_footnotes: true,
    ext_description_lists: false,
};

pub const COMRAK_ARTICLE_OPTS: ComrakOptions = ComrakOptions {
    unsafe_: true,
    ..COMRAK_OPTS
};
