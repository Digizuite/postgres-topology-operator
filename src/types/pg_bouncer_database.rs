use kube::{CustomResource};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::types::{HasPgBouncerReference, PgBouncerReference};


#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[kube(
    group = "postgres.digizuite.com",
    version = "v1alpha1",
    kind = "PgBouncerDatabase",
    plural = "pgbouncerdatabases",
    derive = "PartialEq",
    status = "PgBouncerDatabaseStatus",
    printcolumn = r#"{"name":"Database", "type":"string", "description":"Name of the database", "jsonPath":".databaseName"}"#,
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct PgBouncerDatabaseSpec {
    pub exposed_database_name: String,
    pub internal_database_name: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub pg_bouncer: PgBouncerReference,
}


impl HasPgBouncerReference for PgBouncerDatabase {
    fn get_pg_bouncer_reference(&self) -> Option<&PgBouncerReference> {
        Some(&self.spec.pg_bouncer)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct PgBouncerDatabaseStatus {
    pub ready: bool,
}