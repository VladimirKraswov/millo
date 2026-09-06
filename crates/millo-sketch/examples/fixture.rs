use std::io::{self, Read};

// Test adapter: no controller, transport, or filesystem access.
fn main() {
    let tools = millo_tooling::factory_presets();
    if std::env::args().nth(1).as_deref() == Some("tools") {
        println!("{}", serde_json::to_string(&tools).unwrap());
        return;
    }
    let mut source = String::new();
    io::stdin()
        .take(512_001)
        .read_to_string(&mut source)
        .unwrap();
    let result = serde_json::from_str(&source)
        .map_err(|e| e.to_string())
        .and_then(|request| {
            millo_sketch::generate_sketch_job(request, &tools).map_err(|e| e.to_string())
        });
    match result {
        Ok(job) => println!("{}", serde_json::to_string(&job).unwrap()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
