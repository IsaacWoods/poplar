mod flags;

use eyre::Result;

fn main() -> Result<()> {
    color_eyre::install()?;
    let flags = flags::Task::from_env()?;
    match flags.subcommand {
        flags::TaskCmd::Help(_) => {
            println!("{}", flags::Task::HELP);
            Ok(())
        }

        flags::TaskCmd::Dist(dist) => {
            println!("Doing dist");
            Ok(())
        }
    }
}
