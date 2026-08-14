use millo_virtual_controller::VirtualController;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let controller = VirtualController::start().await?;
    println!("{}", controller.port_name().display());
    controller.wait().await
}
