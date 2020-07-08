use juniper::{EmptyMutation, EmptySubscription, FieldResult, IntoFieldError, RootNode};
use slog::Logger;
use std::collections::HashMap;

use super::environment;

#[derive(Debug, Clone)]
pub struct Context {
    pub logger: Logger,
    pub envs: HashMap<String, String>,
}

impl juniper::Context for Context {}

pub struct Query;

#[juniper::graphql_object(
    Context = Context
)]
impl Query {
    /// Return a list of all environments
    async fn environments(
        &self,
        context: &Context,
    ) -> FieldResult<environment::MultiEnvironmentsResponseBody> {
        environment::list_environments(context)
            .await
            .map_err(IntoFieldError::into_field_error)
    }
}

type Schema = RootNode<'static, Query, EmptyMutation<Context>, EmptySubscription<Context>>;

pub fn schema() -> Schema {
    Schema::new(Query, EmptyMutation::new(), EmptySubscription::new())
}
