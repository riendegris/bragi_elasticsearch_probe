# Bragi Elasticsearch Probe

Microservice to discover bragi's elasticsearch environments. It returns a list of environments,
each containing all the indexes.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Getting Started

These instructions will get you a copy of the project up and running on your local machine for
development and testing purposes. See deployment for notes on how to deploy the project on a live
system.

### Prerequisites

`besp` is a rust project, so you need to setup a rust environment to develop or compile. See
[Install Rust](https://www.rust-lang.org/tools/install) for instructions.

`besp` also relies on `jq-rs`, which is a rust binding to `libjq`, which needs to be installed.
On a debian stretch system, you can install the following packages: `libjq1`, `libjq-dev`,
`libonig4`, `libonig-dev`. You also need to set the variable `JQ_LIB_DIR` to the directory where
`libjq.so` is installed. For example: `JQ_LIB_DIR=/usr/lib/x86_64-linux-gnu`.

### Installing

This is aightforward rust project, so the standard `cargo` incantation should do the trick:

```
git clone https://github.com/riendegris/bragi_elasticsearch_probe
cd bragi_elasticsearch_probe
cargo build --release
```

This will create a binary in `target/release/server`

It requires (for now, maybe that requirement will change in the future...) a json file containing
the list of environments to scan: `env.json`:

```json
[
  {
    "env": "local",
    "url": "http://localhost:4000"
  },
  {
    "env": "dev",
    "url": "http://dev.acme.org:4000"
  }
  [...]
]
```

Alternatively, you can construct a docker container

```
git clone https://github.com/riendegris/bragi_elasticsearch_probe
cd bragi_elasticsearch_probe
docker build -t besp -f ./docker/Dockerfile .
```

## Running the tests

Lets try the program using docker... Assuming you ran the docker build command above, you
can now run the container using

```
docker run -p 8080:8080 besp:latest
```

This will expose a GraphQL API on port 8080.

The description of the API is in the file schema.graphql

You can test this interface directly in your browser via the playground, or using the command line:

### Playground

The playground is a GraphQL IDE. It is available at `localhost:8080`

### Break down into end to end tests

```
cargo test --release
```

## Deployment

Add additional notes about how to deploy this on a live system

## Built With

These are some of the crates used:

* [juniper](https://docs.rs/juniper/0.14.2/juniper/) - Graphql implementation in rust
* [warp](https://docs.rs/warp/0.2.3/warp/) - Web framework
* [jq-rs](https://docs.rs/jq-rs/0.4.1/jq_rs/) - Using with jq

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct, and the process
for submitting pull requests to us.

## Versioning

We use [SemVer](http://semver.org/) for versioning. For the versions available, see the [tags on
this repository](https://github.com/your/project/tags). 

## Authors

* **Matthieu Paindavoine** - *Initial work* - [riendegris](https://github.com/riendegris)

See also the list of [contributors](https://github.com/riendegris/ctl2mimir/contributors) who
participated in this project.

## License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details

## Acknowledgments

Coming up
