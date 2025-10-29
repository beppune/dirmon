
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use interprocess::local_socket::{Listener, *};
use interprocess::local_socket::traits::Listener as Listen;
use notify::Watcher;

use std::io::Read;

type AcceptHandler = Box<dyn Fn(Stream) -> Option<Event>>;
type ReadHandler = Box<dyn Fn(Stream,usize) -> Option<Event>>;
type WriteHandler = Box<dyn Fn(Stream,usize) -> Option<Event>>;
type WatchHandler = Box<dyn Fn(String) -> Option<Event>>;

enum Handler {
    OnAccept(AcceptHandler),
    OnRead(ReadHandler),
    OnWrite(WriteHandler),
    OnCreate(WatchHandler),
    OnDelete(WatchHandler),
}

pub enum Event {
    //pipes
    Accept(Stream),
    Read(Stream),
    Write(Stream),

    //wacthers
    DirmonCreate(String),
    DirmonDelete(String),
}

impl Event {
    fn read(stream:Stream) -> Option<Event> {
        Some( Event::Read(stream) )
    }

    fn write(stream:Stream) -> Option<Event> {
        Some( Event::Write(stream) )
    }

    fn created(path:String) -> Option<Event> {
        Some( Event::DirmonCreate(path) )
    }

    fn deleted(path:String) -> Option<Event> {
        Some( Event::DirmonDelete(path) )
    }
}

pub struct Reactor {
    listener: Listener,
    queue: Arc<RwLock<VecDeque<Event>>>,
    handlers: Vec<Handler>,
    watcher: notify::RecommendedWatcher,
}

impl Reactor {
    fn new(listener: Listener) -> Self {
        let q = Arc::new(RwLock::new(VecDeque::new()));
        Self {
            listener,
            queue: q.clone(),
            handlers: vec![],
            watcher: notify::recommended_watcher(move |res:notify::Result<notify::Event>| {
                match res {
                    Ok(event) => {
                        match event.kind {
                            // notify::EventKind::Any => todo!(),
                            // notify::EventKind::Access(access_kind) => todo!(),
                            // notify::EventKind::Modify(modify_kind) => todo!(),
                            // notify::EventKind::Other => todo!(),
                            notify::EventKind::Create(_) => {
                                let path = event.paths.get(0).unwrap().clone().into_os_string().into_string().unwrap();
                                {
                                    q.write().unwrap().push_back( Event::DirmonCreate(path) ); 
                                }
                            },
                            notify::EventKind::Remove(_) => {
                                let path = event.paths.get(0).unwrap().clone().into_os_string().into_string().unwrap();
                                {
                                    q.write().unwrap().push_back( Event::DirmonDelete(path) ); 
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

    fn run(&mut self) {

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
                Err(err) if err.kind() == ErrorKind::WouldBlock => {},
                Err(err) => {
                    println!("{err}");
                },
            }
        }
    }

    fn demux(&mut self) -> Option<Event> {
        self.queue.write().unwrap().pop_front()
    }

    fn dispatch(&mut self, event:Event, buffer:&mut String) {

        match event {
            Event::Accept(stream) => {
                if let Some(Handler::OnAccept(callback)) = &self.handlers.iter().find( |h| matches!(h, Handler::OnAccept(_)) ) {
                    if let Some(ev) = callback(stream) {   
                        self.queue.write().unwrap().push_back( ev ); 
                    }
                }
            },
            Event::Read(mut stream) => {
                if let Some(Handler::OnRead(callback)) = &self.handlers.iter().find( |h| matches!(h, Handler::OnRead(_)) ) {
                    let ev:Option<Event>;
                    buffer.clear();
                    match stream.read_to_string(buffer) {
                        Ok(amount) if amount == 0 => {
                            ev = Event::read(stream);
                        },
                        Ok(amount) => {
                            print!("In dispatcher: {buffer}");
                            ev = callback(stream, amount);
                        },
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            ev = Event::read(stream);
                        },
                        Err(_) => {
                            ev = None;
                        }
                    }
                    if let Some(e) = ev {
                        self.queue.write().unwrap().push_back( e );
                    }
                }
            },
            Event::Write(mut stream) => {
                if let Some(Handler::OnWrite(callback)) = &self.handlers.iter().find( |h| matches!(h, Handler::OnWrite(_)) ) {
                    let ev:Option<Event>;
                    match stream.write(buffer.as_bytes()) {
                        Ok(amount) if amount == 0 => {
                            ev = Event::write(stream);
                        },
                        Ok(amount) => {
                            ev = callback(stream, amount);
                        },
                        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                            ev = Event::write(stream);
                        },
                        Err(_) => {
                            ev = None;
                        },
                    }
                    if let Some(e) = ev {
                        self.queue.write().unwrap().push_back( e );
                    }
                }
            },
            Event::DirmonCreate(_) => println!("create"),
            Event::DirmonDelete(_) => println!("create"),
        }
    }

    fn accept<T>(&mut self, handler:T)
        where T: Fn(Stream) -> Option<Event> + 'static
    {
        self.handlers.push( Handler::OnAccept(Box::new(handler)) );
    }

    fn read<T>(&mut self, handler:T)
        where T: Fn(Stream,usize) -> Option<Event> + 'static
    {
        self.handlers.push( Handler::OnRead(Box::new(handler)) );
    }

    fn write<T>(&mut self, handler:T)
        where T: Fn(Stream,usize) -> Option<Event> + 'static
    {
        self.handlers.push( Handler::OnWrite(Box::new(handler)) );
    }

    fn create<T>(&mut self, path:PathBuf, handler:T)
        where T: Fn(String) -> Option<Event> + 'static
    {
        self.watcher.watch(&path, notify::RecursiveMode::NonRecursive).unwrap();
        self.handlers.push( Handler::OnCreate(Box::new(handler)) );
    }

    fn delete<T>(&mut self, path:PathBuf, handler:T)
        where T: Fn(String) -> Option<Event> + 'static
    {
        self.watcher.watch(&path, notify::RecursiveMode::NonRecursive).unwrap();
        self.handlers.push( Handler::OnDelete(Box::new(handler)) );
    }
}
