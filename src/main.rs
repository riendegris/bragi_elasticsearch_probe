use chrono::prelude::*;
use clap::{App, Arg};
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use std::collections::HashMap;
use std::fs;
use url::Url;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Could not identify environment {}", env))]
    Environment { env: String },

    #[snafu(display("Could not access url {}", url))]
    NotAccessible { url: String, source: reqwest::Error },

    #[snafu(display("Could not access url {}", url))]
    StatusNotAccessible { url: String, source: reqwest::Error },

    // FIXME Not sure how to specify the source type here,
    // it's a serde deserialization error, but it requires a lifetime...
    #[snafu(display("JSON Status not readable {}", url))]
    StatusNotReadable { url: String, source: reqwest::Error },

    #[snafu(display("elasticsearch url not parsable {}", url))]
    ElasticsearchURLNotReadable {
        url: String,
        source: url::ParseError,
    },

    #[snafu(display("deserialize"))]
    DeserializeError { source: serde_json::error::Error },

    #[snafu(display("lack of imagination: {}", msg))]
    MiscError { msg: String },

    #[snafu(display("IO Error: {}", msg))]
    IOError { msg: String, source: std::io::Error },

    #[snafu(display("JSON Error: {} - {}", msg, source))]
    JSONError {
        msg: String,
        source: serde_json::Error,
    },
}

#[derive(Debug, Deserialize)]
pub struct Env {
    pub env: String,
    pub url: String,
}

