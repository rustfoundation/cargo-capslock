use std::{
    fmt::Debug,
    fs::File,
    io::{BufReader, BufWriter, Seek},
    path::{Path, PathBuf},
    sync::LazyLock,
};

use itertools::Itertools;
pub use osv;
use osv::schema::Vulnerability;
use reqwest::blocking::Client;
use xdg::BaseDirectories;
use zip::ZipArchive;

pub use crate::error::Error;
use crate::modified::Modified;

mod error;
mod modified;

static BASE_DIRS: LazyLock<BaseDirectories> =
    LazyLock::new(|| BaseDirectories::with_prefix("cargo-capslock"));

#[derive(Debug)]
pub struct Cache {
    client: Client,
    path: PathBuf,
}

impl Cache {
    #[tracing::instrument(err)]
    pub fn new(base: impl AsRef<Path> + Debug) -> Result<Self, Error> {
        let path = base.as_ref().join("osv");
        std::fs::create_dir_all(&path).map_err(|e| Error::Create {
            e,
            path: path.display().to_string(),
        })?;

        Ok(Self {
            client: Client::builder()
                .user_agent("cargo-capslock/0.0.0")
                .build()
                .map_err(Error::Client)?,
            path,
        })
    }

    pub fn default_path() -> Result<PathBuf, Error> {
        BASE_DIRS.get_cache_home().ok_or(Error::Home)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn get(&self, id: &str) -> Result<Vulnerability, Error> {
        self.get_local(self.advisory_path(id))
    }

    pub fn try_iter_advisories(
        &self,
    ) -> Result<impl Iterator<Item = Result<Vulnerability, Error>>, Error> {
        Ok(std::fs::read_dir(&self.path)
            .map_err(Error::ReadDir)?
            .filter_map_ok(|entry| match entry.file_type() {
                Ok(ty) if ty.is_file() => Some(Ok(entry)),
                Ok(_) => None,
                Err(e) => Some(Err(e)),
            })
            .flatten()
            .map(|result| -> Result<Vulnerability, Error> {
                self.get_local(result.map_err(Error::ReadDir)?.path())
            }))
    }

    #[tracing::instrument(skip(progress), err)]
    pub fn update<F>(&mut self, base_url: &str, progress: Option<F>) -> Result<(), Error>
    where
        F: Fn(usize),
    {
        let update_progress = move |n| {
            if let Some(progress) = &progress {
                progress(n);
            }
        };

        // Short circuit if there are no advisories in the cache, because then
        // we can just download the full set.
        if self.is_empty()? {
            return self.update_from_all(base_url);
        }

        let response = self
            .client
            .get(format!("{base_url}%2Fmodified_id.csv?alt=media"))
            .send()
            .map_err(Error::ModifiedRequest)?
            .error_for_status()
            .map_err(Error::ModifiedResponse)?;

        let mut n = 0;
        for (id, modified_at) in Modified::from_reader(response)? {
            update_progress(n);
            n += 1;

            // Skip if there isn't an update.
            if let Ok(advisory) = self.get(&id)
                && advisory.modified >= modified_at
            {
                continue;
            }

            let advisory = self.get_remote(base_url, &id)?;

            let file =
                File::create(self.advisory_path(&id)).map_err(|e| Error::AdvisoryCreate {
                    e,
                    id: id.to_string(),
                })?;
            serde_json::to_writer_pretty(BufWriter::new(file), &advisory)
                .map_err(move |e| Error::AdvisoryWrite { e, id })?;
        }

        update_progress(n);
        Ok(())
    }

    fn advisory_path(&self, id: &str) -> PathBuf {
        self.path.join(format!("{id}.json"))
    }

    #[tracing::instrument(level = "TRACE", err)]
    fn get_local(&self, path: impl AsRef<Path> + Debug) -> Result<Vulnerability, Error> {
        let file = File::open(path.as_ref()).map_err(|e| Error::AdvisoryOpen {
            e,
            path: path.as_ref().display().to_string(),
        })?;

        serde_json::from_reader(BufReader::new(file)).map_err(|e| Error::AdvisoryLocalParse {
            e,
            path: path.as_ref().display().to_string(),
        })
    }

    #[tracing::instrument(level = "TRACE", err)]
    fn get_remote(&self, base_url: &str, id: &str) -> Result<Vulnerability, Error> {
        self.client
            .get(format!("{base_url}%2F{id}.json?alt=media"))
            .send()
            .map_err(|e| Error::AdvisoryRequest {
                e,
                id: id.to_string(),
            })?
            .error_for_status()
            .map_err(|e| Error::AdvisoryResponse {
                e,
                id: id.to_string(),
            })?
            .json()
            .map_err(|e| Error::AdvisoryRemoteParse {
                e,
                id: id.to_string(),
            })
    }

    #[tracing::instrument(level = "TRACE", err)]
    fn is_empty(&self) -> Result<bool, Error> {
        for result in std::fs::read_dir(&self.path).map_err(Error::ReadDir)? {
            let entry = result.map_err(Error::ReadDir)?;
            if entry
                .file_type()
                .map_err(|e| Error::AdvisoryOpen {
                    e,
                    path: entry.path().display().to_string(),
                })?
                .is_file()
            {
                return Ok(true);
            }
        }

        Ok(true)
    }

    #[tracing::instrument(err)]
    fn update_from_all(&mut self, base_url: &str) -> Result<(), Error> {
        let mut temp = tempfile::tempfile().map_err(Error::AllTemp)?;
        self.client
            .get(format!("{base_url}%2Fall.zip?alt=media"))
            .send()
            .map_err(Error::AllRequest)?
            .error_for_status()
            .map_err(Error::AllResponse)?
            .copy_to(&mut temp)
            .map_err(Error::AllResponse)?;

        temp.rewind().map_err(Error::AllTemp)?;

        let mut archive = ZipArchive::new(temp).map_err(Error::AllOpen)?;
        archive.extract(&self.path).map_err(Error::AllExtract)
    }
}
