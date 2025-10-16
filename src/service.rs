use windows_service::*;
use std::ffi::OsString;

define_windows_service!(SERVICE_MAIN, dirmon_service);

fn main() -> Result<()> {
    println!("SERVICE");

    service_dispatcher::start("DirMon", SERVICE_MAIN)?;

    Ok(())
}

fn dirmon_service(args: Vec<OsString>) {
    
}
