use std::sync::Arc;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::{Api, Resource, ResourceExt};
use kube::api::{Patch, PatchParams};
use kube_runtime::controller::Action;
use crate::{ContextData, Error};
use crate::reconcilers::finalizers::{ensure_finalizer, remove_finalizer};
use crate::reconcilers::helpers::get_postgres_connection;
use crate::types::{PgBouncerUser, PgBouncerUserSpec, PostgresRole};

pub async fn reconcile_postgres_role(resource: Arc<PostgresRole>, context: Arc<ContextData>) -> anyhow::Result<Action, Error> {
    run_reconciler(resource, context).await.map_err(|e| e.into())
}

async fn run_reconciler(resource: Arc<PostgresRole>, context: Arc<ContextData>) -> anyhow::Result<Action> {
    info!("Reconciler postgres role {:?}", resource.metadata.name);


    if resource.metadata.deletion_timestamp.is_some() {
        info!("Deleting postgres role {:?}", resource.metadata.name);

        let pg_connection = get_postgres_connection(resource.as_ref(), context.kubernetes_client.clone()).await?;


        if pg_connection.query_opt("SELECT FROM pg_roles WHERE rolname = $1", &[&resource.spec.role]).await?.is_none() {
            info!("Role {} does not exist", resource.spec.role);
        } else {
            info!("Dropping role {}", resource.spec.role);
            pg_connection.execute(&format!("REVOKE ALL PRIVILEGES ON DATABASE {} FROM {} CASCADE", pg_connection.database, resource.spec.role), &[]).await?;
            pg_connection.execute(&format!("DROP ROLE {}", resource.spec.role), &[]).await?;
            info!("Dropped role {}", resource.spec.role);
        }


        remove_finalizer(resource.as_ref().clone(), context.kubernetes_client.clone()).await?;

        return Ok(Action::await_change());
    }

    let resource = ensure_finalizer(resource.as_ref().clone(), context.kubernetes_client.clone()).await?;

    let pg_connection = get_postgres_connection(&resource, context.kubernetes_client.clone()).await?;

    let password_text = resource.spec.password.get_password_text(&resource.spec.role);
    let username = &resource.spec.role;

    if pg_connection.query_opt("SELECT 1 FROM pg_roles WHERE rolname = $1", &[&username]).await?.is_some() {
        info!("User {username} already exists, updating password to be safe");
        pg_connection.execute(&format!("ALTER USER {username} WITH PASSWORD '{password_text}'"), &[]).await?;
    } else {
        info!("User {username} does not exist");
        pg_connection.execute(&format!("CREATE USER {username} WITH PASSWORD '{password_text}'"), &[]).await?;
    }

    if resource.spec.grant_role_to_admin_user == Some(true) {
        info!("Granting {username} to admin user");
        pg_connection.execute(&format!("GRANT {} TO {}", username, pg_connection.admin_username), &[]).await?;
    }

    info!("Granting connect to {} to database {}", username, pg_connection.database);
    pg_connection.execute(&format!("GRANT CONNECT ON DATABASE {} TO {}", pg_connection.database, username), &[]).await?;

    info!("Postgres role {username} reconciled in database.");


    if let Some(pg_bouncer_reference) = &resource.spec.register_in_pg_bouncer {
        info!("Registering role {username} in pg_bouncer {}", pg_bouncer_reference.name);

        let namespace = resource.namespace().expect("Resource should be namespaced");
        let name = resource.name_any();
        let pg_bouncer_users_api: Api<PgBouncerUser> = Api::namespaced(context.kubernetes_client.clone(), &namespace);


        let serverside = PatchParams::apply("postgres-topology-operator").force();

        let pg_bouncer_user = PgBouncerUser {
            metadata: ObjectMeta {
                namespace: Some(namespace),
                name: Some(name.clone()),
                owner_references: Some(vec![resource.controller_owner_ref(&()).unwrap()]),
                ..Default::default()
            },
            spec: PgBouncerUserSpec {
                username: username.clone(),
                password: resource.spec.password.clone(),
                pg_bouncer: pg_bouncer_reference.clone(),
            },
            status: None,
        };


        pg_bouncer_users_api.patch(&name, &serverside, &Patch::Apply(pg_bouncer_user)).await?;
        info!("Registered role {username} in pg_bouncer {}", pg_bouncer_reference.name);
    }




    Ok(Action::await_change())
}