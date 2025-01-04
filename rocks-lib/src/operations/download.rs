use std::{
    io::{self, Cursor, Read as _},
    path::PathBuf,
    string::FromUtf8Error,
};

use bytes::Bytes;
use thiserror::Error;
use url::Url;

use crate::{
    config::Config,
    luarocks,
    package::{PackageName, PackageReq, PackageSpec, PackageVersion, RemotePackageTypeFilterSpec},
    progress::{Progress, ProgressBar},
    remote_package_db::{RemotePackageDB, RemotePackageDBError, SearchError},
    remote_package_source::RemotePackageSource,
    rockspec::{Rockspec, RockspecError},
};

/// Builder for a rock downloader.
pub struct Download<'a> {
    package_req: &'a PackageReq,
    package_db: Option<&'a RemotePackageDB>,
    config: &'a Config,
    progress: &'a Progress<ProgressBar>,
}

impl<'a> Download<'a> {
    /// Construct a new `.src.rock` downloader.
    pub fn new(
        package_req: &'a PackageReq,
        config: &'a Config,
        progress: &'a Progress<ProgressBar>,
    ) -> Self {
        Self {
            package_req,
            package_db: None,
            config,
            progress,
        }
    }

    /// Sets the package database to use for searching for packages.
    /// Instantiated from the config if not set.
    pub fn package_db(self, package_db: &'a RemotePackageDB) -> Self {
        Self {
            package_db: Some(package_db),
            ..self
        }
    }

    /// Download the package's Rockspec.
    pub async fn download_rockspec(self) -> Result<DownloadedRockspec, SearchAndDownloadError> {
        match self.package_db {
            Some(db) => download_rockspec(self.package_req, db, self.progress).await,
            None => {
                let db = RemotePackageDB::from_config(self.config).await?;
                download_rockspec(self.package_req, &db, self.progress).await
            }
        }
    }

    /// Download a `.src.rock` to a file.
    /// `destination_dir` defaults to the current working directory if not set.
    pub async fn download_src_rock_to_file(
        self,
        destination_dir: Option<PathBuf>,
    ) -> Result<DownloadedPackedRock, SearchAndDownloadError> {
        match self.package_db {
            Some(db) => {
                download_src_rock_to_file(self.package_req, destination_dir, db, self.progress)
                    .await
            }
            None => {
                let db = RemotePackageDB::from_config(self.config).await?;
                download_src_rock_to_file(self.package_req, destination_dir, &db, self.progress)
                    .await
            }
        }
    }

    /// Search for a `.src.rock` and download it to memory.
    pub async fn search_and_download_src_rock(
        self,
    ) -> Result<DownloadedPackedRockBytes, SearchAndDownloadError> {
        match self.package_db {
            Some(db) => search_and_download_src_rock(self.package_req, db, self.progress).await,
            None => {
                let db = RemotePackageDB::from_config(self.config).await?;
                search_and_download_src_rock(self.package_req, &db, self.progress).await
            }
        }
    }

    pub(crate) async fn download_remote_rock(
        self,
    ) -> Result<RemoteRockDownload, SearchAndDownloadError> {
        match self.package_db {
            Some(db) => download_remote_rock(self.package_req, db, self.progress).await,
            None => {
                let db = RemotePackageDB::from_config(self.config).await?;
                download_remote_rock(self.package_req, &db, self.progress).await
            }
        }
    }
}

pub struct DownloadedPackedRockBytes {
    pub name: PackageName,
    pub version: PackageVersion,
    pub bytes: Bytes,
    pub file_name: String,
}

