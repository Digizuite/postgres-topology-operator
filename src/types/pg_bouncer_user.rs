use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::postgres_password::PostgresPassword;
use crate::types::{HasPgBouncerReference, PgBouncerReference};


#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[kube(
    group = "postgres.digizuite.com",
    version = "v1alpha1",
    kind = "PgBouncerUser",
    plural = "pgbouncerusers",
    derive = "PartialEq",
    status = "PgBouncerUserStatus",
    printcolumn = r#"{"name":"Database", "type":"string", "description":"Name of the database", "jsonPath":".databaseName"}"#,
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct PgBouncerUserSpec {
    pub username: String,
    pub password: PostgresPassword,
    pub pg_bouncer: PgBouncerReference,
}

impl HasPgBouncerReference for PgBouncerUser {
    fn get_pg_bouncer_reference(&self) -> Option<&PgBouncerReference> {
        Some(&self.spec.pg_bouncer)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct PgBouncerUserStatus {
    pub ready: bool,
}