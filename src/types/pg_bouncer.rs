use std::collections::BTreeMap;
use std::fmt::{Debug, Display, Formatter};
use k8s_openapi::api::core::v1::ResourceRequirements;
use kube::{CustomResource, ResourceExt};
use kube_runtime::reflector::ObjectRef;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::postgres_password::PostgresPassword;
use crate::types::{PostgresSslMode};


#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[kube(
group = "postgres.digizuite.com",
version = "v1alpha1",
kind = "PgBouncer",
plural = "pgbouncers",
derive = "PartialEq",
status = "PgBouncerStatus",
printcolumn = r#"{"name":"Role", "type":"string", "description":"Name of the role", "jsonPath":".role"}"#,
namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct PgBouncerSpec {
    pub pg_bouncer: PgBouncerSettings,
    pub pod_options: Option<PgBouncerPodOptions>,
    pub service: PgBouncerServiceSettings,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct PgBouncerReference {
    pub name: String,
    pub namespace: Option<String>,
}

impl PgBouncerReference {
    pub fn to_object_ref(&self, current_namespace: &str) -> ObjectRef<PgBouncer> {
        ObjectRef::new(&self.name)
            .within(self.namespace.as_deref().unwrap_or(current_namespace))
    }
}

pub trait HasPgBouncerReference: ResourceExt + Debug {
    fn get_pg_bouncer_object_ref(&self) -> Option<ObjectRef<PgBouncer>> {
        let ns = self.namespace()?;
        let pg_bouncer_ref = self.get_pg_bouncer_reference()?;
        Some(pg_bouncer_ref.to_object_ref(&ns))
    }

    fn get_pg_bouncer_reference(&self) -> Option<&PgBouncerReference>;

    fn is_for(&self, bouncer: &PgBouncer) -> bool {
        let ns = self.namespace().expect("Resource should be namespaced");
        let pg_bouncer_ref = self.get_pg_bouncer_reference();

        if let Some(pg_bouncer_ref) = pg_bouncer_ref {
            pg_bouncer_ref.name == bouncer.name_any() &&
                pg_bouncer_ref.namespace.as_ref().unwrap_or(&ns) == &bouncer.namespace().expect("pg_bouncer resource should always be namespaced")
        } else {
            false
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct PgBouncerServiceSettings {
    pub name: String,
    pub annotations: Option<BTreeMap<String, String>>,
    pub port: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct PgBouncerPodOptions {
    pub node_selector: Option<BTreeMap<String, String>>,
    pub resources: Option<ResourceRequirements>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct PgBouncerSettings {
    pub pool_mode: PgBouncerPoolMode,
    pub auth_type: PgBouncerAuthType,
    pub admin_users: Option<Vec<String>>,
    pub ignore_startup_parameters: Option<Vec<String>>,
    pub server_tls_ssl_mode: PostgresSslMode,
    pub client_tls_ssl_mode: PostgresSslMode,
    pub max_client_conn: u32,
    pub max_db_connections: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PgBouncerAuthUser {
    pub username: String,
    pub password: PostgresPassword,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct PgBouncerStatus {
    pub last_user_config_hash: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum PgBouncerPoolMode {
    #[default]
    Transaction,
    Session,
    Statement,
}

impl Display for PgBouncerPoolMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            PgBouncerPoolMode::Transaction => "transaction",
            PgBouncerPoolMode::Session => "session",
            PgBouncerPoolMode::Statement => "statement",
        };


        f.write_str(s)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PgBouncerAuthType {
    #[default]
    Plain,
    Md5,
    ScramSha256,
}

impl Display for PgBouncerAuthType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            PgBouncerAuthType::Plain => "plain",
            PgBouncerAuthType::Md5 => "md5",
            PgBouncerAuthType::ScramSha256 => "scram-sha-256",
        };


        write!(f, "{}", s)
    }
}
