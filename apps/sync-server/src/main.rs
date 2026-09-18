use std::{env, net::SocketAddr, sync::Arc};

use dragonforge_sync_server::{AppState, InMemoryStore, SyncStore, build_router};

#[cfg(feature = "postgres")]
use dragonforge_sync_server::PostgresStore;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("DragonForge sync server failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let bind: SocketAddr = env::var("DRAGONFORGE_SYNC_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8787".to_owned())
        .parse()?;
    let admin_token = env::var("DRAGONFORGE_SYNC_ADMIN_TOKEN").ok();

    #[cfg(feature = "postgres")]
    let store: Arc<dyn SyncStore> = if let Ok(database_url) =
        env::var("DRAGONFORGE_SYNC_DATABASE_URL")
    {
        Arc::new(PostgresStore::connect(&database_url).await?)
    } else {
        eprintln!(
            "WARNING: DRAGONFORGE_SYNC_DATABASE_URL is not set; using volatile in-memory storage"
        );
        Arc::new(InMemoryStore::default())
    };

    #[cfg(not(feature = "postgres"))]
    let store: Arc<dyn SyncStore> = {
        if env::var_os("DRAGONFORGE_SYNC_DATABASE_URL").is_some() {
            return Err(
                "server was built without the postgres feature but DRAGONFORGE_SYNC_DATABASE_URL is set"
                    .into(),
            );
        }
        eprintln!(
            "WARNING: sync server was built without PostgreSQL support; using volatile in-memory storage"
        );
        Arc::new(InMemoryStore::default())
    };

    if admin_token.is_none() {
        eprintln!(
            "WARNING: DRAGONFORGE_SYNC_ADMIN_TOKEN is not set; account provisioning endpoint is disabled"
        );
    }

    let router = build_router(AppState::new(store, admin_token.as_deref()));
    let listener = tokio::net::TcpListener::bind(bind).await?;
    eprintln!("DragonForge sync server listening on {bind}");
    axum::serve(listener, router).await?;
    Ok(())
}
