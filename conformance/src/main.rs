#![deny(rust_2018_idioms)]
#![deny(unused_crate_dependencies)]

use axum::Router;
use registry_conformance::{CommandExt, CreatedCrate, Registry};
use snafu::prelude::*;
use std::{
    env,
    future::IntoFuture,
    io,
    net::SocketAddr,
    path::{Path, PathBuf},
    process::ExitCode,
};
use tokio::{net::TcpListener, process::Command, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> Result<ExitCode, BuildError> {
    if env::var_os("MARGO_BINARY").is_none() {
        Margo::build().await?;
    }

    Ok(registry_conformance::test_conformance::<Margo>(std::env::args()).await)
}

pub struct Margo {
    directory: PathBuf,
    webserver_cancel: CancellationToken,
    webserver_address: SocketAddr,
    webserver: JoinHandle<io::Result<()>>,
}

impl Margo {
    const EXE_PATH: &'static str = "../target/debug/margo";

    async fn build() -> Result<(), BuildError> {
        use build_error::*;

        Command::new("cargo")
            .current_dir("..")
            .arg("build")
            .expect_success()
            .await
            .map(drop)
            .context(ExecutionSnafu)
    }

    async fn start_(directory: impl Into<PathBuf>) -> Result<Self, StartError> {
        use start_error::*;

        let directory = directory.into();

        let webserver_cancel = CancellationToken::new();

        let address = "127.0.0.1:0";
        let listener = TcpListener::bind(address)
            .await
            .context(BindSnafu { address })?;
        let webserver_address = listener.local_addr().context(AddressSnafu)?;

        let serve_files = ServeDir::new(&directory);
        let serve_files = Router::new().fallback_service(serve_files);

        let webserver = axum::serve(listener, serve_files)
            .with_graceful_shutdown(webserver_cancel.clone().cancelled_owned())
            .into_future();

        let webserver = tokio::spawn(webserver);

        let this = Margo {
            directory,
            webserver_cancel,
            webserver_address,
            webserver,
        };

        this.command()
            .arg("init")
            .args(["--base-url", &format!("http://{webserver_address}")])
            .arg("--defaults")
            .arg(&this.directory)
            .expect_success()
            .await
            .context(ExecutionSnafu)?;

        Ok(this)
    }

    async fn publish_crate_(&mut self, crate_: &CreatedCrate) -> Result<(), PublishError> {
        use publish_error::*;

        let package_path = crate_.package().await.context(PackageSnafu)?;

        self.command()
            .arg("add")
            .arg("--registry")
            .arg(&self.directory)
            .arg(package_path)
            .expect_success()
            .await
            .context(ExecutionSnafu)?;

        Ok(())
    }

    async fn shutdown_(self) -> Result<(), ShutdownError> {
        use shutdown_error::*;

        self.webserver_cancel.cancel();
        self.webserver
            .await
            .context(JoinSnafu)?
            .context(ServeSnafu)?;

        Ok(())
    }

    fn command(&self) -> Command {
        let exe_path = env::var_os("MARGO_BINARY").map(PathBuf::from);
        let exe_path = exe_path
            .as_deref()
            .unwrap_or_else(|| Path::new(Self::EXE_PATH));

        let mut cmd = Command::new(exe_path);

        cmd.kill_on_drop(true);

        cmd
    }
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum BuildError {
    #[snafu(display("Could not build the registry"))]
    Execution {
        source: registry_conformance::CommandError,
    },
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum StartError {
    #[snafu(display("Could not bind to address {address}"))]
    Bind {
        source: std::io::Error,
        address: String,
    },

    #[snafu(display("Could not get the listening address"))]
    Address { source: std::io::Error },

    #[snafu(display("Could not initialize the registry"))]
    Execution {
        source: registry_conformance::CommandError,
    },
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum PublishError {
    #[snafu(display("Could not package the crate"))]
    Package {
        source: registry_conformance::PackageError,
    },

    #[snafu(display("Could not add the crate to the registry"))]
    Execution {
        source: registry_conformance::CommandError,
    },
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum ShutdownError {
    #[snafu(display("The webserver task panicked"))]
    Join { source: tokio::task::JoinError },

    #[snafu(display("The webserver had an error"))]
    Serve { source: std::io::Error },
}

impl Registry for Margo {
    type Error = Error;

    async fn start(directory: impl Into<PathBuf>) -> Result<Self, Error> {
        Ok(Self::start_(directory).await?)
    }

    async fn registry_url(&self) -> String {
        format!("sparse+http://{}/", self.webserver_address)
    }

    async fn publish_crate(&mut self, crate_: &CreatedCrate) -> Result<(), Error> {
        Ok(self.publish_crate_(crate_).await?)
    }

    async fn shutdown(self) -> Result<(), Error> {
        Ok(self.shutdown_().await?)
    }
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum Error {
    #[snafu(transparent)]
    Start { source: StartError },

    #[snafu(transparent)]
    Publish { source: PublishError },

    #[snafu(transparent)]
    Shutdown { source: ShutdownError },
}
