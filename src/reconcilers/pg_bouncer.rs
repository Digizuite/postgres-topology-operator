use std::collections::BTreeMap;
use std::sync::Arc;
use itertools::Itertools;
use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec, DeploymentStrategy, RollingUpdateDeployment};
use k8s_openapi::api::core::v1::{ConfigMap, ConfigMapVolumeSource, Container, PodSpec, PodTemplateSpec, Service, ServicePort, ServiceSpec, Volume, VolumeMount};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::{Api, Resource, ResourceExt};
use kube::api::{ListParams, Patch, PatchParams};
use kube_runtime::controller::Action;
use sha2::{Digest};
use crate::{ContextData, Error};
use crate::helpers::ini_builder;
use crate::types::{HasPgBouncerReference, PgBouncer, PgBouncerDatabase, PgBouncerDatabaseSpec, PgBouncerSpec, PgBouncerUser, PgBouncerUserSpec};

const PG_BOUNCER_INI_FILE_NAME: &str = "pgbouncer.ini";
const USERLIST_TXT_FILE_NAME: &str = "userlist.txt";
const USERLIST_HASH: &str = "userlisthash";
const PG_BOUNCER_APP_NAME: &str = "pgbouncer";

const PG_BOUNCER_PORT: i32 = 5432;

pub async fn reconcile_pg_bouncer(resource: Arc<PgBouncer>, context: Arc<ContextData>) -> anyhow::Result<Action, Error> {
    run_reconciler(resource, context).await.map_err(|e| e.into())
}

async fn run_reconciler(resource: Arc<PgBouncer>, context: Arc<ContextData>) -> anyhow::Result<Action> {
    info!("Reconciling pg_bouncer {:?}", resource.metadata.name);


    if resource.metadata.deletion_timestamp.is_some() {
        info!("pg_bouncer {:?} is being deleted, skipping", resource.metadata.name);
        return Ok(Action::await_change());
    }

    let namespace = resource.namespace().expect("Expected pg_bouncer to be namespaced");
    let resource_name = resource.name_any();





    let databases = Api::<PgBouncerDatabase>::all(context.kubernetes_client.clone())
        .list(&ListParams::default())
        .await?;
    let databases = databases
        .iter()
        .filter(|db| db.is_for(&resource))
        .map(|db| &db.spec);

    let users = Api::<PgBouncerUser>::all(context.kubernetes_client.clone())
        .list(&ListParams::default())
        .await?;
    let users = users
        .iter()
        .filter(|u| u.is_for(&resource))
        .map(|u| &u.spec);


    let pg_bouncer_ini = create_pg_bouncer_ini(&resource.spec, databases);
    let (user_list_txt, user_list_hash) = create_user_list(users);


    let config_map_api: Api<ConfigMap> = Api::namespaced(context.kubernetes_client.clone(), &namespace);
    let deployments_api: Api<Deployment> = Api::namespaced(context.kubernetes_client.clone(), &namespace);


    let config_map_name = format!("{}-config", resource_name);


    let serverside = PatchParams::apply("postgres-topology-operator").force();


    let desired_config_map = ConfigMap {
        metadata: ObjectMeta {
            namespace: Some(namespace.clone()),
            name: Some(config_map_name.clone()),
            owner_references: Some(vec![resource.controller_owner_ref(&()).unwrap()]),
            ..Default::default()
        },
        data: Some([
            (PG_BOUNCER_INI_FILE_NAME.to_string(), pg_bouncer_ini.clone()),
            (USERLIST_TXT_FILE_NAME.to_string(), user_list_txt.clone()),
            (USERLIST_HASH.to_string(), user_list_hash.clone()),
        ].into()),
        ..Default::default()
    };


    let config_map = if let Some(config_map) = config_map_api.get_opt(&config_map_name).await? {

        let must_update_config_map = if let Some(existing) = &config_map.data {
            if existing.get(PG_BOUNCER_INI_FILE_NAME) != Some(&pg_bouncer_ini) {
                info!("pg_bouncer.ini has changed, updating config map");
                true
            } else if existing.get(USERLIST_HASH) != Some(&user_list_hash) {
                info!("userlist.txt has changed, updating config map");
                true
            } else {
                false
            }
        } else {
            true
        };

        if must_update_config_map {
            let patch = Patch::Apply(&desired_config_map);
            let config_map = config_map_api.patch(&config_map_name, &serverside, &patch).await?;

            info!("Updated config map {:?}", config_map.metadata.name);

            config_map
        } else {
            info!("Config map does not need to be updated");
            config_map
        }
    } else {
        let patch = Patch::Apply(&desired_config_map);
        let config_map = config_map_api.patch(&config_map_name, &serverside, &patch).await?;

        info!("Created config map {:?}", config_map.metadata.name);

        config_map
    };



    let deployment_name = format!("{}-deployment", resource_name);
    let owner_label = resource.metadata.uid.as_ref().expect("Expected resource to have a UID").to_string();

    let deployment_labels: BTreeMap<String, String> = [
        ("app".to_string(), PG_BOUNCER_APP_NAME.to_string()),
        ("postgres-topology-operator/pg_bouncer".to_string(), owner_label.clone()),
    ].into();

    let deployment = Deployment {
        metadata: ObjectMeta {
            namespace: Some(namespace.clone()),
            name: Some(deployment_name.clone()),
            owner_references: Some(vec![resource.controller_owner_ref(&()).unwrap()]),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            selector: LabelSelector {
                match_labels: Some(deployment_labels.clone()),
                ..Default::default()
            },
            strategy: Some(DeploymentStrategy {
                rolling_update: Some(RollingUpdateDeployment {
                    max_unavailable: Some(IntOrString::Int(0)),
                    max_surge: Some(IntOrString::Int(1)),
                }),
                ..Default::default()
            }),
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(deployment_labels.clone()),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    volumes: Some(vec![
                        Volume {
                            name: "config".to_string(),
                            config_map: Some(ConfigMapVolumeSource {
                                name: Some(config_map.name_any()),
                                optional: Some(false),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }
                    ]),
                    containers: vec![
                        Container {
                            name: "pg-bouncer".to_string(),
                            image: Some("ghcr.io/digizuite/digi-pg-bouncer:task-DEPLOY-22".to_string()),
                            image_pull_policy: Some("Always".to_string()),
                            volume_mounts: Some(vec![
                                VolumeMount {
                                    mount_path: "/etc/pgbouncer".to_string(),
                                    name: "config".to_string(),
                                    read_only: Some(true),
                                    ..Default::default()
                                }
                            ]),
                            resources: resource.spec.pod_options.as_ref().and_then(|o| o.resources.clone()),
                            ..Default::default()
                        }
                    ],
                    node_selector: resource.spec.pod_options.as_ref().and_then(|o| o.node_selector.clone()),
                    ..Default::default()
                }),
            },
            ..Default::default()
        }),
        ..Default::default()
    };

    deployments_api.patch(&deployment_name, &serverside, &Patch::Apply(deployment)).await?;

    info!("Deployment created");

    let service = Service {
        metadata: ObjectMeta {
            namespace: Some(namespace.clone()),
            name: Some(resource.spec.service.name.clone()),
            owner_references: Some(vec![resource.controller_owner_ref(&()).unwrap()]),
            annotations: resource.spec.service.annotations.clone(),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            selector: Some(deployment_labels.clone()),
            ports: Some(vec![
                ServicePort {
                    port: resource.spec.service.port.unwrap_or(PG_BOUNCER_PORT),
                    target_port: Some(IntOrString::Int(PG_BOUNCER_PORT)),
                    ..Default::default()
                }
            ]),
            ..Default::default()
        }),
        ..Default::default()
    };

    let service_api: Api<Service> = Api::namespaced(context.kubernetes_client.clone(), &namespace);

    service_api.patch(&resource.spec.service.name, &serverside, &Patch::Apply(service)).await?;

    info!("Service created/updated");

    Ok(Action::await_change())
}

