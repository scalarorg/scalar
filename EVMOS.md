1. Modify Evmos code base

   Add flowing line into the evmos/server/start.go

   ```
   ....
   server cmd.Flags().String(srvflags.ProxyApp, "tcp://127.0.0.1:26658", "Abci server address")
   .....
   cmd.Flags().String(srvflags.Transport, "socket", "Transport protocol: socket, grpc")
   ```

   evmos/server/flags/flags.go

   ```
   // Tendermint/cosmos-sdk full-node start flags
   const (
       WithTendermint = "with-tendermint"
       ProxyApp = "proxy-app" //Add constant for the abci app endpoint
       Address = "address"
       Transport = "transport"
       TraceStore = "trace-store"
       CPUProfile = "cpu-profile"
       // The type of database for application and snapshots databases
       AppDBBackend = "app-db-backend"
   )

   ```

2. Start dockers
   ```
   docker-compose -f docker/docker-cluster-evmos.yml up
   ```
