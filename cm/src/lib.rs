use std::{
    collections::VecDeque, error::Error as StdError, fmt::Debug, hash::Hash, io::BufRead,
    ops::Deref, str::FromStr,
};

use indexmap::IndexMap;
use itertools::Itertools;
use thiserror::Error;

pub struct Document<Key, Value> {
    records: IndexMap<Key, Vec<Value>>,
}

impl<Key, Value> Document<Key, Value>
where
    Key: FromStr + Hash + Eq,
    <Key as FromStr>::Err: StdError + Debug,
    Value: FromStr,
    <Value as FromStr>::Err: StdError + Debug,
{
    pub fn from_reader<R>(
        reader: R,
    ) -> Result<Self, Error<<Key as FromStr>::Err, <Value as FromStr>::Err>>
    where
        R: BufRead,
    {
        let mut records = IndexMap::new();

        for (line, result) in reader.lines().enumerate() {
            let line = line + 1;
            let content = result.map_err(|e| Error::Read { e, line })?;

            if content.trim_start().starts_with('#') || content.trim().is_empty() {
                continue;
            }

            let mut fields = content
                .trim()
                .split_ascii_whitespace()
                .collect::<VecDeque<_>>();
            if fields.len() < 2 {
                return Err(Error::InsufficientFields { line });
            }

            let key = fields
                .pop_front()
                .unwrap()
                .parse()
                .map_err(|e| Error::Key { e, line })?;

            let values = fields
                .into_iter()
                .enumerate()
                .map(|(field, value)| {
                    value.parse().map_err(|e| Error::Value {
                        e,
                        field: field + 1,
                        line,
                    })
                })
                .try_collect()?;

            records.insert(key, values);
        }

        Ok(Self { records })
    }
}

impl<Key, Value> Deref for Document<Key, Value> {
    type Target = IndexMap<Key, Vec<Value>>;

    fn deref(&self) -> &Self::Target {
        &self.records
    }
}

impl<Key, Value> IntoIterator for Document<Key, Value> {
    type Item = (Key, Vec<Value>);

    type IntoIter = indexmap::map::IntoIter<Key, Vec<Value>>;

    fn into_iter(self) -> Self::IntoIter {
        self.records.into_iter()
    }
}

#[derive(Error, Debug)]
pub enum Error<KeyError, ValueError>
where
    KeyError: StdError + Debug,
    ValueError: StdError + Debug,
{
    #[error("insufficient fields on line {line}")]
    InsufficientFields { line: usize },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("key error on line {line}: {e}")]
    Key {
        #[source]
        e: KeyError,
        line: usize,
    },

    #[error("read error on line {line}: {e}")]
    Read {
        #[source]
        e: std::io::Error,
        line: usize,
    },

    #[error("value error on line {line}, field {field}: {e}")]
    Value {
        #[source]
        e: ValueError,
        field: usize,
        line: usize,
    },
}
