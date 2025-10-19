use serde::{Serialize, Deserialize};
use std::collections::HashMap;

const DEFAULT_PATH:&'static str = ".\\dirmon.toml";

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pipe_name: String,

    #[serde(flatten)]
    dirconfs: HashMap<std::path::PathBuf, HashMap<String, String>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pipe_name: String::from("DirMon"),
            dirconfs: HashMap::new(),
        }
    }
}

pub fn load(mut path: Option<&'static str>) -> Result<Config,std::io::Error> {
    path = path.or( Some(DEFAULT_PATH) );

    let vec = std::fs::read(path.unwrap())?; 
    let strconf = str::from_utf8(vec.as_slice()).unwrap();
    
    //Add Parsing Error control
    let config:Config = toml::from_str(&strconf).unwrap();

    
    Ok(config)
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[ignore]
    fn default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.pipe_name, "DirMon");
    }

    #[test]
    fn load_test() {
        let cfg = load(None).unwrap();
        println!("{:?}", cfg);
        assert_eq!(cfg.pipe_name, "DirMon");
    }

    #[test]
    #[ignore]
    fn serialize_toml() {
        let cfg = Config {
            pipe_name: "PipeNama".to_owned(),
            dirconfs: HashMap::from([
                (
                    std::path::PathBuf::from("C:\\Temp"),
                    HashMap::from([ (String::from("Event"), String::from("Action") ) ])
                )
            ]),
        };
        
        let serialized = toml::to_string(&cfg).unwrap();

        println!();
        println!("{serialized}");
        println!();

        assert_eq!(true, true);
    }

}
