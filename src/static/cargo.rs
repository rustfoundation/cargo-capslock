use std::{
    collections::BTreeSet,
    ffi::{OsStr, OsString},
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::Path,
};

#[derive(Debug, Default)]
pub struct ExecutableSet(BTreeSet<OsString>);

impl ExecutableSet {
    pub fn contains_prefix_match(&self, needle: impl AsRef<OsStr>) -> bool {
        let needle = needle.as_ref().to_normalised_file_name();
        self.0.iter().any(|haystack| needle.starts_with(haystack))
    }

    pub fn insert(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        self.0.insert(path.as_ref().to_normalised_file_name()?);
        Ok(())
    }
}

trait PathUtil {
    fn to_normalised_file_name(&self) -> anyhow::Result<OsString>;
}

impl PathUtil for Path {
    fn to_normalised_file_name(&self) -> anyhow::Result<OsString> {
        Ok(self
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("no file name for {}", self.display()))?
            .to_normalised_file_name())
    }
}

trait OsStrUtil {
    fn to_normalised_file_name(&self) -> OsString;
    fn starts_with(&self, other: impl AsRef<OsStr>) -> bool;
}

impl OsStrUtil for OsStr {
    fn to_normalised_file_name(&self) -> OsString {
        OsString::from_vec(
            self.as_bytes()
                .iter()
                .copied()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { b'_' })
                .collect(),
        )
    }

    fn starts_with(&self, other: impl AsRef<OsStr>) -> bool {
        self.as_bytes().starts_with(other.as_ref().as_bytes())
    }
}
