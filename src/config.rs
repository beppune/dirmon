

pub struct Config {
    pipe_name: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pipe_name: String::from("DirMon"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = Config::default();

        assert_eq!(cfg.pipe_name, "DirMon");
    }
}
