use std::{fmt::Write, time::Instant};

use millo_dry_run::build_dry_run_plan;
use millo_gcode::{ProgramParseRequest, parse_program};
use millo_sender::{Sender, SenderState};

fn main() {
    let motion_count = std::env::args()
        .nth(1)
        .map(|value| value.parse::<usize>().expect("motion count"))
        .unwrap_or(1_000_000);
    assert!((1..=1_999_998).contains(&motion_count));
    let mut source = String::with_capacity(motion_count * 24);
    source.push_str("G21 G90 G94 G17 G54\n");
    for index in 0..motion_count {
        writeln!(
            source,
            "G1 X{} Y{} Z-0.1 F600",
            index % 100,
            (index / 100) % 100
        )
        .unwrap();
    }
    let bytes = source.len();
    let started = Instant::now();
    let program = parse_program(ProgramParseRequest {
        source_name: "million-lines.nc".into(),
        source,
    })
    .unwrap();
    let parse_ms = started.elapsed().as_millis();
    assert_eq!(program.summary.motion_count, motion_count);
    assert!(program.summary.preview_complete);
    let started = Instant::now();
    let plan = build_dry_run_plan(&program).unwrap();
    let plan_ms = started.elapsed().as_millis();
    let planned_lines = plan.lines().len();
    let mut sender = Sender::default();
    sender.load_air_run(plan).unwrap();
    sender.start().unwrap();
    let started = Instant::now();
    let mut commands = 0;
    let mut maximum_pause_us = 0;
    while let Some(line) = sender.next_line() {
        assert!(line.wire_command_len() <= 255);
        commands += 1;
        if commands % 100_000 == 0 {
            let pause = Instant::now();
            sender.pause().unwrap();
            assert!(sender.next_line().is_none());
            maximum_pause_us = maximum_pause_us.max(pause.elapsed().as_micros());
            sender.resume().unwrap();
        }
        sender.acknowledge_ok().unwrap();
    }
    assert_eq!(sender.snapshot().state, SenderState::Draining);
    sender.complete_draining().unwrap();
    assert_eq!(sender.snapshot().state, SenderState::Completed);
    assert_eq!(commands, planned_lines);
    assert_eq!(sender.snapshot().acknowledged_lines, planned_lines);
    println!(
        "source_bytes={bytes} motions={motion_count} sender_lines={commands} parse_ms={parse_ms} plan_ms={plan_ms} dispatch_ack_ms={} max_sender_pause_us={maximum_pause_us}",
        started.elapsed().as_millis()
    );
}
