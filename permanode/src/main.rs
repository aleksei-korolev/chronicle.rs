#![warn(missing_docs)]
//! # Permanode

use futures::executor::block_on;
use permanode_api::application::*;
use permanode_broker::application::*;
use permanode_common::{
    config::*,
    metrics::*,
};
use scylla::application::*;

launcher!
(
    builder: AppsBuilder
    {
        [] -> PermanodeBroker<Sender>: PermanodeBrokerBuilder<Sender>,
        [] -> PermanodeAPI<Sender>: PermanodeAPIBuilder<Sender>,
        [PermanodeBroker, PermanodeAPI] -> Scylla<Sender>: ScyllaBuilder<Sender>
    },
    state: Apps {config: Config}
);

impl Builder for AppsBuilder {
    type State = Apps;

    fn build(self) -> Self::State {
        let config = self.config.as_ref().expect("No config provided!");
        let permanode_api_builder = PermanodeAPIBuilder::new()
            .api_config(config.api_config.clone())
            .storage_config(config.storage_config.clone());
        let logs_dir_path = std::path::PathBuf::from("permanode/logs/");
        let permanode_broker_builder = PermanodeBrokerBuilder::new()
            .listen_address(config.broker_config.websocket_address)
            .logs_dir_path(logs_dir_path)
            .broker_config(config.broker_config.clone())
            .storage_config(config.storage_config.clone());
        let scylla_builder = ScyllaBuilder::new()
            .listen_address(config.storage_config.listen_address.to_string())
            .thread_count(match config.storage_config.thread_count {
                ThreadCount::Count(c) => c,
                ThreadCount::CoreMultiple(c) => num_cpus::get() * c,
            })
            .reporter_count(config.storage_config.reporter_count)
            .local_dc(config.storage_config.local_datacenter.clone());

        self.PermanodeAPI(permanode_api_builder)
            .PermanodeBroker(permanode_broker_builder)
            .Scylla(scylla_builder)
            .to_apps()
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().unwrap();
    env_logger::init();
    register_metrics();

    let mut config = Config::load().expect("Failed to deserialize config!");
    if let Err(e) = block_on(config.verify()) {
        panic!("{}", e)
    }
    config.save().unwrap();
    let nodes = config.storage_config.nodes.clone();

    let apps = AppsBuilder::new().config(config).build();

    apps.Scylla()
        .await
        .future(|apps| async {
            let ws = format!("ws://{}/", "127.0.0.1:8080");
            add_nodes(&ws, nodes.clone(), 1)
                .await
                .unwrap_or_else(|e| panic!("Unable to add nodes: {}", e));
            apps
        })
        .await
        .PermanodeAPI()
        .await
        .PermanodeBroker()
        .await
        .start(None)
        .await;
}

fn register_metrics() {
    REGISTRY
        .register(Box::new(INCOMING_REQUESTS.clone()))
        .expect("Could not register collector");

    REGISTRY
        .register(Box::new(RESPONSE_CODE_COLLECTOR.clone()))
        .expect("Could not register collector");

    REGISTRY
        .register(Box::new(RESPONSE_TIME_COLLECTOR.clone()))
        .expect("Could not register collector");
}
