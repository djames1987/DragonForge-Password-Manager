use std::{env, net::SocketAddr, sync::Arc};

use dragonforge_sync_server::{AppState, InMemoryStore, SyncStore, build_router};
use zeroize::Zeroize;

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
    let mut admin_token = env::var("DRAGONFORGE_SYNC_ADMIN_TOKEN").ok();
    if admin_token.as_ref().is_some_and(|token| token.len() < 32) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "DRAGONFORGE_SYNC_ADMIN_TOKEN must be at least 32 bytes",
        )
        .into());
    }

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

    let state = AppState::new(store, admin_token.as_deref());
    if let Some(token) = admin_token.as_mut() {
        token.zeroize();
    }
    let router = build_router(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    eprintln!("DragonForge sync server listening on {bind}");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    if tokio::signal::ctrl_c().await.is_err() {
        eprintln!("DragonForge sync server shutdown signal listener failed");
    }
}
