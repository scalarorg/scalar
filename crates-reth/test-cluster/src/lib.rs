use clap::Parser;
use reth::node::NodeCommand;
use reth::runner::CliRunner;

#[derive(Clone, Default)]
pub struct TestCluster {
    port: String,
    discovery_port: String,
    discovery_addr: String,
    addr: String,
    metrics_port: String,
    metrics_addr: String,
}

const DEFAULT_DISCOVERY_ADDR: &str = "127.0.0.1";
const DEFAULT_DISCOVERY_PORT: u32 = 30333;
const DEFAULT_ADDR: &str = "127.0.0.1";
const DEFAULT_PORT: u32 = 51000;
const DEFAULT_METRICS_ADDR: &str = "127.0.0.1";
const DEFAULT_METRICS_PORT: u32 = 9001;

impl TestCluster {
    pub fn start(&mut self) -> eyre::Result<()> {
        let node_cmd = NodeCommand::<()>::try_parse_from([
            "reth",
            "--chain",
            "dev",
            "--discovery.addr",
            self.discovery_addr.as_str(),
            "--discovery.port",
            self.discovery_port.as_str(),
            "--addr",
            self.addr.as_str(),
            "--port",
            self.port.as_str(),
            "--http",
            "--http.addr",
            "0.0.0.0",
            "--http.corsdomain",
            "*",
            "--http.api",
            "admin,debug,eth,net,trace,txpool,web3,rpc",
            "--ws",
            "--ws.addr",
            "0.0.0.0",
            "--metrics",
            format!("{}:{}", self.metrics_addr, self.metrics_port).as_str(),
        ])
        .unwrap();

        let runner = CliRunner::default();
        runner.run_command_until_exit(|ctx| node_cmd.execute(ctx))
    }
}

pub struct TestClusterBuilder {
    addr: String,
    port: String,
    discovery_port: String,
    discovery_addr: String,
    metrics_port: String,
    metrics_addr: String,
}

impl Default for TestClusterBuilder {
    fn default() -> Self {
        Self {
            addr: DEFAULT_ADDR.to_string(),
            port: DEFAULT_PORT.to_string(),
            discovery_port: DEFAULT_DISCOVERY_PORT.to_string(),
            discovery_addr: DEFAULT_DISCOVERY_ADDR.to_string(),
            metrics_port: DEFAULT_METRICS_PORT.to_string(),
            metrics_addr: DEFAULT_METRICS_ADDR.to_string(),
        }
    }
}

impl TestClusterBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn port(&mut self, port: String) -> &mut Self {
        self.port = port;
        self
    }

    pub fn discovery_port(&mut self, discovery_port: String) -> &mut Self {
        self.discovery_port = discovery_port;
        self
    }

    pub fn discovery_addr(&mut self, discovery_addr: String) -> &mut Self {
        self.discovery_addr = discovery_addr;
        self
    }

    pub fn addr(&mut self, addr: String) -> &mut Self {
        self.addr = addr;
        self
    }

    pub fn metrics_port(&mut self, metrics_port: String) -> &mut Self {
        self.metrics_port = metrics_port;
        self
    }

    pub fn metrics_addr(&mut self, metrics_addr: String) -> &mut Self {
        self.metrics_addr = metrics_addr;
        self
    }

    pub fn increase_port_with_index(&mut self, index: u32) -> &mut Self {
        self.port = (DEFAULT_PORT + index).to_string();
        self.discovery_port = (DEFAULT_DISCOVERY_PORT + index).to_string();
        self.metrics_port = (DEFAULT_METRICS_PORT + index).to_string();
        self
    }

    pub fn build(&self) -> TestCluster {
        TestCluster {
            metrics_addr: self.metrics_addr.clone(),
            metrics_port: self.metrics_port.clone(),
            port: self.port.clone(),
            discovery_port: self.discovery_port.clone(),
            discovery_addr: self.discovery_addr.clone(),
            addr: self.addr.clone(),
        }
    }
}