fn create_pg_bouncer_ini<'a>(spec: &PgBouncerSpec, databases: impl Iterator<Item=&'a PgBouncerDatabaseSpec>) -> String {
    let mut builder = ini_builder::new();

    builder.add_section("pgbouncer");

    let settings = &spec.pg_bouncer;

    builder.add_setting("pool_mode", &settings.pool_mode);
    builder.add_setting("listen_port", PG_BOUNCER_PORT);
    builder.add_setting("listen_addr", "0.0.0.0");
    builder.add_setting("auth_type", &settings.auth_type);
    if let Some(admin_users) = &settings.admin_users {
        builder.add_comma_separated("admin_users", admin_users);
    }
    if let Some(ignore_startup_parameters) = &settings.ignore_startup_parameters {
        builder.add_comma_separated("ignore_startup_parameters", ignore_startup_parameters);
    }
    builder.add_setting("server_tls_sslmode", &settings.server_tls_ssl_mode);
    builder.add_setting("client_tls_sslmode", &settings.client_tls_ssl_mode);
    builder.add_setting("max_client_conn", settings.max_client_conn);
    builder.add_setting("max_db_connections", settings.max_db_connections);
    builder.add_setting("auth_file", format!("/etc/pgbouncer/{}", USERLIST_TXT_FILE_NAME));


    builder.add_section("databases");
    for db in databases {
        let key = &db.exposed_database_name;
        let mut value = String::new();
        value.push_str(&format!("host={} ", db.host));
        if let Some(port) = &db.port {
            value.push_str(&format!("port={} ", port));
        }
        if let Some(user) = &db.user {
            value.push_str(&format!("user={} ", user));
        }
        if let Some(name) = &db.internal_database_name {
            value.push_str(&format!("dbname={} ", name));
        }
        builder.add_setting(key, value);
    }

    builder.build()
}

fn create_user_list<'a>(users: impl Iterator<Item=&'a PgBouncerUserSpec>) -> (String, String) {
    let users = users.sorted_by_key(|u| &u.username);

    let mut hasher = sha2::Sha256::new();

    let mut s = String::new();

    for user in users {
        debug!("Adding user {}", user.username);
        hasher.update(user.username.as_bytes());
        hasher.update(user.password.get_raw_text().as_bytes());
        let password_text = user.password.get_password_text(&user.username);
        s.push_str(&format!("\"{}\" \"{}\"\n", user.username, password_text))
    }

    let hash = &hasher.finalize()[..];
    (s, base16ct::lower::encode_string(hash))
}