use std::env;

use crate::package::PackageVersion;
use crate::project::project_toml::RemoteProjectTomlValidationError;
use crate::rockspec::Rockspec;
use crate::TOOL_VERSION;
use crate::{config::Config, project::Project};

use reqwest::StatusCode;
use reqwest::{
    multipart::{Form, Part},
    Client,
};
use serde::Deserialize;
use serde_enum_str::Serialize_enum_str;
use thiserror::Error;
use url::Url;

#[cfg(not(target_env = "msvc"))]
use gpgme::{Context, Data};
#[cfg(not(target_env = "msvc"))]
use std::io::Read;

/// A rocks package uploader, providing fine-grained control
/// over how a package should be uploaded.
pub struct ProjectUpload<'a> {
    project: Project,
    api_key: Option<ApiKey>,
    sign_protocol: SignatureProtocol,
    config: &'a Config,
}

impl<'a> ProjectUpload<'a> {
    /// Construct a new package uploader.
    pub fn new(project: Project, config: &'a Config) -> Self {
        Self {
            project,
            api_key: None,
            sign_protocol: SignatureProtocol::default(),
            config,
        }
    }

    /// Set the luarocks API key.
    pub fn api_key(self, api_key: ApiKey) -> Self {
        Self {
            api_key: Some(api_key),
            ..self
        }
    }

    /// Set the signature protocol.
    pub fn sign_protocol(self, sign_protocol: SignatureProtocol) -> Self {
        Self {
            sign_protocol,
            ..self
        }
    }

    /// Upload a package to a luarocks server.
    pub async fn upload_to_luarocks(self) -> Result<(), UploadError> {
        let api_key = self.api_key.unwrap_or(ApiKey::new()?);
        upload_from_project(&self.project, &api_key, self.sign_protocol, self.config).await
    }
}

#[derive(Deserialize, Debug)]
pub struct VersionCheckResponse {
    version: String,
}

