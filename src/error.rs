use juniper::{graphql_value, FieldError, IntoFieldError};
use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Could not identify environment {}", env))]
    #[snafu(visibility(pub))]
    Environment { env: String },

    #[snafu(display("URL {} not accessible", url))]
    #[snafu(visibility(pub))]
    NotAccessible { url: String, source: reqwest::Error },

    #[snafu(display("Status {} not accessible", url))]
    #[snafu(visibility(pub))]
    StatusNotAccessible { url: String, source: reqwest::Error },

    // FIXME Not sure how to specify the source type here,
    // it's a serde deserialization error, but it requires a lifetime...
    #[snafu(display("JSON Status not readable {}", url))]
    #[snafu(visibility(pub))]
    StatusNotReadable { url: String, source: reqwest::Error },

    #[snafu(display("elasticsearch url not parsable {}", url))]
    #[snafu(visibility(pub))]
    ElasticsearchURLNotReadable {
        url: String,
        source: url::ParseError,
    },

    #[snafu(display("deserialize"))]
    #[snafu(visibility(pub))]
    DeserializeError { source: serde_json::error::Error },

    #[snafu(display("lack of imagination: {}", msg))]
    #[snafu(visibility(pub))]
    MiscError { msg: String },

    #[snafu(display("IO Error: {}", msg))]
    #[snafu(visibility(pub))]
    IOError { msg: String, source: std::io::Error },

    #[snafu(display("JSON Error: {} - {}", msg, source))]
    #[snafu(visibility(pub))]
    JSONError {
        msg: String,
        source: serde_json::Error,
    },
}

impl IntoFieldError for Error {
    fn into_field_error(self) -> FieldError {
        match self {
            err @ Error::Environment { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "Environment Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }

            err @ Error::NotAccessible { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new("Access Error", graphql_value!({ "internal_error": errmsg }))
            }

            err @ Error::StatusNotAccessible { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "Status Not Accessible Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }

            err @ Error::StatusNotReadable { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "Status Not Readable Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }

            err @ Error::ElasticsearchURLNotReadable { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "URL Not Readable Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }

            err @ Error::DeserializeError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "Deserialization Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }

            err @ Error::MiscError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new(
                    "Miscellaneous Error",
                    graphql_value!({ "internal_error": errmsg }),
                )
            }

            err @ Error::IOError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new("IO Error", graphql_value!({ "internal_error": errmsg }))
            }

            err @ Error::JSONError { .. } => {
                let errmsg = format!("{}", err);
                FieldError::new("JSON Error", graphql_value!({ "internal_error": errmsg }))
            }
        }
    }
}
