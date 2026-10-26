//! `buzzdome-quiz`: quiz files from the command line, for scripts and for
//! assistants that run commands (docs/ai-assistants.md).

use quiz_ai::workbench::{self, Check};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
Usage: buzzdome-quiz <command>

  check <folder or file>       Check a quiz folder or quiz file as Buzzdome does.
  pack <folder> <file.quiz>    Make a quiz file of a quiz folder (quiz.json and media/).
  unpack <file.quiz> <folder>  Unpack a quiz file into a new folder to change it.
  schema                       Print the JSON Schema of quiz.json.
  skill <file.zip>             Write the skill package for chat assistants.
  mcp                          Serve the MCP tools on standard input and output.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let path = |arg: &str| PathBuf::from(arg);
    let done = match args.as_slice() {
        ["check", target] => report(workbench::check(&path(target))),
        ["pack", folder, file] => report(workbench::pack(&path(folder), &path(file))),
        ["unpack", file, folder] => report(workbench::unpack(&path(file), &path(folder))),
        ["schema"] => {
            println!("{}", serde_json::to_string_pretty(&quiz_ai::quiz_schema()).expect("JSON values serialize"));
            Ok(())
        }
        ["skill", file] => quiz_ai::skill::write(&path(file)).map_err(|e| e.to_string()),
        ["mcp"] => quiz_ai::mcp::serve(std::io::stdin().lock(), std::io::stdout().lock(), env!("CARGO_PKG_VERSION")).map_err(|e| e.to_string()),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// Prints what the quiz is and what is wrong with it; an error when it is not ready.
fn report(checked: workbench::Result<Check>) -> Result<(), String> {
    let check = checked.map_err(|e| e.to_string())?;
    println!("{}: {} slides, {} questions", check.title, check.slides, check.questions);
    for left_out in &check.left_out {
        println!("left out: {left_out}");
    }
    if check.is_ready() {
        return Ok(());
    }
    Err(check.problems.iter().map(|problem| format!("problem: {problem}")).collect::<Vec<_>>().join("\n"))
}
