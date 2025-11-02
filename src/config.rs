use serde::{Serialize, Deserialize};
use std::collections::HashMap;

const DEFAULT_PATH:&'static str = ".\\dirmon.toml";

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Hash)]
pub enum FsEvent {
    Create,
    Delete,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Config {
    pub pipe_name: String,
    pub logfile: String,
    pub nopipe: bool,

    #[serde(flatten)]
    pub dirconfs: HashMap<std::path::PathBuf, HashMap<FsEvent, String>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pipe_name: String::from("DirMon"),
            logfile: String::from(".\\dirmon.log"),
            nopipe: false,
            dirconfs: HashMap::new(),
        }
    }
}

pub fn load(mut path: Option<String>) -> Result<Config,std::io::Error> {
    path = path.or( Some(DEFAULT_PATH.to_string()) );

    let vec = std::fs::read(path.unwrap())?; 
    let strconf = str::from_utf8(vec.as_slice()).unwrap();
    
    //Add Parsing Error control
    let config:Config = toml::from_str(&strconf).unwrap();

    
    Ok(config)
}

