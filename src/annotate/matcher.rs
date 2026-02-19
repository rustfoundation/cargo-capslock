use std::collections::{BTreeMap, BTreeSet};

use osv_cache::{Cache, osv::schema::Vulnerability};
use serde::Deserialize;
use serde_json::Value;

use crate::annotate::Error;

#[derive(Debug)]
pub struct Matcher {
    // FIXME: cache version metadata so we can match that once we have it in the
    // report.
    functions: BTreeMap<String, BTreeSet<Affected>>,
}

impl Matcher {
    #[tracing::instrument(err)]
    pub fn new(cache: &Cache) -> Result<Self, Error> {
        let mut functions: BTreeMap<String, BTreeSet<Affected>> = BTreeMap::new();

        for result in cache.try_iter_advisories()? {
            let Vulnerability { affected, id, .. } = result?;
            for (index, affected) in affected.into_iter().enumerate() {
                if let Some(package) = affected.package
                    && let Some(specific) = affected.ecosystem_specific
                    && let Value::Object(map) = &specific
                    && !map.is_empty()
                {
                    let spec: RustSpecific = serde_json::from_value(specific).map_err(|_| {
                        Error::EcosystemSpecificNotRust {
                            id: id.clone(),
                            index,
                        }
                    })?;

                    for function in spec.affects.functions.into_iter() {
                        functions.entry(function).or_default().insert(Affected {
                            id: id.clone(),
                            package: package.name.clone(),
                        });
                    }
                }
            }
        }

        dbg!(&functions);

        Ok(Self { functions })
    }

    pub fn iter_advisories_for_function(
        &self,
        function: &str,
    ) -> Option<impl Iterator<Item = &Affected>> {
        self.functions.get(function).map(|affected| affected.iter())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct Affected {
    pub id: String,
    pub package: String,
}

#[derive(Debug, Deserialize)]
struct RustSpecific {
    affects: Affects,
}

#[derive(Debug, Deserialize)]
struct Affects {
    functions: Vec<String>,
}
