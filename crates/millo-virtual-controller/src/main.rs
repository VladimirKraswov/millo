use millo_virtual_controller::VirtualController;

#[derive(Debug, PartialEq, Eq)]
enum LaunchMode {
    Xyz,
    Rotary,
    Help,
}

fn launch_mode(args: impl IntoIterator<Item = String>) -> std::io::Result<LaunchMode> {
    let args = args.into_iter().collect::<Vec<_>>();
    match args.as_slice() {
        [] => Ok(LaunchMode::Xyz),
        [arg] if arg == "--rotary" => Ok(LaunchMode::Rotary),
        [arg] if arg == "--help" || arg == "-h" => Ok(LaunchMode::Help),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "usage: millo-virtual-controller [--rotary] (default: XYZ; --rotary: angular XYZA)",
        )),
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let controller = match launch_mode(std::env::args().skip(1))? {
        LaunchMode::Xyz => VirtualController::start().await?,
        LaunchMode::Rotary => VirtualController::start_rotary().await?,
        LaunchMode::Help => {
            println!(
                "usage: millo-virtual-controller [--rotary]\nDefault: XYZ. --rotary: angular XYZA virtual firmware."
            );
            return Ok(());
        }
    };
    println!("{}", controller.port_name().display());
    tokio::select! {
        result = controller.wait() => result,
        result = tokio::signal::ctrl_c() => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotary_is_explicit_and_unknown_options_are_rejected() {
        assert_eq!(launch_mode([]).unwrap(), LaunchMode::Xyz);
        assert_eq!(
            launch_mode(["--rotary".to_owned()]).unwrap(),
            LaunchMode::Rotary
        );
        assert_eq!(
            launch_mode(["--help".to_owned()]).unwrap(),
            LaunchMode::Help
        );
        assert!(launch_mode(["--axes=4".to_owned()]).is_err());
        assert!(launch_mode(["--rotary".to_owned(), "--rotary".to_owned()]).is_err());
    }
}
