// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm launch`: start a game with mods through XXMI's own command line (R7).

use std::io::{BufRead, IsTerminal, Write};

use gmm_adapters::GameManifest;
use gmm_core::{Machine, ModsTarget, SetupHints, confirm_exe, restore_pending, start};
use gmm_db::Db;

use crate::AnyError;

pub async fn run(
    db: &Db,
    game: &'static GameManifest,
    yes: bool,
    hints: SetupHints,
    json: bool,
) -> Result<(), AnyError> {
    let machine = Machine::scan(db, hints).await?;
    let mut plan = machine.launch_plan(db, game).await?;

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

    // In-game toggles that waited for the game to close go back before it starts again.
    if let Ok(target) = ModsTarget::from_machine(&machine, game) {
        let _ = restore_pending(db, &target.ws, target.mods, game).await;
    }
    let pid = start(db, game, &plan).await?;
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
        let process = pid.map(|p| format!(" (process {p})")).unwrap_or_default();
        println!(
            "started XXMI Launcher{process}; it starts {} with mods.",
            game.game.name
        );
    }
    Ok(())
}
