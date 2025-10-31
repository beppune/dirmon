
use crate::reactor::Event;

pub fn parse_command(string:&str) -> Result<Event,()> {
    if string.contains("QUIT") {
        return Ok(Event::Quit);
    }

    return Err(());
}
