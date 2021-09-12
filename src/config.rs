use comrak::{
    plugins::syntect::SyntectAdapter, ComrakExtensionOptions, ComrakOptions, ComrakParseOptions,
    ComrakPlugins, ComrakRenderOptions, ComrakRenderPlugins,
};
use gotham_derive::StateData;

/// Application wide settings defined in configuration file.
#[derive(Deserialize, StateData, Clone)]
pub struct Settings {
    /// Postgres database url
    pub database_url: String,
    /// IP address to bind to
    pub host_address: String,
    /// Toggles for enabling and disabling features
    pub features: Features,
    /// Cookie settings
    pub cookie: Cookie,
}

impl Settings {
    pub fn from_slice(data: &[u8]) -> Result<Self, toml::de::Error> {
        toml::from_slice(data)
    }
}

/// Feature toggles
#[derive(Deserialize, Clone)]
pub struct Features {
    /// Allow registering an account
    pub signups: bool,
    /// Allow unregistered users to make comments
    pub guest_comments: bool,
}

/// Cookie related settings
#[derive(Deserialize, Clone)]
pub struct Cookie {
    /// Require HTTPS for cookies
    pub secure: bool,
    /// Restrict cookies to given domain if set
    pub domain: Option<String>,
}

/// Options for comment markdown formatting using comrak
pub const COMRAK_OPTS: ComrakOptions = ComrakOptions {
    extension: ComrakExtensionOptions {
        strikethrough: true,
        tagfilter: false,
        table: true,
        autolink: true,
        tasklist: false,
        superscript: false,
        header_ids: None,
        footnotes: true,
        description_lists: false,
        front_matter_delimiter: None,
    },
    parse: ComrakParseOptions {
        smart: false,
        default_info_string: None,
    },
    render: ComrakRenderOptions {
        hardbreaks: false,
        github_pre_lang: true,
        width: 0,
        unsafe_: false,
        escape: true,
    },
};

/// Options for aritcle markdown formatting using comrak
pub const COMRAK_ARTICLE_OPTS: ComrakOptions = ComrakOptions {
    render: ComrakRenderOptions {
        unsafe_: true,
        escape: false,
        ..COMRAK_OPTS.render
    },
    ..COMRAK_OPTS
};

pub fn comrak_syntax_adapter() -> SyntectAdapter<'static> {
    SyntectAdapter::new("base16-ocean.light")
}

pub fn comrak_plugins<'a>(adapter: &'a SyntectAdapter) -> ComrakPlugins<'a> {
    ComrakPlugins {
        render: ComrakRenderPlugins {
            codefence_syntax_highlighter: Some(adapter),
        },
    }
}
