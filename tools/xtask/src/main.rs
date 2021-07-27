mod flags;

use anyhow::Result;

fn main() -> Result<()> {
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
