use cargo_capslock::Opt;
use clap::Parser;

fn main() -> anyhow::Result<()> {
    Opt::parse().main()
}
