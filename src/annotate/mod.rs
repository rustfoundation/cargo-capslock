use std::{
    fs::File,
    io::{BufReader, Read},
    path::PathBuf,
};

use capslock::{Report, report::Process};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use osv_cache::Cache;

use crate::annotate::matcher::{Affected, Matcher};

pub use self::error::Error;

mod error;
mod matcher;

#[derive(Parser, Debug)]
pub struct Annotate {
    /// Base URL to the OSV vulnerability directory to source advisories from.
    #[arg(
        long,
        default_value = "https://www.googleapis.com/download/storage/v1/b/osv-vulnerabilities/o/crates.io"
    )]
    osv_base_url: String,

    /// OSV cache.
    #[arg(long, default_value_os_t = Cache::default_path().unwrap())]
    osv_cache: PathBuf,

    /// If enabled, the OSV advisory cache will not be updated.
    #[arg(long)]
    skip_osv_cache_update: bool,

    /// `cargo capslock` output to annotate. If omitted, data will be read from
    /// stdin.
    #[arg()]
    path: Option<PathBuf>,
}

impl Annotate {
    #[tracing::instrument(err)]
    pub fn main(self) -> Result<(), Error> {
        // Set up the OSV advisory cache.
        let mut cache = Cache::new(&self.osv_cache)?;
        if !self.skip_osv_cache_update {
            let bar = ProgressBar::new(0).with_style(
                ProgressStyle::with_template("Updating advisory cache: {pos}").unwrap(),
            );
            cache.update(&self.osv_base_url, Some(|n| bar.set_position(n as u64)))?;
            bar.finish_and_clear();
        }

        // Generate the functions we're looking for.
        //
        // FIXME: this needs to be version aware.
        let matcher = Matcher::new(&cache)?;

        // Parse the report.
        let Report { process, children } = self.report()?;

        match_process(&matcher, process);
        for child in children.into_iter() {
            match_process(&matcher, child);
        }

        Ok(())
    }

    fn input_reader(&self) -> Result<Box<dyn Read>, Error> {
        if let Some(path) = &self.path {
            Ok(Box::new(File::open(path).map_err(|e| {
                Error::ReportOpen {
                    e,
                    path: path.display().to_string(),
                }
            })?))
        } else {
            Ok(Box::new(std::io::stdin()))
        }
    }

    #[tracing::instrument(err)]
    fn report(&self) -> Result<Report, Error> {
        serde_json::from_reader(BufReader::new(self.input_reader()?)).map_err(Error::ReportParse)
    }
}

#[tracing::instrument(skip_all)]
fn match_process(matcher: &Matcher, process: Process) {
    for function in process.functions.into_iter() {
        if let Some(affected) = matcher.iter_advisories_for_function(function.display_name()) {
            println!("{}:", function.display_name());
            for Affected { id, package } in affected {
                println!("\tadvisory {id} affecting crate {package}");
            }
            println!();
        }
    }
}
