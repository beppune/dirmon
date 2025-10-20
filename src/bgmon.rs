mod config;

use named_pipe::PipeServer;
use notify::{Event, ReadDirectoryChangesWatcher, RecursiveMode, Result, Watcher};
use std::sync::mpsc::Receiver;
use std::sync::mpsc;
use std::fs::File;
use std::fs;
use std::io::Write;
use log::{info, warn, error};
use simplelog::*;

type Rx = Receiver<Result<Event>>;

type WatchDir = (Rx,ReadDirectoryChangesWatcher);

fn main() -> Result<()> {
    // CONFIGURATION
    // Read Config
    let config = config::load(None).unwrap();
    let pipe_name = format!("\\\\.\\pipe\\{}", config.pipe_name.clone());
    //
    // LOGGING
    let _ = CombinedLogger::init(vec![
        SimpleLogger::new(LevelFilter::Info, Config::default()),
        WriteLogger::new(LevelFilter::Info, Config::default(), File::create(config.logfile.as_str()).unwrap()),
    ]);

    // WATCHERS
    let mut watchers: Vec<WatchDir> = vec![];

    for key in config.dirconfs.keys() {
        if !key.is_dir() {
            let ss = key.to_str().unwrap();
            warn!( "Not a directory [{ss}]: skipping." );
            continue;
        }
        let (tx, rx) = mpsc::channel::<Result<Event>>();
        let mut w = notify::recommended_watcher(tx)?;
        w.watch( key, RecursiveMode::NonRecursive )?;
        watchers.push( (rx, w) );
    }

    // PIPES
    let mut pipe = named_pipe::PipeOptions::new(&pipe_name)
        .single().unwrap().wait().unwrap();
    info!("Opened Pipe [{pipe_name}]");

    loop {
        for (rec, _) in &watchers {
            let tryrecv = rec.try_recv();
            if tryrecv.is_err() {
                continue;
            }

            match tryrecv.unwrap() {
                Ok(event) => {
                    match event.kind {
                        notify::EventKind::Create(_) => run_create(event, &mut pipe),
                        notify::EventKind::Remove(_) => run_remove(event, &mut pipe),
                        // notify::EventKind::Any => todo!(),
                        // notify::EventKind::Access(access_kind) => todo!(),
                        // notify::EventKind::Modify(modify_kind) => todo!(),
                        // notify::EventKind::Other => todo!(),
                        _ => {},
                    }
                },
                Err(error) => {
                    error!("{error}");
                },
            }

        }
    }

}

fn run_create(ev:Event, pipe: &mut PipeServer) {
    let info = fs::metadata(ev.paths[0].as_path()).unwrap();
    let ftype = if info.file_type().is_dir() {
        "DIR"
    } else {
        "FILE"
    };
    let ss = format!("Created {}: {}", ftype, ev.paths[0].to_str().unwrap());
    info!("{ss}");
    pipe.write_all(ss.as_bytes()).unwrap();
    pipe.write_all(b"\n").unwrap();
}

fn run_remove(ev:Event, pipe: &mut PipeServer) {
    let ss = format!("Removed: {}", ev.paths[0].to_str().unwrap());
    info!("{ss}");
    pipe.write_all(ss.as_bytes()).unwrap();
    pipe.write_all(b"\n").unwrap();
}


