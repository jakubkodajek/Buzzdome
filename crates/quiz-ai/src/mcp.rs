//! An MCP server (Model Context Protocol, modelcontextprotocol.io) over
//! standard input and output, for desktop assistants such as Claude
//! Desktop: its tools let the assistant read the quiz format, look at the
//! quizmaster's pictures, and write, check and pack quizzes on this
//! computer (`workbench`), only in the folders the quizmaster lets it work
//! in (`Roots`). The app serves it when started with `--mcp`, and so does
//! `buzzdome-quiz mcp`, each with `--root <folder>` for each such folder.
//!
//! Messages are JSON-RPC 2.0, one per line. The server answers requests in
//! order; it sends nothing of its own, so it needs no other thread.

use crate::roots::Roots;
use crate::workbench::{self, Check, WorkError};
use base64::Engine;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};

/// Protocol versions the server speaks, newest first. It answers a client
/// with the version it asks for when it is one of these, else the newest.
pub const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
/// The server's name, as assistants show it.
pub const SERVER_NAME: &str = "buzzdome";
/// Longest message read, in bytes: a whole quiz with long texts stays far below it.
const MAX_MESSAGE_BYTES: u64 = 32 << 20;

/// What the assistant is told when it connects.
fn instructions(roots: &Roots) -> String {
    format!(
        "Buzzdome is a desktop quiz show played with wireless buzzers. \
To make a quiz: read the format with quiz_format, find the quizmaster's media with list_media_files \
(look_at_picture shows you a picture), copy the ones the quiz uses into a new quiz folder with add_media, \
write the quiz with write_quiz and fix the problems it reports, then save_quiz_file makes the .quiz file \
the quizmaster opens in Buzzdome (File > Open). Use absolute paths. You may work only in these folders: {}. \
Ask the quizmaster where to work when they have not said.",
        roots.describe()
    )
}

/// JSON-RPC error codes.
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// Serves the protocol on `input` and `output` until the input ends, with
/// the tools working in `roots`. `version` is the app's, which the
/// assistant is told.
pub fn serve(mut input: impl BufRead, mut output: impl Write, version: &str, roots: &Roots) -> io::Result<()> {
    loop {
        let mut line = Vec::new();
        if Read::take(&mut input, MAX_MESSAGE_BYTES + 1).read_until(b'\n', &mut line)? == 0 {
            return Ok(());
        }
        let reply = if line.len() as u64 > MAX_MESSAGE_BYTES {
            skip_line(&mut input)?;
            Some(failure(Value::Null, INVALID_REQUEST, "the message is too long"))
        } else {
            match String::from_utf8(line) {
                Ok(text) if text.trim().is_empty() => None,
                Ok(text) => answer(text.trim(), version, roots),
                Err(_) => Some(failure(Value::Null, PARSE_ERROR, "the message is not UTF-8")),
            }
        };
        if let Some(reply) = reply {
            write_message(&mut output, &reply)?;
        }
    }
}

/// Reads past the rest of the line without keeping it.
fn skip_line(input: &mut impl BufRead) -> io::Result<()> {
    loop {
        let buffer = input.fill_buf()?;
        if buffer.is_empty() {
            return Ok(());
        }
        let (used, ended) = match buffer.iter().position(|&b| b == b'\n') {
            Some(end) => (end + 1, true),
            None => (buffer.len(), false),
        };
        input.consume(used);
        if ended {
            return Ok(());
        }
    }
}

fn write_message(output: &mut impl Write, message: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *output, message)?;
    output.write_all(b"\n")?;
    output.flush()
}

