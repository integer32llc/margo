use axum::{
    extract::Request,
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use axum_extra::{
    headers::{self, authorization::Basic},
    TypedHeader,
};
use registry_conformance::{CommandExt, CreatedCrate, Registry, RegistryBuilder};
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

type BasicAuth = Option<(String, String)>;

#[derive(Debug, Default)]
pub struct MargoBuilder {
    webserver_basic_auth: BasicAuth,
}

impl MargoBuilder {
    fn enable_basic_auth_(mut self, username: &str, password: &str) -> Self {
        self.webserver_basic_auth = Some((username.into(), password.into()));
        self
    }

    async fn start_(
        self,
        directory: impl Into<PathBuf>,
    ) -> Result<<Self as RegistryBuilder>::Registry, StartError> {
        use start_error::*;

        let Self {
            webserver_basic_auth,
        } = self;
        let auth_required = webserver_basic_auth.is_some();

        let directory = directory.into();

        let webserver_cancel = CancellationToken::new();

        let address = "127.0.0.1:0";
        let listener = TcpListener::bind(address)
            .await
            .context(BindSnafu { address })?;
        let webserver_address = listener.local_addr().context(AddressSnafu)?;

        let serve_files = ServeDir::new(&directory);

        let auth_middleware = middleware::from_fn(move |hdr, req, next| {
            let webserver_basic_auth = webserver_basic_auth.clone();
            auth(webserver_basic_auth, hdr, req, next)
        });

        let serve_files = Router::new()
            .fallback_service(serve_files)
            .layer(auth_middleware);

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

        let mut cmd = this.command();

        cmd.arg("init")
            .args(["--base-url", &format!("http://{webserver_address}")])
            .arg("--defaults");

        if auth_required {
            cmd.args(["--auth-required", "true"]);
        }

        cmd.arg(&this.directory)
            .expect_success()
            .await
            .context(ExecutionSnafu)?;

        Ok(this)
    }
}

async fn auth(
    webserver_basic_auth: BasicAuth,
    auth_header: Option<TypedHeader<headers::Authorization<Basic>>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Some((username, password)) = webserver_basic_auth {
        let creds_match = auth_header.as_ref().map_or(false, |auth| {
            auth.username() == username && auth.password() == password
        });

        if !creds_match {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    Ok(next.run(req).await.into_response())
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

    async fn remove_crate_(&mut self, crate_: &CreatedCrate) -> Result<(), RemoveError> {
        use remove_error::*;

        self.command()
            .arg("rm")
            .arg("--registry")
            .arg(&self.directory)
            .arg(crate_.name())
            .args(["--version", crate_.version()])
            .expect_success()
            .await
            .context(ExecutionSnafu)?;

        Ok(())
    }

    async fn yank_crate_(&mut self, crate_: &CreatedCrate, yanked: bool) -> Result<(), YankError> {
        use yank_error::*;

        let mut cmd = self.command();
        cmd.arg("yank")
            .arg("--registry")
            .arg(&self.directory)
            .arg(crate_.name())
            .args(["--version", crate_.version()]);

        if !yanked {
            cmd.arg("--undo");
        }

        cmd.expect_success().await.context(ExecutionSnafu)?;

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
pub enum YankError {
    #[snafu(display("Could not yank the crate from the registry"))]
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
pub enum RemoveError {
    #[snafu(display("Could not remove the crate from the registry"))]
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

impl RegistryBuilder for MargoBuilder {
    type Registry = Margo;
    type Error = Error;

    fn enable_basic_auth(self, username: &str, password: &str) -> Self {
        self.enable_basic_auth_(username, password)
    }

    async fn start(self, directory: impl Into<PathBuf>) -> Result<Self::Registry, Error> {
        Ok(self.start_(directory).await?)
    }
}

impl Registry for Margo {
    type Builder = MargoBuilder;
    type Error = Error;

    async fn registry_url(&self) -> String {
        format!("sparse+http://{}/", self.webserver_address)
    }

    async fn publish_crate(&mut self, crate_: &CreatedCrate) -> Result<(), Error> {
        Ok(self.publish_crate_(crate_).await?)
    }

    async fn remove_crate(&mut self, crate_: &CreatedCrate) -> Result<(), Error> {
        Ok(self.remove_crate_(crate_).await?)
    }

    async fn yank_crate(&mut self, crate_: &CreatedCrate) -> Result<(), Error> {
        Ok(self.yank_crate_(crate_, true).await?)
    }

    async fn unyank_crate(&mut self, crate_: &CreatedCrate) -> Result<(), Error> {
        Ok(self.yank_crate_(crate_, false).await?)
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
    Remove { source: RemoveError },

    #[snafu(transparent)]
    Yank { source: YankError },

    #[snafu(transparent)]
    Shutdown { source: ShutdownError },
}
