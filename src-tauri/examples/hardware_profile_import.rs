use std::{error::Error, io, path::PathBuf};

use millo_command::CommandArbiter;
use millo_controller::ControllerConfig;
use millo_domain::HardwareProfile;
use millo_profile::{MachineConnectionPreset, MachineProfileDraft, MachineProfileStore};
use millo_serial::{SerialConfig, SerialTransport};

const USAGE: &str =
    "usage: hardware_profile_import <serial-port> <profile-store.json> <machine-name>";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let port = args.next().ok_or_else(|| input_error(USAGE))?;
    let profile_path = PathBuf::from(args.next().ok_or_else(|| input_error(USAGE))?);
    let name = args.next().ok_or_else(|| input_error(USAGE))?;
    if args.next().is_some() {
        return Err(input_error(USAGE).into());
    }

    let transport = SerialTransport::new(SerialConfig::new(&port, 115_200)?);
    let (arbiter, worker) = CommandArbiter::new(
        Box::new(transport),
        ControllerConfig::default(),
        HardwareProfile::first_machine(),
    );
    let worker = tokio::spawn(worker);

    let result = import_profile(&arbiter, &port, &profile_path, name).await;
    let _ = arbiter.disconnect().await;
    worker.abort();
    result
}

async fn import_profile(
    arbiter: &CommandArbiter,
    port: &str,
    profile_path: &PathBuf,
    name: String,
) -> Result<(), Box<dyn Error>> {
    println!("Read-only GRBL inspection on {port} at 115200 baud");
    arbiter.connect().await?;
    let snapshot = arbiter.refresh_status().await?;
    if snapshot.reset_notice.is_some() {
        arbiter.acknowledge_reset().await?;
    }
    let inspection = arbiter.inspect_device().await?;
    let draft = MachineProfileDraft::from_grbl_inspection(
        name,
        &inspection.device,
        MachineConnectionPreset {
            transport_id: format!("serial:{port}"),
            baud_rate: 115_200,
        },
    )?;
    let mut store = MachineProfileStore::load(profile_path)?;
    let state = store.create_and_select(draft)?;
    let profile = state
        .selected()
        .ok_or_else(|| input_error("profile store did not select the imported machine"))?;

    println!(
        "Saved {}: X {:.3} mm, Y {:.3} mm, Z {:.3} mm; firmware={}; limits={}; homing={}; probe=false",
        profile.name,
        profile.travel_mm.x,
        profile.travel_mm.y,
        profile.travel_mm.z,
        profile
            .detected_controller
            .as_ref()
            .and_then(|controller| controller.firmware_version.as_deref())
            .unwrap_or("unknown"),
        profile.limit_switches_installed,
        profile.homing_installed,
    );
    println!("Profile store: {}", profile_path.display());
    Ok(())
}

fn input_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