// This is used for POIs, to indicate if its a private or public source of POI.
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum PrivateStatus {
    Private,
    Public,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ServerStatus {
    Available,
    NotAvailable,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum BragiStatus {
    Available,
    BragiNotAvailable,
    ElasticsearchNotAvailable,
}

#[derive(Debug, Serialize)]
pub struct BragiInfo {
    pub label: String,
    pub url: String,
    pub version: String,
    pub status: BragiStatus,
    pub updated_at: DateTime<Utc>,
    pub elastic: Option<ElasticsearchInfo>,
}

// This struct is used to return the call to 'bragi/status'
// Its information will be inserted in the BragiStatus
#[derive(Debug, Deserialize)]
pub struct BragiStatusDetails {
    pub version: String,
    #[serde(rename = "es")]
    pub elasticsearch: String,
    pub status: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ElasticsearchInfo {
    pub label: String,
    pub url: String,
    pub name: String,
    pub status: ServerStatus,
    pub version: String,
    pub indices: Vec<ElasticsearchIndexInfo>,
    pub index_prefix: String, // eg munin
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ElasticsearchIndexInfo {
    pub label: String,
    pub place_type: String,
    pub coverage: String,
    #[serde(skip_serializing_if = "is_public")]
    pub private: PrivateStatus,
    pub date: DateTime<Utc>,
    pub count: u32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ElasticsearchIndexInfoDetails {
    pub health: String,
    pub status: String,
    pub index: String,
    #[serde(skip)]
    pub prim: u32,
    #[serde(skip)]
    pub rep: u32,
    #[serde(rename = "docs.count")]
    pub count: String,
    #[serde(rename = "docs.deleted", skip)]
    pub deleted: String,
    #[serde(rename = "store.size", skip)]
    pub size: String,
    #[serde(rename = "pri.store.size", skip)]
    pub pri_size: String,
}

fn is_public(status: &PrivateStatus) -> bool {
    status == &PrivateStatus::Public
}

fn main() -> Result<(), Error> {
    // We read env.json a first time to get a list of available
    // environments, which will be used in the help message
    let envs = fs::read_to_string("env.json").context(IOError {
        msg: "Could not open env.json",
    })?;
    let envs: Vec<Env> = serde_json::from_str(&envs).context(JSONError {
        msg: "Could not deserialize env.json content",
    })?;
    let envs: Vec<String> = envs.into_iter().map(|e| e.env).collect();
    let envs = envs.join(", ");
    let matches = App::new("Elasticsearch Discovery")
        .version("0.2")
        .author("Matthieu Paindavoine")
        .about("Provide a list of indexes available in a given environment")
        .arg(
            Arg::with_name("environment")
                .help(&format!("target environment (one of '{}')", envs))
                .required(true),
        )
        .get_matches();
    let env = matches.value_of("environment").ok_or(Error::Environment {
        env: String::from("You did not provide an environment"),
    })?;
    if !envs.contains(env) {
        return Err(Error::Environment {
            env: format!("{} is not a known environment. Use one of {}", env, envs),
        });
    }
    let bragi = run(&env).unwrap_or(BragiInfo {
        label: String::from(env),
        url: String::from(""),
        version: String::from(""),
        status: BragiStatus::BragiNotAvailable,
        updated_at: Utc::now(),
        elastic: None,
    });
    let b = serde_json::to_string(&bragi).unwrap();
    println!("{}", b);
    Ok(())
}

fn run(env: &str) -> Result<BragiInfo, Error> {
    get_url(env)
        .and_then(|(env, url)| check_accessible(env, url))
        .and_then(|(env, url)| check_bragi_status(env, url))
        .and_then(|bragi| update_elasticsearch_indices(bragi))
}

// Return a pair (environment, url)
fn get_url(env: &str) -> Result<(String, String), Error> {
    let envs = fs::read_to_string("env.json").context(IOError {
        msg: "Could not open envs.json",
    })?;
    let envs: Vec<Env> = serde_json::from_str(&envs).context(JSONError {
        msg: "Could not deserialize env.json content",
    })?;
    let info: HashMap<String, String> = envs.into_iter().map(|e| (e.env, e.url)).collect();

    info.get(env)
        .ok_or(Error::Environment {
            env: String::from(env),
        })
        .map(|s| (String::from(env), s.clone()))
}

// Check that the url is accessible (should be done with some kind of 'ping')
// and return its arguments
fn check_accessible(env: String, url: String) -> Result<(String, String), Error> {
    match reqwest::blocking::get(&url) {
        Ok(_) => Ok((env, url)),
        Err(err) => Err(Error::NotAccessible { url, source: err }),
    }
}

fn check_bragi_status(env: String, url: String) -> Result<BragiInfo, Error> {
    let status_url = format!("{}/status", url);
    let resp =
        reqwest::blocking::get(&status_url).context(StatusNotAccessible { url: url.clone() })?;
    let status: BragiStatusDetails = resp
        .json()
        .context(StatusNotReadable { url: url.clone() })?;
    let elastic = Url::parse(&status.elasticsearch).context(ElasticsearchURLNotReadable {
        url: String::from(status.elasticsearch),
    })?;

    let elastic_url = match elastic.port() {
        None => format!("{}://{}", elastic.scheme(), elastic.host_str().unwrap()),
        Some(port) => format!(
            "{}://{}:{}",
            elastic.scheme(),
            elastic.host_str().unwrap(),
            port
        ),
    };

    let prefix = String::from(elastic.path_segments().unwrap().next().unwrap_or("munin"));

    // We return a bragi info with empty elastic search indices... We delegate filling
    // this information to a later stage.
    Ok(BragiInfo {
        label: format!("bragi_{}", env),
        url: url,
        version: status.version,
        status: BragiStatus::Available,
        elastic: Some(ElasticsearchInfo {
            label: format!("elasticsearch_{}", env),
            url: elastic_url,
            name: String::from(""),
            status: ServerStatus::NotAvailable,
            version: String::from(""),
            indices: Vec::new(),
            index_prefix: prefix,
            updated_at: Utc::now(),
        }),
        updated_at: Utc::now(),
    })
}

// We retrieve all indices in json format, then use serde to deserialize into a data structure,
// and finally parse the label to extract the information.
fn update_elasticsearch_indices(info: BragiInfo) -> Result<BragiInfo, Error> {
    info.elastic
        .clone()
        .ok_or(Error::MiscError {
            msg: String::from("hello"),
        })
        .map(|es_info| {
            let indices_url = format!("{}/_cat/indices?format=json", es_info.url);
            let indices: Option<Vec<ElasticsearchIndexInfo>> = reqwest::blocking::get(&indices_url)
                .ok()
                .and_then(|resp| resp.json().ok())
                .map(|is: Vec<ElasticsearchIndexInfoDetails>| {
                    is.iter()
                        .map(|i| {
                            let zs: Vec<&str> = i.index.split('_').collect();
                            let (private, coverage) = if zs[2].starts_with("priv.") {
                                (PrivateStatus::Private, zs[2].chars().skip(5).collect())
                            } else {
                                (PrivateStatus::Public, zs[2].to_string())
                            };
                            ElasticsearchIndexInfo {
                                label: i.index.clone(),
                                place_type: zs[1].to_string(),
                                coverage,
                                private,
                                date: DateTime::<Utc>::from_utc(
                                    NaiveDateTime::new(
                                        NaiveDate::parse_from_str(zs[3], "%Y%m%d")
                                            .unwrap_or(NaiveDate::from_ymd(1970, 1, 1)),
                                        NaiveTime::parse_from_str(zs[4], "%H%M%S")
                                            .unwrap_or(NaiveTime::from_hms(0, 1, 1)),
                                    ),
                                    Utc,
                                ),
                                count: i.count.parse().unwrap_or(0),
                                updated_at: Utc::now(),
                            }
                        })
                        .collect()
                });
            let status = if indices.is_some() {
                ServerStatus::Available
            } else {
                ServerStatus::NotAvailable
            };
            let es_update_info = ElasticsearchInfo {
                status,
                indices: indices.unwrap_or(Vec::new()),
                updated_at: Utc::now(),
                ..es_info
            };
            BragiInfo {
                elastic: Some(es_update_info),
                ..info
            }
        })
}
