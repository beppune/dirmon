mod config;

use named_pipe::{ConnectingServer, PipeServer};
use notify::{Event, ReadDirectoryChangesWatcher, RecursiveMode, Result, Watcher};
use std::{path::PathBuf, sync::mpsc::Receiver};
use std::sync::mpsc;
use std::fs::File;
use std::fs;
use std::io::Write;
use log::{info, warn, error};
use simplelog::*;
use std::collections::HashMap;
use config::FsEvent;

type Rx = Receiver<Result<Event>>;

type WatchDir = (Rx,ReadDirectoryChangesWatcher);

fn main() -> Result<()> {

    // ARGS
    let mut args = pico_args::Arguments::from_env();
    if args.contains(["-h", "--help"]) {
        println!(r#"
     bgmon [-c <config file> (default: .\\dirmon.toml)]

     Other options:
         -h, --help:     Show this help
         -n, --nopipe:   No pipe will be used
"#);
        std::process::exit(0);
    }
    let config_file:Option<String> = args.value_from_str("-c").ok();

    // CONFIGURATION
    // Read Config
    let mut config = config::load( config_file ).unwrap();
    let pipe_name = format!("\\\\.\\pipe\\{}", config.pipe_name.clone());

    // Override options
    if args.contains(["-n", "--nopipe"]) {
        config.nopipe = true;
    }

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

    // PIPE
    let pipe_server:ConnectingServer;
    let mut pipe:Option<PipeServer> = None;

    if !config.nopipe {
        pipe_server = named_pipe::PipeOptions::new(&pipe_name)
            .single().unwrap() ;
        pipe = Some( pipe_server.wait().unwrap() );
        info!("Opened Pipe [{pipe_name}]");
    } else {
        info!("Pipe is disabled [--nopipe, see help]")
    }

    info!("Watching directories.");
    loop {
        for (rec, _) in &watchers {
            let tryrecv = rec.try_recv();
            if tryrecv.is_err() {
                continue;
            }

            match tryrecv.unwrap() {
                Ok(event) => {
                    match event.kind {
                        notify::EventKind::Create(_) => run_create(event, pipe.as_mut()),
                        notify::EventKind::Remove(_) => run_remove(&config.dirconfs, event, pipe.as_mut()),
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

fn run_create(ev:Event, pipe: Option<&mut PipeServer>) {
    let info = fs::metadata(ev.paths[0].as_path()).unwrap();
    let ftype = if info.file_type().is_dir() {
        "DIR"
    } else {
        "FILE"
    };
    let ss = format!("Created {}: {}", ftype, ev.paths[0].to_str().unwrap());
    info!("{ss}");

    if let Some(p) = pipe {
        p.write_all(ss.as_bytes()).unwrap();
        p.write_all(b"\n").unwrap();
    }
}

fn run_remove(dirconfs:&HashMap<PathBuf,HashMap<FsEvent,String>>, ev:Event, pipe: Option<&mut PipeServer>) {
    let path = &ev.paths[0];
    let parent = path.parent().unwrap();

    dbg!( dirconfs.get(parent).unwrap().get(&FsEvent::Delete) );

    let ss = format!("Removed: {}", path.to_str().unwrap());
    info!("{ss}");
    
    if let Some(p) = pipe {
        p.write_all(ss.as_bytes()).unwrap();
        p.write_all(b"\n").unwrap();
    }
}


