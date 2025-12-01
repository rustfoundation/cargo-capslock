use clap::Parser;

#[derive(Parser)]
#[clap(bin_name = "cargo")]
enum Command {
    Capslock(cargo_capslock::Opt),
}

fn main() -> anyhow::Result<()> {
    let Command::Capslock(opt) = Command::parse();
    opt.main()
}
