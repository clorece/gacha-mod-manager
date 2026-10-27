// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gmm log` (the local error log) and `gmm report-bug` (a link to a pre-filled GitHub
//! issue). Nothing is sent anywhere: the link is printed, and the user opens and submits it.

use gmm_adapters::Registry;
use gmm_core::{AppDirs, Machine, SetupHints};
use gmm_db::Db;

use crate::AnyError;

/// `gmm log`: the newest entries, or `--clear`.
pub fn log(clear: bool, last: usize, json: bool) -> Result<(), AnyError> {
    let log = AppDirs::system()?.error_log()?;
    if clear {
        let n = log.clear();
        if json {
            println!("{}", serde_json::json!({ "cleared": n }));
        } else {
            println!("deleted {n} log entr{}", if n == 1 { "y" } else { "ies" });
        }
        return Ok(());
    }
    let entries = log.entries(last);
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else if entries.is_empty() {
        println!("no errors logged");
    } else {
        for e in &entries {
            println!("{}  {:<8} {}", e.time, e.source, e.message);
        }
        println!("\nlog folder: {}", log.folder()?.display());
    }
    Ok(())
}

/// `gmm report-bug`: prints the pre-filled issue's address and what it says.
pub async fn report_bug(
    db: &Db,
    title: &str,
    text: &str,
    hints: SetupHints,
    json: bool,
) -> Result<(), AnyError> {
    let log = AppDirs::system()?.error_log()?;
    let machine = Machine::scan(db, hints).await?;
    let statuses: Vec<_> = Registry::builtin()?
        .games()
        .iter()
        .map(|g| machine.status(g))
        .collect();
    let details = log.redact(&gmm_core::system_details(&statuses));
    let newest = log.newest_file();
    let report = gmm_core::bug_report(
        title,
        &log.redact(text),
        &details,
        newest.as_ref().map(|f| f.name.as_str()),
    );
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    println!("{}\n\n{}\n", report.title, report.body);
    if report.shortened {
        println!("(The description was shortened to fit the link; paste the rest on GitHub.)\n");
    }
    println!(
        "Open this link in your browser to check and submit the report:\n{}",
        report.url
    );
    Ok(())
}
