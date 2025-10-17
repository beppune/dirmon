// # Build in release mode
// cargo build --release
//
// # Install the service (run as Administrator)
// sc create MyService binPath= "C:\path\to\your\my_service.exe"
//
// # Start the service
// sc start MyService
//
// # Stop the service
// sc stop MyService
//
// # Send custom control code 128 to reload the service
// sc control MyService 128

use named_pipe;
use std::io::{BufReader, BufWriter, BufRead, Write};

use std::ffi::OsString;
use std::time::Duration;
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

const SERVICE_NAME: &str = "my_service";
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

// Entry point for the service
fn main() -> Result<(), windows_service::Error> {
    // Register the service with the system
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

// Define the service entry point function
define_windows_service!(ffi_service_main, service_main);

// Service main function
fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service() {
        // Log error - in production, use a proper logging mechanism
        eprintln!("Service error: {}", e);
    }
}

fn run_service() -> Result<(), windows_service::Error> {
    // Define channels for service control events
    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
    let (reload_tx, reload_rx) = std::sync::mpsc::channel();

    // Define the service control handler
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                // Signal the service to stop
                shutdown_tx.send(()).unwrap();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            // Handle custom reload command (user-defined control code 128)
            ServiceControl::UserEvent(code) if code.to_raw() == 128 => {
                // Signal the service to reload configuration
                reload_tx.send(()).unwrap();
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    // Register the service control handler
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    // Tell the system that the service is running
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    // Define named pipe
    let pipename = "\\\\.\\pipe\\DirMon";
    let server = named_pipe::PipeOptions::new(pipename)
        .single().unwrap();
    let mut pipe = server.wait().unwrap();
    pipe.set_read_timeout(Some(Duration::from_secs(1)));
    let mut reader = BufReader::new(pipe);
    let mut s = String::new();

    // Main service loop
    loop {
        // Check if shutdown signal received
        match shutdown_rx.recv_timeout(Duration::from_secs(1)) {
            Ok(_) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                // Shutdown signal received
                break;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // No shutdown signal, check for reload signal
                if reload_rx.try_recv().is_ok() {
                    // TODO: Reload configuration here
                    println!("Reloading configuration...");
                }

                // TODO: Add your service logic here
                reader.read_line( &mut s ).unwrap();
                reader.get_mut().write_all( s.as_ref() ).unwrap();
                //Read Command
                

                //Update 
            }
        }
    }

    // Tell the system that the service has stopped
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

