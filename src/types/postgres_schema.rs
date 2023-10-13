use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::types::{HasPostgresAdminConnection, PostgresAdminConnectionReference, PostgresRoleReference};


#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[kube(
    group = "postgres.digizuite.com",
    version = "v1alpha1",
    kind = "PostgresSchema",
    plural = "postgresschemas",
    derive = "PartialEq",
    status = "PostgresSchemaStatus",
    printcolumn = r#"{"name":"Schema", "type":"string", "description":"Name of the schema", "jsonPath":".schema"}"#,
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct PostgresSchemaSpec {
    pub schema: String,
    pub schema_owner: Option<PostgresSchemaOwner>,
    pub connection: PostgresAdminConnectionReference,
}

impl HasPostgresAdminConnection for PostgresSchema {
    fn get_connection(&self) -> &PostgresAdminConnectionReference {
        &self.spec.connection
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct PostgresSchemaStatus {

}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PostgresSchemaOwner {
    ManagedRole(PostgresRoleReference),
    Name(String),
}