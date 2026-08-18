use agent::AgentRuntime;
use helper::HelperApi;
use protocol::Capability;

#[tokio::main]
async fn main() {
    let _runtime = AgentRuntime::new(
        HelperApi::from_env(),
        vec![
            Capability {
                name: "systemd".to_string(),
                version: "v1".to_string(),
            },
            Capability {
                name: "system.metrics".to_string(),
                version: "v1".to_string(),
            },
        ],
    );

    println!("argus-agent started");
}
