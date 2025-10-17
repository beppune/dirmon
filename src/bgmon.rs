use named_pipe;
use notify::{Event, RecursiveMode, Result, Watcher};
use std::{path::Path, sync::mpsc};
use std::fs;
use std::io::Write;

const PIPE_NAME:&str = "\\\\.\\pipe\\DirMon";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 2 {
        println!("bgmon <directory>");
        std::process::exit(1);
    }

    // Watcher
    let (tx, rx) = mpsc::channel::<Result<Event>>();

    let mut watcher = notify::recommended_watcher(tx)?;

    let path = Path::new(args[1].as_str());
    if !fs::metadata(path).unwrap().is_dir() {
        println!("Path must be a dir");
        std::process::exit(2);
    }

    watcher.watch(path, RecursiveMode::NonRecursive)?;

    for res in rx {
        match res {
            Ok(event) => {
                match event.kind {
                    notify::EventKind::Create(_) => {
                        let info = fs::metadata(event.paths[0].as_path())?;
                        let ftype = if info.file_type().is_dir() {
                            "DIR"
                        } else {
                            "FILE"
                        };
                        println!("Created {}: {}", ftype, event.paths[0].to_str().unwrap());
                        // let ss = format!("Created {}: {}", ftype, event.paths[0].to_str().unwrap());
                        // client.write_all(ss.as_bytes()).unwrap();
                    },
                    notify::EventKind::Remove(_) => {
                        println!("Removed: {}",  event.paths[0].to_str().unwrap());
                    },
                    _ => {},

                }
            },
            Err(error) => println!("{:?}", error),
        }
    }

    Ok(())

}