/// The reply to one message; none to a notification.
pub fn answer(message: &str, version: &str, roots: &Roots) -> Option<Value> {
    let Ok(message) = serde_json::from_str::<Value>(message) else {
        return Some(failure(Value::Null, PARSE_ERROR, "the message is not JSON"));
    };
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        // A response to a request of ours (we send none), or no message at all.
        return message.get("id").is_none().then(|| failure(Value::Null, INVALID_REQUEST, "not a JSON-RPC request"));
    };
    // A notification (`notifications/initialized`, `notifications/cancelled`) needs no reply.
    let id = message.get("id")?.clone();
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => Ok(initialize(&params, version, roots)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": TOOLS.iter().map(Tool::describe).collect::<Vec<_>>() })),
        "tools/call" => call(&params, roots),
        _ => Err((METHOD_NOT_FOUND, format!("no method {method}"))),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, text)) => failure(id, code, &text),
    })
}

fn failure(id: Value, code: i64, text: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": text } })
}

fn initialize(params: &Value, version: &str, roots: &Roots) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let protocol = asked.filter(|asked| PROTOCOL_VERSIONS.contains(asked)).unwrap_or(PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": protocol,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": SERVER_NAME, "title": "Buzzdome", "version": version },
        "instructions": instructions(roots),
    })
}

fn call(params: &Value, roots: &Roots) -> Result<Value, (i64, String)> {
    let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
    let Some(tool) = TOOLS.iter().find(|tool| tool.name == name) else {
        return Err((INVALID_PARAMS, format!("no tool {name}")));
    };
    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    // A tool's failure is the assistant's to read and act on, not a protocol error.
    Ok(match (tool.run)(&Arguments { values: arguments, roots }) {
        Ok(content) => json!({ "content": content, "isError": false }),
        Err(text) => json!({ "content": [text_block(&text)], "isError": true }),
    })
}

/// A tool's arguments, read with the error the assistant needs to fix them.
/// Every path is one inside the roots (`Roots::resolve`).
struct Arguments<'a> {
    values: Value,
    roots: &'a Roots,
}

impl Arguments<'_> {
    fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    fn path(&self, name: &str) -> Result<PathBuf, String> {
        let path = self.get(name).and_then(Value::as_str).ok_or_else(|| format!("{name} is missing: give an absolute path"))?;
        self.roots.resolve(Path::new(path))
    }

    fn paths(&self, name: &str) -> Result<Vec<PathBuf>, String> {
        let paths = self.get(name).and_then(Value::as_array).ok_or_else(|| format!("{name} is missing: give a list of absolute paths"))?;
        let resolve = |value: &Value| match value.as_str() {
            Some(path) => self.roots.resolve(Path::new(path)),
            None => Err(format!("{name} lists something that is not a path")),
        };
        paths.iter().map(resolve).collect()
    }
}

fn text_block(text: &str) -> Value {
    json!({ "type": "text", "text": text })
}

fn json_block(value: &impl Serialize) -> Vec<Value> {
    vec![text_block(&serde_json::to_string_pretty(value).expect("tool results serialize: plain data"))]
}

fn worded(error: WorkError) -> String {
    error.to_string()
}

/// A check, with what to do next.
fn checked(check: &Check, ready: &str) -> Vec<Value> {
    let next = if check.is_ready() { ready } else { "Fix the problems and write the quiz again." };
    let mut content = json_block(check);
    content.push(text_block(next));
    content
}

struct Tool {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    /// The tool only reads.
    read_only: bool,
    input: fn() -> Value,
    run: fn(&Arguments<'_>) -> Result<Vec<Value>, String>,
}

impl Tool {
    fn describe(&self) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": (self.input)(),
            "annotations": { "title": self.title, "readOnlyHint": self.read_only, "openWorldHint": false },
        })
    }
}

fn object(properties: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": properties, "required": required })
}

