use clap::Parser;

#[derive(Parser)]
#[clap(bin_name = "cargo")]
enum Opt {
    Capslock(cargo_capslock::Opt),
}

fn main() -> anyhow::Result<()> {
    let Opt::Capslock(opt) = Opt::parse();
    opt.main()
}
