use chrono::prelude::*;
use futures::future::TryFutureExt;
use futures::stream::{self, TryStreamExt};
use juniper::{GraphQLEnum, GraphQLObject};
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::convert::TryFrom;
use url::Url;

use super::gql::Context;
use crate::error;

/// The response body for multiple indexes
#[derive(Debug, Serialize, GraphQLObject)]
#[serde(rename_all = "camelCase")]
pub struct MultiEnvironmentsResponseBody {
    environments: Vec<BragiInfo>,
    environments_count: i32,
}

impl From<Vec<BragiInfo>> for MultiEnvironmentsResponseBody {
    fn from(environments: Vec<BragiInfo>) -> Self {
        let environments_count = i32::try_from(environments.len()).unwrap();
        Self {
            environments,
            environments_count,
        }
    }
}

// This is used for POIs, to indicate if its a private or public source of POI.
#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, GraphQLEnum)]
#[serde(rename_all = "snake_case")]
pub enum PrivateStatus {
    Private,
    Public,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, GraphQLEnum)]
#[serde(rename_all = "snake_case")]
pub enum ServerStatus {
    Available,
    NotAvailable,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone, GraphQLEnum)]
#[serde(rename_all = "snake_case")]
pub enum BragiStatus {
    Available,
    BragiNotAvailable,
    ElasticsearchNotAvailable,
}

#[derive(Debug, Serialize, GraphQLObject)]
pub struct BragiInfo {
    pub label: String,
    pub url: String,
    pub version: String,
    pub status: BragiStatus,
    pub updated_at: DateTime<Utc>,
    pub elastic: Option<ElasticsearchInfo>,
}

impl BragiInfo {
    fn new<S: Into<String>>(label: S, url: S) -> BragiInfo {
        BragiInfo {
            label: label.into(),
            url: url.into(),
            version: String::from(""),
            status: BragiStatus::BragiNotAvailable,
            updated_at: Utc::now(),
            elastic: None,
        }
    }
}

// This struct is used to return the call to 'bragi/status'
// Its information will be inserted in the BragiStatus
#[derive(Debug, Deserialize, GraphQLObject)]
pub struct BragiStatusDetails {
    pub version: String,
    #[serde(rename = "es")]
    pub elasticsearch: String,
    pub status: String,
}

#[derive(Debug, Serialize, Clone, GraphQLObject)]
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

#[derive(Debug, Serialize, Clone, GraphQLObject)]
pub struct ElasticsearchIndexInfo {
    pub label: String,
    pub place_type: String,
    pub coverage: String,
    #[serde(skip_serializing_if = "is_public")]
    pub private: PrivateStatus,
    pub created_at: DateTime<Utc>,
    pub count: i32,
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

pub async fn list_environments(
    context: &Context,
) -> Result<MultiEnvironmentsResponseBody, error::Error> {
    let envs = stream::iter(context.envs.iter().map(|env| Ok(env)))
        .try_fold(Vec::new(), |mut acc, (env, url)| async move {
            let env = probe_environment(env, url, context).await?;
            acc.push(env);
            Ok(acc)
        })
        .await?;
    Ok(envs.into())
}

pub async fn probe_environment<S: Into<String>>(
    env: S,
    url: S,
    _context: &Context,
) -> Result<BragiInfo, error::Error> {
    let env = env.into();
    let url = url.into();
    check_accessible(env.clone(), url.clone())
        .and_then(|(env, url)| check_bragi_status(env, url))
        .and_then(|info| update_elasticsearch_indices(info))
        .or_else(|_err| async { Ok(BragiInfo::new(env, url)) })
        .await
}

// We retrieve all indices in json format, then use serde to deserialize into a data structure,
// and finally parse the label to extract the information.
pub async fn update_elasticsearch_indices(info: BragiInfo) -> Result<BragiInfo, error::Error> {
    let es_info = info.elastic.clone();
    let label = info.label.clone();
    let url = info.label.clone();
    let future = async {
        es_info.ok_or(error::Error::MiscError {
            msg: String::from("hello"),
        })
    };
    future
        .and_then(|es_info| async move { foo(es_info).await })
        .map_ok_or_else(
            |_err| Ok(BragiInfo::new(label, url)),
            |es_info| {
                Ok(BragiInfo {
                    elastic: Some(es_info),
                    ..info
                })
            },
        )
        .await
}

async fn check_bragi_status(env: String, url: String) -> Result<BragiInfo, error::Error> {
    let status_url = format!("{}/status", url);
    let resp = reqwest::get(&status_url)
        .await
        .context(error::StatusNotAccessible { url: url.clone() })?;
    let status: BragiStatusDetails = resp
        .json()
        .await
        .context(error::StatusNotReadable { url: url.clone() })?;
    let elastic =
        Url::parse(&status.elasticsearch).context(error::ElasticsearchURLNotReadable {
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
        url,
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

// Check that the url is accessible (should be done with some kind of 'ping')
// and return its arguments
pub async fn check_accessible(env: String, url: String) -> Result<(String, String), error::Error> {
    match reqwest::get(&url).await {
        Ok(_) => Ok((env, url)),
        Err(err) => Err(error::Error::NotAccessible { url, source: err }),
    }
}

pub async fn foo(es_info: ElasticsearchInfo) -> Result<ElasticsearchInfo, error::Error> {
    let indices_url = format!("{}/_cat/indices?format=json", es_info.url);
    let indices: Option<Vec<ElasticsearchIndexInfo>> = reqwest::get(&indices_url)
        .await
        .context(error::NotAccessible {
            url: indices_url.clone(),
        })?
        .json::<Vec<ElasticsearchIndexInfoDetails>>()
        .await
        .context(error::NotAccessible { url: indices_url })
        .ok()
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
                        created_at: DateTime::<Utc>::from_utc(
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
    Ok(ElasticsearchInfo {
        status,
        indices: indices.unwrap_or(Vec::new()),
        updated_at: Utc::now(),
        ..es_info
    })
}