#[derive(Error, Debug)]
pub enum ToolCheckError {
    #[error("error parsing tool check URL: {0}")]
    ParseError(#[from] url::ParseError),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error("`lux` is out of date with {0}'s expected tool version! `lux` is at version {TOOL_VERSION}, server is at {server_version}", server_version = _1.version)]
    ToolOutdated(String, VersionCheckResponse),
}

#[derive(Error, Debug)]
pub enum UserCheckError {
    #[error("error parsing user check URL: {0}")]
    ParseError(#[from] url::ParseError),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error("invalid API key provided")]
    UserNotFound,
    #[error("server {0} responded with error status: {1}")]
    Server(Url, StatusCode),
}

#[derive(Error, Debug)]
#[error("could not check rock status on server: {0}")]
pub enum RockCheckError {
    #[error(transparent)]
    ParseError(#[from] url::ParseError),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
}

#[derive(Error, Debug)]
#[error(transparent)]
pub enum UploadError {
    #[error("error parsing upload URL: {0}")]
    ParseError(#[from] url::ParseError),
    Lua(#[from] mlua::Error),
    Request(#[from] reqwest::Error),
    #[error("server {0} responded with error status: {1}")]
    Server(Url, StatusCode),
    #[error("client error when requesting {0}\nStatus code: {1}")]
    Client(Url, StatusCode),
    RockCheck(#[from] RockCheckError),
    #[error("rock already exists on server: {0}")]
    RockExists(Url),
    #[error("unable to read rockspec: {0}")]
    RockspecRead(#[from] std::io::Error),
    #[cfg(not(target_env = "msvc"))]
    #[error("{0}.\nHINT: If you'd like to skip the signing step supply `--sign-protocol none` to the CLI")]
    Signature(#[from] gpgme::Error),
    ToolCheck(#[from] ToolCheckError),
    UserCheck(#[from] UserCheckError),
    ApiKeyUnspecified(#[from] ApiKeyUnspecified),
    ValidationError(#[from] RemoteProjectTomlValidationError),
    #[error(
        "unsupported version: `{0}`.\nLux can upload packages with a SemVer version, 'dev' or 'scm'"
    )]
    UnsupportedVersion(String),
    #[error("{0}")] // We don't know the concrete error type
    Rockspec(String),
}

pub struct ApiKey(String);

#[derive(Error, Debug)]
#[error("no API key provided! Please set the $LUX_API_KEY variable")]
pub struct ApiKeyUnspecified;

impl ApiKey {
    /// Retrieves the rocks API key from the `$LUX_API_KEY` environment
    /// variable and seals it in this struct.
    pub fn new() -> Result<Self, ApiKeyUnspecified> {
        Ok(Self(
            env::var("LUX_API_KEY").map_err(|_| ApiKeyUnspecified)?,
        ))
    }

    /// Creates an API key from a String.
    ///
    /// # Safety
    ///
    /// This struct is designed to be sealed without a [`Display`](std::fmt::Display) implementation
    /// so that it can never accidentally be printed.
    ///
    /// Ensure that you do not do anything else with the API key string prior to sealing it in this
    /// struct.
    pub unsafe fn from(str: String) -> Self {
        Self(str)
    }

    /// Retrieves the underlying API key as a [`String`].
    ///
    /// # Safety
    ///
    /// Strings may accidentally be printed as part of its [`Display`](std::fmt::Display)
    /// implementation. Ensure that you never pass this variable somewhere it may be displayed.
    pub unsafe fn get(&self) -> &String {
        &self.0
    }
}

#[derive(Serialize_enum_str, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[cfg_attr(feature = "clap", clap(rename_all = "lowercase"))]
#[serde(rename_all = "lowercase")]
pub enum SignatureProtocol {
    None,
    #[cfg(not(target_env = "msvc"))]
    Assuan,
    #[cfg(not(target_env = "msvc"))]
    CMS,
    #[cfg(not(target_env = "msvc"))]
    Default,
    #[cfg(not(target_env = "msvc"))]
    G13,
    #[cfg(not(target_env = "msvc"))]
    GPGConf,
    #[cfg(not(target_env = "msvc"))]
    OpenPGP,
    #[cfg(not(target_env = "msvc"))]
    Spawn,
    #[cfg(not(target_env = "msvc"))]
    UIServer,
}

#[cfg(not(target_env = "msvc"))]
impl Default for SignatureProtocol {
    fn default() -> Self {
        Self::Default
    }
}

#[cfg(target_env = "msvc")]
impl Default for SignatureProtocol {
    fn default() -> Self {
        Self::None
    }
}

#[cfg(not(target_env = "msvc"))]
impl From<SignatureProtocol> for gpgme::Protocol {
    fn from(val: SignatureProtocol) -> Self {
        match val {
            SignatureProtocol::Default => gpgme::Protocol::Default,
            SignatureProtocol::OpenPGP => gpgme::Protocol::OpenPgp,
            SignatureProtocol::CMS => gpgme::Protocol::Cms,
            SignatureProtocol::GPGConf => gpgme::Protocol::GpgConf,
            SignatureProtocol::Assuan => gpgme::Protocol::Assuan,
            SignatureProtocol::G13 => gpgme::Protocol::G13,
            SignatureProtocol::UIServer => gpgme::Protocol::UiServer,
            SignatureProtocol::Spawn => gpgme::Protocol::Spawn,
            SignatureProtocol::None => unreachable!(),
        }
    }
}

async fn upload_from_project(
    project: &Project,
    api_key: &ApiKey,
    #[cfg(target_env = "msvc")] _protocol: SignatureProtocol,
    #[cfg(not(target_env = "msvc"))] protocol: SignatureProtocol,
    config: &Config,
) -> Result<(), UploadError> {
    let client = Client::builder().https_only(true).build()?;

    let rockspec = project.toml().into_remote()?;

    if let PackageVersion::StringVer(ver) = rockspec.version() {
        return Err(UploadError::UnsupportedVersion(ver.to_string()));
    }

    helpers::ensure_tool_version(&client, config.server()).await?;
    helpers::ensure_user_exists(&client, api_key, config.server()).await?;

    if helpers::rock_exists(
        &client,
        api_key,
        rockspec.package(),
        rockspec.version(),
        config.server(),
    )
    .await?
    {
        return Err(UploadError::RockExists(config.server().clone()));
    }

    let rockspec_content = rockspec
        .to_lua_remote_rockspec_string()
        .map_err(|err| UploadError::Rockspec(err.to_string()))?;

    #[cfg(target_env = "msvc")]
    let signed: Option<String> = None;

    #[cfg(not(target_env = "msvc"))]
    let signed = if let SignatureProtocol::None = protocol {
        None
    } else {
        let mut ctx = Context::from_protocol(protocol.into())?;
        let mut signature = Data::new()?;

        ctx.set_armor(true);
        ctx.sign_detached(rockspec_content.clone(), &mut signature)?;

        let mut signature_str = String::new();
        signature.read_to_string(&mut signature_str)?;

        Some(signature_str)
    };

    let rockspec = Part::text(rockspec_content)
        .file_name(format!(
            "{}-{}.rockspec",
            rockspec.package(),
            rockspec.version()
        ))
        .mime_str("application/octet-stream")?;

    let multipart = {
        let multipart = Form::new().part("rockspec_file", rockspec);

        match signed {
            Some(signature) => {
                let part = Part::text(signature).file_name("project.rockspec.sig");
                multipart.part("rockspec_sig", part)
            }
            None => multipart,
        }
    };

    let response = client
        .post(unsafe { helpers::url_for_method(config.server(), api_key, "upload")? })
        .multipart(multipart)
        .send()
        .await?;

    let status = response.status();
    if status.is_client_error() {
        Err(UploadError::Client(config.server().clone(), status))
    } else if status.is_server_error() {
        Err(UploadError::Server(config.server().clone(), status))
    } else {
        Ok(())
    }
}

mod helpers {
    use super::*;
    use crate::package::{PackageName, PackageVersion};
    use crate::upload::RockCheckError;
    use crate::upload::{ToolCheckError, UserCheckError};
    use reqwest::Client;
    use url::Url;

    /// WARNING: This function is unsafe,
    /// because it adds the unmasked API key to the URL.
    /// When using URLs created by this function,
    /// pay attention not to leak the API key in errors.
    pub(crate) unsafe fn url_for_method(
        server_url: &Url,
        api_key: &ApiKey,
        endpoint: &str,
    ) -> Result<Url, url::ParseError> {
        server_url
            .join("api/1/")
            .expect("error constructing 'api/1/' path")
            .join(&format!("{}/", api_key.get()))?
            .join(endpoint)
    }

    pub(crate) async fn ensure_tool_version(
        client: &Client,
        server_url: &Url,
    ) -> Result<(), ToolCheckError> {
        let url = server_url.join("api/tool_version")?;
        let response: VersionCheckResponse = client
            .post(url)
            .json(&("current", TOOL_VERSION))
            .send()
            .await?
            .json()
            .await?;

        if response.version == TOOL_VERSION {
            Ok(())
        } else {
            Err(ToolCheckError::ToolOutdated(
                server_url.to_string(),
                response,
            ))
        }
    }

    pub(crate) async fn ensure_user_exists(
        client: &Client,
        api_key: &ApiKey,
        server_url: &Url,
    ) -> Result<(), UserCheckError> {
        let response = client
            .get(unsafe { url_for_method(server_url, api_key, "status")? })
            .send()
            .await?;
        let status = response.status();
        if status.is_client_error() {
            Err(UserCheckError::UserNotFound)
        } else if status.is_server_error() {
            Err(UserCheckError::Server(server_url.clone(), status))
        } else {
            Ok(())
        }
    }

    pub(crate) async fn rock_exists(
        client: &Client,
        api_key: &ApiKey,
        name: &PackageName,
        version: &PackageVersion,
        server: &Url,
    ) -> Result<bool, RockCheckError> {
        Ok(client
            .get(unsafe { url_for_method(server, api_key, "check_rockspec")? })
            .query(&(
                ("package", name.to_string()),
                ("version", version.to_string()),
            ))
            .send()
            .await?
            .text()
            .await?
            != "{}")
    }
}
