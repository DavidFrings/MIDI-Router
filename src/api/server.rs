use crate::{
    api::{
        auth::Lockout,
        routes::{ApiState, configure},
    },
    router::remote::RemoteHub,
    utils::config::ApiConfig,
};
use actix_web::{App, HttpServer, middleware::Logger, web};
use log::warn;
use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

/// `exe_dir/web-remote` (installed), then `cwd/tools/web-remote` (dev checkout).
pub fn find_web_root(exe_dir: Option<&Path>, cwd: &Path) -> Option<PathBuf> {
    exe_dir
        .map(|d| d.join("web-remote"))
        .into_iter()
        .chain([cwd.join("tools/web-remote")])
        .find(|d| d.join("index.html").is_file())
}

pub async fn start_server(
    config: ApiConfig,
    hub: Arc<RemoteHub>,
) -> anyhow::Result<actix_web::dev::Server> {
    let exe = env::current_exe().ok();
    let root = find_web_root(exe.as_deref().and_then(Path::parent), &env::current_dir()?);
    if root.is_none() {
        warn!(target: "api", "web-remote files not found, serving an error page at /");
    }

    let state = web::Data::new(ApiState { hub, pin: config.pin, lockout: Lockout::default() });

    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default().exclude("/health"))
            .configure(configure(state.clone(), root.clone()))
    })
    .bind((config.bind_address.as_str(), config.port))?
    .disable_signals()
    .run();

    Ok(server)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn find_web_root_prefers_exe_dir() {
        let exe = dir("find-root-exe");
        let cwd = dir("find-root-cwd");
        for root in [exe.join("web-remote"), cwd.join("tools/web-remote")] {
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join("index.html"), "").unwrap();
        }

        assert_eq!(find_web_root(Some(&exe), &cwd), Some(exe.join("web-remote")));
        assert_eq!(find_web_root(None, &cwd), Some(cwd.join("tools/web-remote")));
        fs::remove_file(cwd.join("tools/web-remote/index.html")).unwrap();
        assert_eq!(find_web_root(None, &cwd), None);

        fs::remove_dir_all(exe).unwrap();
        fs::remove_dir_all(cwd).unwrap();
    }

    #[actix_web::test]
    async fn start_server_returns_err_when_port_taken() {
        let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let config = ApiConfig {
            enabled: true,
            bind_address: "127.0.0.1".into(),
            port: taken.local_addr().unwrap().port(),
            pin: "4711".into(),
        };

        assert!(start_server(config, RemoteHub::new()).await.is_err());
    }
}
