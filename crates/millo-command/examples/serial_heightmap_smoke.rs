use std::{
    env,
    error::Error,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use millo_command::{CommandArbiter, ExecutionTarget};
use millo_controller::ControllerConfig;
use millo_domain::{HardwareProfile, MachineTravel, ProbeWorkflowMode};
use millo_heightmap::{
    HeightmapOperationState, HeightmapPlanRequest, HeightmapStartRequest, SurfaceSessionStore,
};
use millo_serial::{SerialConfig, SerialTransport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let port = env::args()
        .nth(1)
        .unwrap_or_else(|| "/dev/cu.usbmodem11101".to_owned());
    let width_mm = argument(2, 2.0)?;
    let height_mm = argument(3, 2.0)?;
    let columns = argument(4, 2_usize)?;
    let rows = argument(5, 2_usize)?;
    let surface_session_path = env::args().nth(6).map(PathBuf::from);
    let max_probe_depth_mm = argument(7, 3.0)?;

    let mut profile = HardwareProfile::first_machine();
    profile.name = "LUNYEE hardware smoke".to_owned();
    profile.travel_mm = Some(MachineTravel {
        x: 500.0,
        y: 500.0,
        z: 200.0,
    });
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::Heightmap;

    let serial = SerialTransport::new(SerialConfig::new(port.clone(), 115_200)?);
    let (arbiter, worker) = CommandArbiter::new_with_execution_target(
        Box::new(serial),
        ControllerConfig::default(),
        profile,
        ExecutionTarget::Serial,
    );
    let worker = tokio::spawn(worker);
    let result = async {
        let connected = arbiter.connect().await?;
        println!(
            "connected={:?} mode={:?} work={:?}",
            connected.connection, connected.machine.mode, connected.machine.work_position
        );
        let mut synchronized = arbiter.refresh_status().await?;
        if synchronized.reset_notice.is_some() {
            println!("acknowledging serial-open reset banner");
            arbiter.acknowledge_reset().await?;
            synchronized = arbiter.refresh_status().await?;
        }
        println!(
            "synchronized={:?} mode={:?} work={:?}",
            synchronized.connection, synchronized.machine.mode, synchronized.machine.work_position
        );

        let prepared = arbiter
            .prepare_heightmap(HeightmapStartRequest {
                plan: HeightmapPlanRequest {
                    origin_x_mm: 0.0,
                    origin_y_mm: 0.0,
                    width_mm,
                    height_mm,
                    columns,
                    rows,
                    clearance_z_mm: 2.0,
                    max_probe_depth_mm,
                    probe_feed_mm_per_min: 25.0,
                    travel_feed_mm_per_min: 300.0,
                    retract_feed_mm_per_min: 100.0,
                    ..HeightmapPlanRequest::default()
                },
                setup_confirmed: true,
                contact_available_at_every_point: true,
            })
            .await?;
        let mut surface_store = surface_session_path
            .map(SurfaceSessionStore::load)
            .transpose()?;
        if let Some(store) = surface_store.as_mut() {
            if store.session().pending.is_some() {
                store.discard_pending()?;
            }
            store.begin("machine-0001", prepared.clone(), unix_time_ms())?;
        }
        println!(
            "prepared sequence={} points={}",
            prepared.operation_sequence, prepared.progress.total
        );
        let operation_timeout = prepared
            .map
            .as_ref()
            .map(|map| Duration::from_secs_f64(map.plan.estimated_max_seconds() + 60.0))
            .unwrap_or(Duration::from_secs(180));
        arbiter
            .commit_prepared_heightmap(prepared.operation_sequence)
            .await?;

        let mut updates = arbiter.subscribe_heightmap();
        let final_snapshot = tokio::time::timeout(operation_timeout, async {
            loop {
                let snapshot = updates.borrow_and_update().clone();
                println!(
                    "state={:?} measured={}/{} point={:?} mode={:?}",
                    snapshot.state,
                    snapshot.progress.measured,
                    snapshot.progress.total,
                    snapshot.current_sequence,
                    arbiter.snapshot().machine.mode,
                );
                if let Some(store) = surface_store.as_mut() {
                    store.checkpoint(snapshot.clone(), unix_time_ms())?;
                    if snapshot.state == HeightmapOperationState::Completed {
                        store.activate_completed(snapshot.operation_sequence, unix_time_ms())?;
                    }
                }
                if matches!(
                    snapshot.state,
                    HeightmapOperationState::Completed
                        | HeightmapOperationState::Failed
                        | HeightmapOperationState::Cancelled
                ) {
                    break Ok::<_, Box<dyn Error>>(snapshot);
                }
                updates.changed().await.expect("heightmap actor stopped");
            }
        })
        .await??;

        println!("result={}", serde_json::to_string_pretty(&final_snapshot)?);
        if final_snapshot.state != HeightmapOperationState::Completed {
            return Err(format!("heightmap ended as {:?}", final_snapshot.state).into());
        }
        let final_controller = arbiter.refresh_status().await?;
        println!(
            "final controller={:?} mode={:?} machine={:?} work={:?}",
            final_controller.connection,
            final_controller.machine.mode,
            final_controller.machine.machine_position,
            final_controller.machine.work_position,
        );
        Ok::<_, Box<dyn Error>>(())
    }
    .await;

    let _ = arbiter.disconnect().await;
    worker.abort();
    result
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn argument<T>(index: usize, default: T) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + 'static,
{
    env::args()
        .nth(index)
        .map_or(Ok(default), |value| value.parse().map_err(Into::into))
}
