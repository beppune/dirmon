mod config;
mod reactor;

use interprocess::local_socket::{GenericNamespaced, ListenerNonblockingMode, ListenerOptions, ToNsName};
use notify::{Event, ReadDirectoryChangesWatcher, RecursiveMode, Result, Watcher};
use std::cell::RefCell;
use std::ffi::OsStr;
use std::rc::Rc;
use std::{path::PathBuf, sync::mpsc::Receiver};
use std::sync::mpsc;
use std::fs::File;
use std::fs;
use std::io::Write;
use log::{info, warn, error};
use simplelog::*;
use std::collections::HashMap;
use config::FsEvent;
use reactor::{Reactor, Event as REvent};

type Rx = Receiver<Result<Event>>;

type WatchDir = (Rx,ReadDirectoryChangesWatcher);

fn main() {

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

    // Override options
    if args.contains(["-n", "--nopipe"]) {
        config.nopipe = true;
    }

    // LOGGING
    let _ = CombinedLogger::init(vec![
        SimpleLogger::new(LevelFilter::Info, Config::default()),
        WriteLogger::new(LevelFilter::Info, Config::default(), File::create(config.logfile.as_str()).unwrap()),
    ]);


    // PIPE
    //
    let listener = ListenerOptions::new()
        .nonblocking(ListenerNonblockingMode::Stream)
        .name( OsStr::new( config.pipe_name.as_str() ).to_ns_name::<GenericNamespaced>().unwrap() )
        .create_sync().unwrap();
    let mut reactor = Reactor::new(listener);

    for key in config.dirconfs.keys() {
        if !key.is_dir() {
            let ss = key.to_str().unwrap();
            warn!( "Not a directory [{ss}]: skipping." );
            continue;
        }

        reactor.watch(key.to_path_buf(), |p|{
            info!("{p}");
            None
        });

    }

    let proto = Rc::new(RefCell::new(String::new()));

    {
        reactor.accept(move |stream| {
            REvent::read(stream, String::new())
        });
    }

    {
        let proto = Rc::clone(&proto);
        reactor.read(move |stream, mut buffer| {
            REvent::write(stream, buffer)
        });
    }

    {
        let proto = Rc::clone(&proto);
        reactor.write(move |stream, mut buffer| {
            REvent::read(stream, buffer)
        });
    }

    reactor.run();
    info!("Watching directories.");

    while let Some(event) = reactor.demux() {
        reactor.dispatch(event);
    }
    info!("Queue ended!");
}

