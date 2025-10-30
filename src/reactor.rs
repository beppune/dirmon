
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use interprocess::local_socket::{Listener, *};
use interprocess::local_socket::traits::Listener as Listen;
use notify::Watcher;

use std::io::Read;

type AcceptHandler = Box<dyn Fn(Stream) -> Option<Event>>;
type ReadHandler = Box<dyn Fn(Stream,String) -> Option<Event>>;
type WriteHandler = Box<dyn Fn(Stream,String) -> Option<Event>>;
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
    Accept(Stream),
    Read(Stream,String),
    Write(Stream,String),

    //wacthers
    Dirmon(String),
}

impl Event {
    pub fn read(stream:Stream, buffer:String) -> Option<Event> {
        Some( Event::Read(stream, buffer) )
    }

    pub fn write(stream:Stream, buffer:String) -> Option<Event> {
        Some( Event::Write(stream, buffer) )
    }

    pub fn watch(path:String) -> Option<Event> {
        Some( Event::Dirmon(path) )
    }
}

pub struct Reactor {
    listener: Listener,
    queue: Arc<RwLock<VecDeque<Event>>>,
    handlers: Vec<Handler>,
    watcher: notify::RecommendedWatcher,
}

impl Reactor {
    pub fn new(listener: Listener) -> Self {
        let q = Arc::new(RwLock::new(VecDeque::new()));
        Self {
            listener,
            queue: q.clone(),
            handlers: vec![],
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
                                    q.write().unwrap().push_back( Event::Dirmon(path) ); 
                                }
                            },
                            notify::EventKind::Remove(_) => {
                                let path = event.paths.get(0).unwrap().clone().into_os_string().into_string().unwrap();
                                {
                                    q.write().unwrap().push_back( Event::Dirmon(path) ); 
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

        loop {
            match self.listener.accept() {
                Ok(stream) => { 
                    {
                        self.queue.write().unwrap()
                            .push_back( Event::Accept(stream) );
                        }
                    break;
                },
                Err(err) if err.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                },
                Err(err) => {
                    println!("{err}");
                    break;
                },
            }
        }
    }

    pub fn demux(&mut self) -> Option<Event> {
        self.queue.write().unwrap().pop_front()
    }

    pub fn dispatch(&mut self, event:Event) {

        match event {
            Event::Accept(stream) => {
                if let Some(Handler::OnAccept(callback)) = &self.handlers.iter().find( |h| matches!(h, Handler::OnAccept(_)) ) {
                    if let Some(ev) = callback(stream) {   
                        self.queue.write().unwrap().push_back( ev ); 
                    }
                }
            },
            Event::Read(mut stream, mut buffer) => {
                if let Some(Handler::OnRead(callback)) = &self.handlers.iter().find( |h| matches!(h, Handler::OnRead(_)) ) {
                    let ev:Option<Event>;
                    buffer.clear();
                    match stream.read_to_string(&mut buffer) {
                        Ok(amount) if amount == 0 => {
                            ev = Event::read(stream, buffer);
                        },
                        Ok(_amount) => {
                            ev = callback(stream, buffer);
                        },
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            ev = Event::read(stream, buffer);
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
            Event::Write(mut stream, buffer) => {
                if let Some(Handler::OnWrite(callback)) = &self.handlers.iter().find( |h| matches!(h, Handler::OnWrite(_)) ) {
                    let ev:Option<Event>;
                    match stream.write(buffer.as_bytes()) {
                        Ok(amount) if amount == 0 => {
                            ev = Event::write(stream, buffer);
                        },
                        Ok(_amount) => {
                            ev = callback(stream, buffer);
                        },
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            ev = Event::write(stream, buffer);
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
                    let opt_ev = callback(path);
                    if let Some(ev) = opt_ev {
                        self.queue.write().unwrap().push_back( ev );
                    }
                }
            },
        }
    }

    pub fn accept<T>(&mut self, handler:T)
        where T: Fn(Stream) -> Option<Event> + 'static
    {
        self.handlers.push( Handler::OnAccept(Box::new(handler)) );
    }

    pub fn read<T>(&mut self, handler:T)
        where T: Fn(Stream,String) -> Option<Event> + 'static
    {
        self.handlers.push( Handler::OnRead(Box::new(handler)) );
    }

    pub fn write<T>(&mut self, handler:T)
        where T: Fn(Stream,String) -> Option<Event> + 'static
    {
        self.handlers.push( Handler::OnWrite(Box::new(handler)) );
    }

    pub fn watch<T>(&mut self, path:PathBuf, handler:T)
        where T: Fn(String) -> Option<Event> + 'static
    {
        self.watcher.watch(&path, notify::RecursiveMode::NonRecursive).unwrap();
        self.handlers.push( Handler::OnDir(Box::new(handler)) );
    }
}

