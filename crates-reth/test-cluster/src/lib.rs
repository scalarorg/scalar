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
    data_dir: String,
    ws_port: String,
    ws_addr: String,
    http_port: String,
    http_addr: String,
    auth_port: String,
}

const DEFAULT_DISCOVERY_ADDR: &str = "127.0.0.1";
const DEFAULT_DISCOVERY_PORT: u32 = 30333;
const DEFAULT_ADDR: &str = "127.0.0.1";
const DEFAULT_PORT: u32 = 51000;
const DEFAULT_METRICS_ADDR: &str = "127.0.0.1";
const DEFAULT_METRICS_PORT: u32 = 9001;
const DEFAULT_DATA_DIR: &str = "$HOME/.local/share/reth/";
const DEFAULT_WS_ADDR: &str = "0.0.0.0";
const DEFAULT_WS_PORT: u32 = 8546;
const DEFAULT_HTTP_ADDR: &str = "0.0.0.0";
const DEFAULT_HTTP_PORT: u32 = 8445;
const DEFAULT_AUTH_PORT: u32 = 8351;

impl TestCluster {
    pub fn start(&mut self) -> eyre::Result<()> {
        let node_cmd = NodeCommand::<()>::try_parse_from([
            "reth",
            "--dev",
            "--dev.block-time",
            "1s",
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
            self.http_addr.as_str(),
            "--http.port",
            self.http_port.as_str(),
            "--http.corsdomain",
            "*",
            "--http.api",
            "admin,debug,eth,net,trace,txpool,web3,rpc",
            "--ws",
            "--ws.addr",
            self.ws_addr.as_str(),
            "--ws.port",
            self.ws_port.as_str(),
            "--metrics",
            format!("{}:{}", self.metrics_addr, self.metrics_port).as_str(),
            "--datadir",
            self.data_dir.as_str(),
            "--authrpc.port",
            self.auth_port.as_str(),
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
    ws_port: String,
    ws_addr: String,
    http_port: String,
    http_addr: String,
    data_dir: String,
    auth_port: String,
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
            data_dir: DEFAULT_DATA_DIR.to_string(),
            ws_port: DEFAULT_WS_PORT.to_string(),
            ws_addr: DEFAULT_WS_ADDR.to_string(),
            http_port: DEFAULT_HTTP_PORT.to_string(),
            http_addr: DEFAULT_HTTP_ADDR.to_string(),
            auth_port: DEFAULT_AUTH_PORT.to_string(),
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

    pub fn data_dir(&mut self, data_dir: String) -> &mut Self {
        self.data_dir = data_dir;
        self
    }

    pub fn ws_port(&mut self, ws_port: String) -> &mut Self {
        self.data_dir = ws_port;
        self
    }

    pub fn http_port(&mut self, http_port: String) -> &mut Self {
        self.data_dir = http_port;
        self
    }

    pub fn increase_port_with_index(&mut self, index: u32) -> &mut Self {
        self.port = (DEFAULT_PORT + index).to_string();
        self.discovery_port = (DEFAULT_DISCOVERY_PORT + index).to_string();
        self.metrics_port = (DEFAULT_METRICS_PORT + index).to_string();
        self.data_dir = DEFAULT_DATA_DIR.to_string() + index.to_string().as_str() + "/";
        self.ws_port = (DEFAULT_WS_PORT + index).to_string();
        self.http_port = (DEFAULT_HTTP_PORT + index).to_string();
        self.auth_port = (DEFAULT_AUTH_PORT + index).to_string();

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
            data_dir: self.data_dir.clone(),
            ws_port: self.ws_port.clone(),
            ws_addr: self.ws_addr.clone(),
            http_port: self.http_port.clone(),
            http_addr: self.http_addr.clone(),
            auth_port: self.auth_port.clone(),
        }
    }
}
