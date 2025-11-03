
use std::collections::VecDeque;
use std::io::{ErrorKind, Write};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use interprocess::local_socket::{Listener, *};
use interprocess::local_socket::traits::Listener as Listen;
use log::{info, error};
use notify::Watcher;

use std::io::Read;

type AcceptHandler = Box<dyn Fn() -> Option<Event>>;
type ReadHandler = Box<dyn Fn(String) -> Option<Event>>;
type WriteHandler = Box<dyn Fn(String) -> Option<Event>>;
type WatchHandler = Box<dyn Fn(String) -> Option<Event>>;

enum Handler {
    OnAccept(AcceptHandler),
    OnRead(ReadHandler),
    OnWrite(WriteHandler),
    OnDir(WatchHandler),
}

#[derive(Debug)]
pub enum Event {
    //pipes
    Accept,
    Read(String),
    Write(String),

    //wacthers
    Dirmon(String),

    //management
    Quit,
}

impl Event {
    pub fn read(buffer:String) -> Option<Event> {
        Some( Event::Read(buffer) )
    }

    pub fn write(buffer:String) -> Option<Event> {
        Some( Event::Write(buffer) )
    }
}

pub struct Reactor {
    listener: Listener,
    queue: Arc<RwLock<VecDeque<Event>>>,
    handlers: Vec<Handler>,
    stream: std::cell::OnceCell<Stream>,
    watcher: notify::RecommendedWatcher,
}

impl Reactor {
    pub fn new(listener: Listener) -> Self {
        let q = Arc::new(RwLock::new(VecDeque::new()));
        Self {
            listener,
            queue: q.clone(),
            handlers: vec![],
            stream: std::cell::OnceCell::new(),
            watcher: notify::recommended_watcher(move |res:notify::Result<notify::Event>| {
                match res {
                    Ok(event) => {
                        match event.kind {
                            // notify::EventKind::Any => println!("any"),
                            // notify::EventKind::Access(access_kind) => todo!(),
                            // notify::EventKind::Modify(modify_kind) => todo!(),
                            // notify::EventKind::Other => todo!(),
                            notify::EventKind::Create(_) => {
                                let path = event.paths.get(0).unwrap().clone().into_os_string().into_string().unwrap();
                                {
                                    let s = format!("CREATED: {path}");
                                    q.write().unwrap().push_back( Event::Dirmon(s) ); 
                                }
                            },
                            notify::EventKind::Remove(_) => {
                                let path = event.paths.get(0).unwrap().clone().into_os_string().into_string().unwrap();
                                {
                                    let s = format!("REMOVED: {path}");
                                    q.write().unwrap().push_back( Event::Dirmon(s) ); 
                                }
                            },
                            _ => {},
                        }
                    },
                    Err(_err) => {},
                }
            }).unwrap()
        }
    }

    pub fn run(&mut self) {

        if self.handlers.is_empty() {
            return;
        }

        {
            self.queue.write().unwrap().push_back( Event::Accept );
        }
    }

    pub fn demux(&mut self) -> Option<Event> {
        while self.queue.write().unwrap().len() == 0 {
            std::thread::sleep(Duration::from_millis(100));
        }
        self.queue.write().unwrap().pop_front()
    }

    pub fn dispatch(&mut self, event:Event) {

        match event {
            Event::Accept => {
                if let Some(Handler::OnAccept(callback)) = &self.handlers.iter().find( |h| matches!(h, Handler::OnAccept(_)) ) {
                    match self.listener.accept() {
                        Ok(stream) => {
                            self.stream.set( stream ).unwrap();
                            if let Some(ev) = callback() {   
                                self.queue.write().unwrap().push_back( ev ); 
                            }
                        },
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            self.queue.write().unwrap().push_back( Event::Accept );
                        },
                        Err(err) => {
                            error!("{err}"); 
                            self.queue.write().unwrap().push_back( Event::Accept );
                        },
                    }
                }
            },
            Event::Read(mut buffer) => {
                if let Some(Handler::OnRead(callback)) = &self.handlers.iter().find( |h| matches!(h, Handler::OnRead(_)) ) {
                    let ev:Option<Event>;
                    buffer.clear();
                    if self.stream.get().is_none() {
                        return;
                    }
                    match self.stream.get_mut().unwrap().read_to_string(&mut buffer) {
                        Ok(amount) if amount == 0 => {
                            ev = Event::read(buffer);
                        },
                        Ok(_amount) => {
                            ev = callback(buffer);
                        },
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            ev = Event::read(buffer);
                        },
                        Err(err) => {
                            println!("{err}");
                            ev = None;
                        }
                    }
                    if let Some(e) = ev {
                        self.queue.write().unwrap().push_back( e );
                    }
                }
            },
            Event::Write(buffer) => {
                if let Some(Handler::OnWrite(callback)) = &self.handlers.iter().find( |h| matches!(h, Handler::OnWrite(_)) ) {
                    let ev:Option<Event>;
                    if self.stream.get().is_none() {
                        return;
                    }
                    match self.stream.get_mut().unwrap().write(buffer.as_bytes()) {
                        Ok(amount) if amount == 0 => {
                            ev = Event::write(buffer);
                        },
                        Ok(_amount) => {
                            ev = callback(buffer);
                        },
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            ev = Event::write(buffer);
                        },
                        Err(err) => {
                            println!("{err}");
                            ev = None;
                        },
                    }
                    if let Some(e) = ev {
                        self.queue.write().unwrap().push_back( e );
                    }
                }
            },
            Event::Dirmon(path) => {
                if let Some(Handler::OnDir(callback)) = &self.handlers.iter().find(|h| matches!(h, Handler::OnDir(_)) ) {
                    info!("{path}");
                    let opt_ev = callback(path);
                    if let Some(ev) = opt_ev {
                        self.queue.write().unwrap().push_back( ev );
                    }
                }
            },
            Event::Quit => {
                info!("SERVICE: Quitting.");
                std::process::exit(0);
            },
        }
    }

    pub fn on_accept<T>(&mut self, handler:T)
        where T: Fn() -> Option<Event> + 'static
    {
        self.handlers.push( Handler::OnAccept(Box::new(handler)) );
    }

    pub fn on_read<T>(&mut self, handler:T)
        where T: Fn(String) -> Option<Event> + 'static
    {
        self.handlers.push( Handler::OnRead(Box::new(handler)) );
    }

    pub fn on_write<T>(&mut self, handler:T)
        where T: Fn(String) -> Option<Event> + 'static
    {
        self.handlers.push( Handler::OnWrite(Box::new(handler)) );
    }

    pub fn on_dir<T>(&mut self, handler:T)
        where T: Fn(String) -> Option<Event> + 'static
    {
        self.handlers.push( Handler::OnDir(Box::new(handler)) );
    }

    pub fn watch(&mut self, path:PathBuf)
    {
        self.watcher.watch(&path, notify::RecursiveMode::NonRecursive).unwrap();
    }
}

