use std::{
    collections::{BTreeMap, btree_map::Entry},
    io::Read,
};

use chrono::{DateTime, Utc};
use csv::ReaderBuilder;
use serde::Deserialize;

use crate::Error;

#[derive(Debug)]
pub struct Modified(BTreeMap<String, DateTime<Utc>>);

impl Modified {
    #[tracing::instrument(skip_all, err)]
    pub fn from_reader(reader: impl Read) -> Result<Self, Error> {
        let mut map = BTreeMap::new();

        // TODO: confirm that there aren't any weird quirks in the OSV CSV
        // dialect.
        let mut reader = ReaderBuilder::new().has_headers(false).from_reader(reader);
        for result in reader.deserialize() {
            let Advisory { modified_at, id } = result?;

            match map.entry(id) {
                Entry::Vacant(entry) => {
                    entry.insert(modified_at);
                }
                Entry::Occupied(entry) => {
                    return Err(Error::ModifiedDupe {
                        id: entry.key().clone(),
                    });
                }
            }
        }

        Ok(Self(map))
    }
}

impl IntoIterator for Modified {
    type Item = (String, DateTime<Utc>);

    type IntoIter = std::collections::btree_map::IntoIter<String, DateTime<Utc>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Deserialize)]
struct Advisory {
    pub modified_at: DateTime<Utc>,
    pub id: String,
}

impl Eq for Advisory {}

impl PartialEq for Advisory {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Ord for Advisory {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // This is intentionally reversed.
        other.modified_at.cmp(&self.modified_at)
    }
}

impl PartialOrd for Advisory {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
