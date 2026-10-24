// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm launch`: start a game with mods through XXMI's own command line (R7).

use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;

use gmm_adapters::{GameManifest, Registry};
use gmm_core::{confirm_exe, plan_launch, start};
use gmm_db::Db;
use gmm_detect::{SystemEnvironment, detect};
use gmm_xxmi::{evaluate, find_xxmi};

use crate::{AnyError, with_saved_setup};

pub async fn run(
    db: &Db,
    game: &'static GameManifest,
    yes: bool,
    xxmi_dir: Option<PathBuf>,
    folders: Vec<(String, PathBuf)>,
    json: bool,
) -> Result<(), AnyError> {
    let (xxmi_dir, folders) = with_saved_setup(db, xxmi_dir, folders).await?;
    let env = SystemEnvironment;
    let detected = detect(Registry::builtin()?, &env, &folders);
    let xxmi = find_xxmi(&env, xxmi_dir.as_deref()).ok_or("XXMI Launcher not found")?;
    let status = evaluate(game, &detected.installs, Some(&xxmi));
    let mut plan = plan_launch(db, &xxmi, &status, game).await?;

    if !plan.confirmed {
        let exe = plan.command.exe.display().to_string();
        let confirmed = if yes {
            true
        } else if !json && std::io::stdin().is_terminal() {
            println!("GMM is about to run this program for the first time:\n  {exe}");
            println!(
                "It's XXMI Launcher's own exe; it isn't code-signed, so please check the path."
            );
            print!("Run it? [y/N] ");
            std::io::stdout().flush()?;
            let mut answer = String::new();
            std::io::stdin().lock().read_line(&mut answer)?;
            matches!(answer.trim(), "y" | "Y" | "yes" | "Yes")
        } else {
            false
        };
        if !confirmed {
            return Err(format!(
                "not started. To confirm that this is your XXMI Launcher, run again with --yes:\n  {exe}"
            )
            .into());
        }
        confirm_exe(db, &mut plan).await?;
    }

    let pid = start(&plan)?;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "game": game.id(),
                "exe": plan.command.exe,
                "args": plan.command.args,
                "pid": pid,
            })
        );
    } else {
        println!(
            "started XXMI Launcher (process {pid}); it starts {} with mods.",
            game.game.name
        );
    }
    Ok(())
}
