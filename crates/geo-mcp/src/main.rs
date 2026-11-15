//! `geokernel-mcp`: the MCP server on stdin/stdout, or with `--batch` one
//! tool call per line in and one result per line out.

use geo_mcp::protocol::{self, Server};
use geo_mcp::Session;
use std::io::{self, BufReader};

const USAGE: &str = "usage: geokernel-mcp            MCP server on stdin/stdout
       geokernel-mcp --batch [FILE] one tool call per line (FILE or stdin), one result per line";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let stdout = io::stdout().lock();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        [] => {
            eprintln!(
                "geokernel-mcp: MCP server on stdio (revisions {} and {})",
                protocol::MODERN,
                protocol::LEGACY.join(", ")
            );
            let mut server = Server::new();
            protocol::serve(io::stdin().lock(), stdout, |line| server.handle_input(line))
        }
        ["--batch"] | ["--batch", "-"] => {
            let mut session = Session::new();
            protocol::serve(io::stdin().lock(), stdout, |line| {
                protocol::batch_line(&mut session, line)
            })
        }
        ["--batch", path] => match std::fs::File::open(path) {
            Ok(f) => {
                let mut session = Session::new();
                protocol::serve(BufReader::new(f), stdout, |line| {
                    protocol::batch_line(&mut session, line)
                })
            }
            Err(e) => {
                eprintln!("geokernel-mcp: {path}: {e}");
                std::process::exit(2);
            }
        },
        ["--help"] | ["-h"] => {
            println!("{USAGE}");
            return;
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("geokernel-mcp: {e}");
        std::process::exit(1);
    }
}
