# scalar
Scalar block chain
Pakages structure
# Prepare
https://docs.sui.io/references/cli/keytool
# Run
Currently following steps work on only ubuntu 20.04 or Ubuntu 22.04 with intel chips

- Start builder and runner docker containers ` ./scripts/start.sh containers`
- Build reth ` ./scripts/build.sh reth `
- Build scalar ` ./scripts/build.sh scalar `
- Run reth ` ./script/run.sh reth `
- Run scalar ` ./script/run.sh scalar `