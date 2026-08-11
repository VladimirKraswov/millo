use std::{error::Error, io, time::Duration};

use millo_command::{CommandArbiter, ExecutionTarget};
use millo_controller::ControllerConfig;
use millo_domain::{
    ControllerOverrides, HardwareProfile, MachineMode, OverrideAdjustment, RapidOverrideTarget,
};
use millo_serial::{SerialConfig, SerialTransport};

const USAGE: &str = "usage: hardware_overrides <serial-port>";
const EXPECTED_TEST_OVERRIDES: ControllerOverrides = ControllerOverrides {
    feed_percent: 110,
    rapid_percent: 50,
    spindle_percent: 99,
};
const RESET_OVERRIDES: ControllerOverrides = ControllerOverrides {
    feed_percent: 100,
    rapid_percent: 100,
    spindle_percent: 100,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let port = args.next().ok_or_else(|| input_error(USAGE))?;
    if args.next().is_some() {
        return Err(input_error(USAGE).into());
    }

    let transport = SerialTransport::new(SerialConfig::new(port.clone(), 115_200)?);
    let (arbiter, worker) = CommandArbiter::new_with_execution_target(
        Box::new(transport),
        ControllerConfig::default(),
        HardwareProfile::first_machine(),
        ExecutionTarget::Serial,
    );
    let worker = tokio::spawn(worker);

    let result = run(&arbiter, &port).await;
    let cleanup = reset_overrides(&arbiter).await;
    let _ = arbiter.disconnect().await;
    worker.abort();

    if let Err(error) = cleanup {
        return Err(input_error(format!(
            "override cleanup failed after test: {error}; reconnect and reset Feed/Rapid/Spindle overrides to 100%"
        ))
        .into());
    }
    result
}

async fn run(arbiter: &CommandArbiter, port: &str) -> Result<(), Box<dyn Error>> {
    println!("Connecting to {port} at 115200 baud; no motion command will be sent");
    arbiter.connect().await?;
    let mut snapshot = arbiter.refresh_status().await?;
    if snapshot.reset_notice.is_some() {
        snapshot = arbiter.acknowledge_reset().await?;
    }
    if snapshot.machine.mode != MachineMode::Idle {
        return Err(input_error(format!(
            "controller must be Idle, got {:?}",
            snapshot.machine.mode
        ))
        .into());
    }

    reset_overrides(arbiter).await?;
    arbiter
        .adjust_feed_override(OverrideAdjustment::IncreaseTen)
        .await?;
    arbiter
        .set_rapid_override(RapidOverrideTarget::Half)
        .await?;
    arbiter
        .adjust_spindle_override(OverrideAdjustment::DecreaseOne)
        .await?;
    wait_for_overrides(arbiter, EXPECTED_TEST_OVERRIDES).await?;
    println!("Observed test overrides: Feed 110%, Rapid 50%, Spindle 99%");

    reset_overrides(arbiter).await?;
    println!("PASS: realtime override bytes were accepted and restored to 100/100/100");
    Ok(())
}

async fn reset_overrides(arbiter: &CommandArbiter) -> Result<(), Box<dyn Error>> {
    arbiter
        .adjust_feed_override(OverrideAdjustment::Reset)
        .await?;
    arbiter
        .set_rapid_override(RapidOverrideTarget::Full)
        .await?;
    arbiter
        .adjust_spindle_override(OverrideAdjustment::Reset)
        .await?;
    wait_for_overrides(arbiter, RESET_OVERRIDES).await
}

async fn wait_for_overrides(
    arbiter: &CommandArbiter,
    expected: ControllerOverrides,
) -> Result<(), Box<dyn Error>> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let snapshot = arbiter.refresh_status().await?;
        if snapshot.machine.overrides == Some(expected) {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(input_error(format!(
                "timed out waiting for overrides {}/{}/{}; last status reported {:?}",
                expected.feed_percent,
                expected.rapid_percent,
                expected.spindle_percent,
                snapshot.machine.overrides
            ))
            .into());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn input_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