fn path_property(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

const TOOLS: [Tool; 9] = [
    Tool {
        name: "quiz_format",
        title: "Read the quiz format",
        description: "The Buzzdome quiz file format: a guide to every field and the JSON Schema of quiz.json. Read it before writing a quiz.",
        read_only: true,
        input: || object(json!({}), &[]),
        run: |_| Ok(vec![text_block(crate::FORMAT_GUIDE), text_block(&serde_json::to_string_pretty(&crate::quiz_schema()).expect("JSON serializes"))]),
    },
    Tool {
        name: "list_media_files",
        title: "List media files",
        description: "Lists the pictures, sounds and videos Buzzdome plays in a folder and the folders in it, with their sizes.",
        read_only: true,
        input: || object(json!({ "folder": path_property("Absolute path of the folder to look in.") }), &["folder"]),
        run: |args| workbench::list_media(&args.path("folder")?).map(|listing| json_block(&listing)).map_err(worded),
    },
    Tool {
        name: "look_at_picture",
        title: "Look at a picture",
        description: "Shows you a PNG, JPEG, GIF or WebP picture of up to 3 MB, to write questions about it or choose pictures for slides.",
        read_only: true,
        input: || object(json!({ "path": path_property("Absolute path of the picture.") }), &["path"]),
        run: |args| {
            let picture = workbench::picture(&args.path("path")?).map_err(worded)?;
            let data = base64::engine::general_purpose::STANDARD.encode(&picture.bytes);
            Ok(vec![json!({ "type": "image", "data": data, "mimeType": picture.mime_type })])
        },
    },
    Tool {
        name: "add_media",
        title: "Add media to a quiz",
        description: "Copies pictures, sounds and videos into a quiz folder (made if missing; a folder with other files is refused) \
                      and returns the name each has in the quiz, media/<file name>, to use in quiz.json. A name that does not work on \
                      every system is changed, and a different file of a name already used gets a number.",
        read_only: false,
        input: || {
            object(
                json!({
                    "folder": path_property("Absolute path of the quiz folder."),
                    "files": { "type": "array", "items": { "type": "string" }, "description": "Absolute paths of the files to copy." },
                }),
                &["folder", "files"],
            )
        },
        run: |args| workbench::add_media(&args.path("folder")?, &args.paths("files")?).map(|added| json_block(&added)).map_err(worded),
    },
    Tool {
        name: "write_quiz",
        title: "Write a quiz",
        description: "Writes a whole quiz as the quiz.json of a quiz folder (made if missing; a folder with other files than a quiz \
                      is refused), replacing the one there, and checks it: \
                      it lists what keeps the quiz from being played, such as a media file not added yet, and what did not fit the \
                      format and was left out.",
        read_only: false,
        input: write_quiz_input,
        run: |args| {
            let quiz = args.get("quiz").ok_or("quiz is missing: give the whole quiz as JSON")?;
            // A quiz sent as a JSON text reads as well as one sent as an object.
            let json = quiz.as_str().map_or_else(|| quiz.to_string(), str::to_owned);
            let check = workbench::write_quiz(&args.path("folder")?, &json).map_err(worded)?;
            Ok(checked(&check, "The quiz is ready: save_quiz_file makes the .quiz file."))
        },
    },
    Tool {
        name: "read_quiz",
        title: "Read a quiz",
        description: "Returns the quiz.json of a quiz folder, to change a quiz written before (unpack_quiz_file makes a folder of a .quiz file).",
        read_only: true,
        input: || object(json!({ "folder": path_property("Absolute path of the quiz folder.") }), &["folder"]),
        run: |args| workbench::read_quiz(&args.path("folder")?).map(|json| vec![text_block(&json)]).map_err(worded),
    },
    Tool {
        name: "check_quiz",
        title: "Check a quiz",
        description: "Checks a quiz folder or a .quiz file as Buzzdome does: lists what keeps it from being played.",
        read_only: true,
        input: || object(json!({ "path": path_property("Absolute path of the quiz folder or .quiz file.") }), &["path"]),
        run: |args| workbench::check(&args.path("path")?).map(|check| checked(&check, "The quiz is ready to play.")).map_err(worded),
    },
    Tool {
        name: "save_quiz_file",
        title: "Save the quiz file",
        description: "Makes the .quiz file of a quiz folder, with the media its slides use. A file of that name is replaced only with \
                      replace: true, which you give only when the quizmaster asked to replace that file. A quiz with problems is not \
                      saved; they are listed. The quizmaster opens the file in Buzzdome with File > Open.",
        read_only: false,
        input: || {
            object(
                json!({
                    "folder": path_property("Absolute path of the quiz folder."),
                    "file": path_property("Absolute path of the .quiz file to write."),
                    "replace": { "type": "boolean", "description": "Replace a file of that name; only when the quizmaster asked to.", "default": false },
                }),
                &["folder", "file"],
            )
        },
        run: |args| {
            let file = args.path("file")?;
            let replace = args.get("replace").and_then(Value::as_bool).unwrap_or(false);
            let check = workbench::pack(&args.path("folder")?, &file, replace).map_err(worded)?;
            let mut content = json_block(&check);
            content.push(text_block(&format!("Saved {}.", file.display())));
            Ok(content)
        },
    },
    Tool {
        name: "unpack_quiz_file",
        title: "Unpack a quiz file",
        description: "Unpacks a .quiz file into a new quiz folder to change it; the folder must not exist yet.",
        read_only: false,
        input: || {
            object(
                json!({
                    "file": path_property("Absolute path of the .quiz file."),
                    "folder": path_property("Absolute path of the new quiz folder."),
                }),
                &["file", "folder"],
            )
        },
        run: |args| workbench::unpack(&args.path("file")?, &args.path("folder")?).map(|check| json_block(&check)).map_err(worded),
    },
];

/// `write_quiz` takes the quiz as the format's schema describes it.
fn write_quiz_input() -> Value {
    let mut quiz = crate::quiz_schema();
    let defs = quiz.as_object_mut().and_then(|schema| schema.remove("$defs")).unwrap_or_else(|| json!({}));
    if let Some(schema) = quiz.as_object_mut() {
        schema.remove("$schema");
    }
    let mut defs: Map<String, Value> = serde_json::from_value(defs).unwrap_or_default();
    defs.insert("Quiz".into(), quiz);
    json!({
        "type": "object",
        "properties": {
            "folder": path_property("Absolute path of the quiz folder."),
            "quiz": { "$ref": "#/$defs/Quiz", "description": "The whole quiz, as quiz.json holds it." },
        },
        "required": ["folder", "quiz"],
        "$defs": defs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench::tests::{quiz_with_picture, scratch, PNG};

    /// The replies to `messages`, sent one per line, as a client reads them,
    /// from a server that works in `roots`.
    fn session(roots: &[&Path], messages: &[Value]) -> Vec<Value> {
        let roots = Roots::new(roots.iter().map(|root| crate::roots::allow(root, &[]).unwrap()).collect());
        let input: String = messages.iter().map(|m| format!("{m}\n")).collect();
        let mut output = Vec::new();
        serve(input.as_bytes(), &mut output, "1.2.3", &roots).unwrap();
        String::from_utf8(output).unwrap().lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    fn request(id: u32, method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
    }

    fn call_tool(id: u32, name: &str, arguments: Value) -> Value {
        request(id, "tools/call", json!({ "name": name, "arguments": arguments }))
    }

    fn text(reply: &Value) -> String {
        reply["result"]["content"].as_array().unwrap().iter().filter_map(|c| c["text"].as_str()).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn a_client_connects_in_its_protocol_version_and_lists_the_tools() {
        let dir = scratch("mcp-hello");
        let replies = session(&[&dir], &[
            request(1, "initialize", json!({ "protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } })),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            request(2, "tools/list", json!({})),
            request(3, "ping", json!({})),
            request(4, "initialize", json!({ "protocolVersion": "2099-01-01" })),
        ]);
        assert_eq!(replies.len(), 4, "a notification gets no reply");
        assert_eq!(replies[0]["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(replies[0]["result"]["serverInfo"], json!({ "name": "buzzdome", "title": "Buzzdome", "version": "1.2.3" }));
        let instructions = replies[0]["result"]["instructions"].as_str().unwrap();
        assert!(instructions.contains(&Roots::new(vec![std::fs::canonicalize(&dir).unwrap()]).describe()), "{instructions}");
        let tools: Vec<&str> = replies[1]["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(
            tools,
            ["quiz_format", "list_media_files", "look_at_picture", "add_media", "write_quiz", "read_quiz", "check_quiz", "save_quiz_file", "unpack_quiz_file"]
        );
        let write = &replies[1]["result"]["tools"][4]["inputSchema"];
        assert_eq!(write["$defs"]["Quiz"]["required"], json!(["schemaVersion", "title"]));
        assert!(write["$defs"]["Slide"].is_object());
        assert_eq!(replies[2], json!({ "jsonrpc": "2.0", "id": 3, "result": {} }));
        assert_eq!(replies[3]["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
    }

    #[test]
    fn an_assistant_makes_a_quiz_file_with_a_picture() {
        let dir = scratch("mcp-flow");
        let photo = dir.join("brno.png");
        std::fs::write(&photo, PNG).unwrap();
        let folder = dir.join("quiz");
        let file = dir.join("Capitals.quiz");
        let quiz: Value = serde_json::from_str(&quiz_with_picture("media/brno.png")).unwrap();
        let replies = session(&[&dir], &[
            call_tool(1, "write_quiz", json!({ "folder": folder, "quiz": quiz })),
            call_tool(2, "save_quiz_file", json!({ "folder": folder, "file": file })),
            call_tool(3, "add_media", json!({ "folder": folder, "files": [photo] })),
            call_tool(4, "check_quiz", json!({ "path": folder })),
            call_tool(5, "save_quiz_file", json!({ "folder": folder, "file": file })),
            call_tool(6, "look_at_picture", json!({ "path": photo })),
            call_tool(7, "list_media_files", json!({ "folder": dir })),
            call_tool(8, "save_quiz_file", json!({ "folder": folder, "file": file })),
            call_tool(9, "save_quiz_file", json!({ "folder": folder, "file": file, "replace": true })),
            call_tool(10, "read_quiz", json!({ "folder": folder })),
        ]);
        assert_eq!(replies[0]["result"]["isError"], false);
        assert!(text(&replies[0]).contains("media/brno.png: the file is missing"), "{}", text(&replies[0]));
        assert_eq!(replies[1]["result"]["isError"], true);
        assert!(text(&replies[2]).contains(r#""reference": "media/brno.png""#));
        assert!(text(&replies[3]).contains(r#""problems": []"#));
        assert_eq!(replies[4]["result"]["isError"], false, "{}", text(&replies[4]));
        assert!(crate::workbench::check(&file).unwrap().is_ready());
        let data = base64::engine::general_purpose::STANDARD.encode(PNG);
        assert_eq!(replies[5]["result"]["content"][0], json!({ "type": "image", "data": data, "mimeType": "image/png" }));
        assert!(text(&replies[6]).contains("brno.png"));
        assert!(text(&replies[7]).contains("exists already"), "{}", text(&replies[7]));
        assert_eq!(replies[8]["result"]["isError"], false, "{}", text(&replies[8]));
        assert!(text(&replies[9]).contains("Capital of Moravia?"));
    }

    #[test]
    fn nothing_outside_the_roots_is_read_or_written() {
        let dir = scratch("mcp-roots");
        let (quizzes, private) = (dir.join("quizzes"), dir.join("private"));
        std::fs::create_dir_all(&quizzes).unwrap();
        std::fs::create_dir_all(&private).unwrap();
        std::fs::write(private.join("photo.png"), PNG).unwrap();
        std::fs::write(private.join(quiz_file::QUIZ_JSON), quiz_with_picture("media/x.png")).unwrap();
        let quiz: Value = serde_json::from_str(&quiz_with_picture("media/x.png")).unwrap();
        let replies = session(&[&quizzes], &[
            call_tool(1, "write_quiz", json!({ "folder": private, "quiz": quiz })),
            call_tool(2, "look_at_picture", json!({ "path": private.join("photo.png") })),
            call_tool(3, "list_media_files", json!({ "folder": dir })),
            call_tool(4, "add_media", json!({ "folder": quizzes.join("q"), "files": [private.join("photo.png")] })),
            call_tool(5, "read_quiz", json!({ "folder": private })),
            call_tool(6, "save_quiz_file", json!({ "folder": quizzes.join("q"), "file": private.join("q.quiz") })),
            call_tool(7, "write_quiz", json!({ "folder": quizzes.join("../private"), "quiz": quiz })),
        ]);
        for reply in &replies {
            assert_eq!(reply["result"]["isError"], true, "{reply}");
        }
        assert!(text(&replies[0]).contains("outside the folders"), "{}", text(&replies[0]));
        assert!(text(&replies[6]).contains(".."));
        assert!(!quizzes.join("q").exists() && !private.join("q.quiz").exists());
        assert_eq!(std::fs::read_to_string(private.join(quiz_file::QUIZ_JSON)).unwrap(), quiz_with_picture("media/x.png"));
    }

    #[cfg(unix)]
    #[test]
    fn a_quiz_json_linked_out_of_the_roots_is_not_read() {
        let dir = scratch("mcp-link");
        let quizzes = dir.join("quizzes");
        std::fs::create_dir_all(quizzes.join("q")).unwrap();
        std::fs::write(dir.join("secret.json"), "{}").unwrap();
        std::os::unix::fs::symlink(dir.join("secret.json"), quizzes.join("q").join(quiz_file::QUIZ_JSON)).unwrap();
        let replies = session(&[&quizzes], &[call_tool(1, "read_quiz", json!({ "folder": quizzes.join("q") }))]);
        assert_eq!(replies[0]["result"]["isError"], true);
        assert!(text(&replies[0]).contains("leads out of its quiz folder"), "{}", text(&replies[0]));
    }

    #[test]
    fn a_relative_path_or_a_missing_argument_is_the_assistants_to_fix() {
        let dir = scratch("mcp-arguments");
        let folder = dir.join("q");
        let replies = session(&[&dir], &[
            call_tool(1, "check_quiz", json!({ "path": "quiz" })),
            call_tool(2, "add_media", json!({ "folder": folder })),
            call_tool(3, "write_quiz", json!({ "folder": folder })),
        ]);
        assert_eq!(text(&replies[0]), "quiz is not an absolute path");
        assert_eq!(text(&replies[1]), "files is missing: give a list of absolute paths");
        assert_eq!(text(&replies[2]), "quiz is missing: give the whole quiz as JSON");
        assert!(replies.iter().all(|reply| reply["result"]["isError"] == true));
    }

    #[test]
    fn what_is_not_a_request_gets_a_protocol_error() {
        let mut input = b"not json\n\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"resources/list\"}\n".to_vec();
        input.extend(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"format\"}}\n");
        input.extend(vec![b'x'; MAX_MESSAGE_BYTES as usize + 10]);
        input.extend(b"\n{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"ping\"}\n");
        let mut output = Vec::new();
        serve(&input[..], &mut output, "1", &Roots::default()).unwrap();
        let replies: Vec<Value> = String::from_utf8(output).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        let codes: Vec<&Value> = replies.iter().map(|r| &r["error"]["code"]).collect();
        assert_eq!(codes, [&json!(PARSE_ERROR), &json!(METHOD_NOT_FOUND), &json!(INVALID_PARAMS), &json!(INVALID_REQUEST), &Value::Null]);
        assert_eq!(replies[4]["id"], 3, "the server reads on after a message too long");
    }
}