pub struct DownloadedPackedRock {
    pub name: PackageName,
    pub version: PackageVersion,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct DownloadedRockspec {
    pub rockspec: Rockspec,
    pub(crate) source: RemotePackageSource,
}

#[derive(Clone, Debug)]
pub(crate) enum RemoteRockDownload {
    RockspecOnly {
        rockspec_download: DownloadedRockspec,
    },
    BinaryRock {
        rockspec_download: DownloadedRockspec,
        packed_rock: Bytes,
    },
    SrcRock {
        rockspec_download: DownloadedRockspec,
        _src_rock: Bytes,
    },
}

impl RemoteRockDownload {
    pub fn rockspec(&self) -> &Rockspec {
        &self.rockspec_download().rockspec
    }
    pub fn rockspec_download(&self) -> &DownloadedRockspec {
        match self {
            Self::RockspecOnly { rockspec_download }
            | Self::BinaryRock {
                rockspec_download, ..
            }
            | Self::SrcRock {
                rockspec_download, ..
            } => rockspec_download,
        }
    }
}

#[derive(Error, Debug)]
pub enum DownloadRockspecError {
    #[error("failed to download rockspec: {0}")]
    Request(#[from] reqwest::Error),
    #[error("failed to convert rockspec response: {0}")]
    ResponseConversion(#[from] FromUtf8Error),
    #[error("error initialising remote package DB: {0}")]
    RemotePackageDB(#[from] RemotePackageDBError),
    #[error(transparent)]
    DownloadSrcRock(#[from] DownloadSrcRockError),
}

/// Find and download a rockspec for a given package requirement
async fn download_rockspec(
    package_req: &PackageReq,
    package_db: &RemotePackageDB,
    progress: &Progress<ProgressBar>,
) -> Result<DownloadedRockspec, SearchAndDownloadError> {
    let rockspec = match download_remote_rock(package_req, package_db, progress).await? {
        RemoteRockDownload::RockspecOnly {
            rockspec_download: rockspec,
        } => rockspec,
        RemoteRockDownload::BinaryRock {
            rockspec_download: rockspec,
            ..
        } => rockspec,
        RemoteRockDownload::SrcRock {
            rockspec_download: rockspec,
            ..
        } => rockspec,
    };
    Ok(rockspec)
}

async fn download_remote_rock(
    package_req: &PackageReq,
    package_db: &RemotePackageDB,
    progress: &Progress<ProgressBar>,
) -> Result<RemoteRockDownload, SearchAndDownloadError> {
    let remote_package = package_db.find(package_req, None, progress)?;
    progress.map(|p| p.set_message(format!("📥 Downloading rockspec for {}", package_req)));
    match &remote_package.source {
        RemotePackageSource::LuarocksRockspec(url) => {
            let package = &remote_package.package;
            let rockspec_name = format!("{}-{}.rockspec", package.name(), package.version());
            let bytes = reqwest::get(format!("{}/{}", &url, rockspec_name))
                .await
                .map_err(DownloadRockspecError::Request)?
                .bytes()
                .await
                .map_err(DownloadRockspecError::Request)?;
            let content = String::from_utf8(bytes.into())?;
            let rockspec = DownloadedRockspec {
                rockspec: Rockspec::new(&content)?,
                source: remote_package.source,
            };
            Ok(RemoteRockDownload::RockspecOnly {
                rockspec_download: rockspec,
            })
        }
        RemotePackageSource::RockspecContent(content) => {
            let rockspec = DownloadedRockspec {
                rockspec: Rockspec::new(content)?,
                source: remote_package.source,
            };
            Ok(RemoteRockDownload::RockspecOnly {
                rockspec_download: rockspec,
            })
        }
        RemotePackageSource::LuarocksBinaryRock(url) => {
            let rock = download_binary_rock(&remote_package.package, url, progress).await?;
            let rockspec = DownloadedRockspec {
                rockspec: unpack_rockspec(&rock).await?,
                source: remote_package.source,
            };
            Ok(RemoteRockDownload::BinaryRock {
                rockspec_download: rockspec,
                packed_rock: rock.bytes,
            })
        }
        RemotePackageSource::LuarocksSrcRock(url) => {
            let rock = download_src_rock(&remote_package.package, url, progress).await?;
            let rockspec = DownloadedRockspec {
                rockspec: unpack_rockspec(&rock).await?,
                source: remote_package.source,
            };
            Ok(RemoteRockDownload::SrcRock {
                rockspec_download: rockspec,
                _src_rock: rock.bytes,
            })
        }
        #[cfg(test)]
        RemotePackageSource::Test => unimplemented!(),
    }
}

#[derive(Error, Debug)]
pub enum SearchAndDownloadError {
    #[error(transparent)]
    Search(#[from] SearchError),
    #[error(transparent)]
    Download(#[from] DownloadSrcRockError),
    #[error(transparent)]
    DownloadRockspec(#[from] DownloadRockspecError),
    #[error("io operation failed: {0}")]
    Io(#[from] io::Error),
    #[error("UTF-8 conversion failed: {0}")]
    Utf8(#[from] FromUtf8Error),
    #[error(transparent)]
    Rockspec(#[from] RockspecError),
    #[error("error initialising remote package DB: {0}")]
    RemotePackageDB(#[from] RemotePackageDBError),
    #[error("failed to read packed rock: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{0} not found in the source rock.")]
    RockspecNotFoundInSrcRock(String),
}

async fn search_and_download_src_rock(
    package_req: &PackageReq,
    package_db: &RemotePackageDB,
    progress: &Progress<ProgressBar>,
) -> Result<DownloadedPackedRockBytes, SearchAndDownloadError> {
    let filter = Some(RemotePackageTypeFilterSpec {
        rockspec: false,
        binary: false,
        src: true,
    });
    let remote_package = package_db.find(package_req, filter, progress)?;
    Ok(download_src_rock(
        &remote_package.package,
        unsafe { &remote_package.source.url() },
        progress,
    )
    .await?)
}

#[derive(Error, Debug)]
#[error("failed to download source rock: {0}")]
pub struct DownloadSrcRockError(#[from] reqwest::Error);

pub(crate) async fn download_src_rock(
    package: &PackageSpec,
    server_url: &Url,
    progress: &Progress<ProgressBar>,
) -> Result<DownloadedPackedRockBytes, DownloadSrcRockError> {
    download_packed_rock_impl(package, server_url, "src.rock", progress).await
}

pub(crate) async fn download_binary_rock(
    package: &PackageSpec,
    server_url: &Url,
    progress: &Progress<ProgressBar>,
) -> Result<DownloadedPackedRockBytes, DownloadSrcRockError> {
    download_packed_rock_impl(
        package,
        server_url,
        format!("{}.rock", luarocks::current_platform_luarocks_identifier()).as_str(),
        progress,
    )
    .await
}

async fn download_src_rock_to_file(
    package_req: &PackageReq,
    destination_dir: Option<PathBuf>,
    package_db: &RemotePackageDB,
    progress: &Progress<ProgressBar>,
) -> Result<DownloadedPackedRock, SearchAndDownloadError> {
    progress.map(|p| p.set_message(format!("📥 Downloading {}", package_req)));

    let rock = search_and_download_src_rock(package_req, package_db, progress).await?;
    let full_rock_name = mk_packed_rock_name(&rock.name, &rock.version, "src.rock");
    tokio::fs::write(
        destination_dir
            .map(|dest| dest.join(&full_rock_name))
            .unwrap_or_else(|| full_rock_name.clone().into()),
        &rock.bytes,
    )
    .await?;

    Ok(DownloadedPackedRock {
        name: rock.name.to_owned(),
        version: rock.version.to_owned(),
        path: full_rock_name.into(),
    })
}

async fn download_packed_rock_impl(
    package: &PackageSpec,
    server_url: &Url,
    ext: &str,
    progress: &Progress<ProgressBar>,
) -> Result<DownloadedPackedRockBytes, DownloadSrcRockError> {
    progress.map(|p| {
        p.set_message(format!(
            "📥 Downloading {}-{}.{}",
            package.name(),
            package.version(),
            ext,
        ))
    });
    let full_rock_name = mk_packed_rock_name(package.name(), package.version(), ext);

    let bytes = reqwest::get(format!("{}/{}", server_url, full_rock_name))
        .await?
        .bytes()
        .await?;
    Ok(DownloadedPackedRockBytes {
        name: package.name().clone(),
        version: package.version().clone(),
        bytes,
        file_name: full_rock_name,
    })
}

fn mk_packed_rock_name(name: &PackageName, version: &PackageVersion, ext: &str) -> String {
    format!("{}-{}.{}", name, version, ext)
}

pub(crate) async fn unpack_rockspec(
    rock: &DownloadedPackedRockBytes,
) -> Result<Rockspec, SearchAndDownloadError> {
    let cursor = Cursor::new(&rock.bytes);
    let rockspec_file_name = format!("{}-{}.rockspec", rock.name, rock.version);
    let mut zip = zip::ZipArchive::new(cursor)?;
    let rockspec_index = (0..zip.len())
        .find(|&i| zip.by_index(i).unwrap().name().eq(&rockspec_file_name))
        .ok_or(SearchAndDownloadError::RockspecNotFoundInSrcRock(
            rockspec_file_name,
        ))?;
    let mut rockspec_file = zip.by_index(rockspec_index)?;
    let mut content = String::new();
    rockspec_file.read_to_string(&mut content)?;
    let rockspec = Rockspec::new(&content)?;
    Ok(rockspec)
}
