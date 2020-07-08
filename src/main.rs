use clap::{App, Arg};
use serde::Deserialize;
use slog::{info, o, Drain, Logger};
use snafu::ResultExt;
use std::collections::HashMap;
use std::net::ToSocketAddrs;
use warp::{self, http, Filter};

use besp::api::gql;
use besp::error;

#[derive(Debug, Deserialize)]
pub struct Env {
    pub env: String,
    pub url: String,
}

#[tokio::main]
async fn main() -> Result<(), error::Error> {
    let matches = App::new("Microservice for probing bragi's elasticsearch")
        .version("0.1")
        .author("Matthieu Paindavoine")
        .arg(
            Arg::with_name("address")
                .value_name("HOST")
                .short("h")
                .long("host")
                .default_value("localhost")
                .help("Address serving this server"),
        )
        .arg(
            Arg::with_name("port")
                .value_name("PORT")
                .short("p")
                .long("port")
                .default_value("8080")
                .help("Port"),
        )
        .get_matches();

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let logger = slog::Logger::root(drain, o!());

    let addr = matches
        .value_of("address")
        .ok_or_else(|| error::Error::MiscError {
            msg: String::from("Could not get address"),
        })?;

    let port = matches
        .value_of("port")
        .ok_or_else(|| error::Error::MiscError {
            msg: String::from("Could not get port"),
        })?;

    let port = port.parse::<u16>().map_err(|err| error::Error::MiscError {
        msg: format!("Could not parse into a valid port number ({})", err),
    })?;

    // XXXX TODO Move this to tokio fs
    let envs = tokio::fs::read_to_string("env.json")
        .await
        .context(error::IOError {
            msg: String::from("Could not open env.json"),
        })?;
    let envs: Vec<Env> = serde_json::from_str(&envs).context(error::JSONError {
        msg: String::from("Could not deserialize env.json content"),
    })?;
    let envs: HashMap<String, String> = envs.into_iter().map(|e| (e.env, e.url)).collect();

    run_server((addr, port), logger, envs).await?;

    Ok(())
}

async fn run_server(
    addr: impl ToSocketAddrs,
    logger: Logger,
    envs: HashMap<String, String>,
) -> Result<(), error::Error> {
    let logger1 = logger.clone();
    let envs1 = envs.clone();
    let state = warp::any().map(move || gql::Context {
        logger: logger1.clone(),
        envs: envs1.clone(),
    });

    let playground = warp::get()
        .and(warp::path("playground"))
        .and(playground_filter("/graphql", Some("/subscriptions")));

    let graphql_filter = juniper_warp::make_graphql_filter(gql::schema(), state.boxed());

    let graphql = warp::path!("graphql").and(graphql_filter);

    let routes = playground.or(graphql);

    let addr = addr
        .to_socket_addrs()
        .context(error::IOError {
            msg: String::from("To Sock Addr"),
        })?
        .next()
        .ok_or(error::Error::MiscError {
            msg: String::from("Cannot resolve addr"),
        })?;

    info!(
        logger.clone(),
        "Serving Bragi Elasticsearch Probe on {}:{}",
        addr.ip(),
        addr.port()
    );
    warp::serve(routes).run(addr).await;

    Ok(())
}

/// Create a filter that replies with an HTML page containing GraphQL Playground. This does not handle routing, so you can mount it on any endpoint.
pub fn playground_filter(
    graphql_endpoint_url: &'static str,
    subscriptions_endpoint_url: Option<&'static str>,
) -> warp::filters::BoxedFilter<(http::Response<Vec<u8>>,)> {
    warp::any()
        .map(move || playground_response(graphql_endpoint_url, subscriptions_endpoint_url))
        .boxed()
}

fn playground_response(
    graphql_endpoint_url: &'static str,
    subscriptions_endpoint_url: Option<&'static str>,
) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .header("content-type", "text/html;charset=utf-8")
        .body(
            juniper::http::playground::playground_source(
                graphql_endpoint_url,
                subscriptions_endpoint_url,
            )
            .into_bytes(),
        )
        .expect("response is valid")
}
