# Example config

This config uses local development solana cluster.
- Run cluster:
> docker run -p 8899:8899 -p 8900:8900 -p 8001:8001 -p 8000-8009:8000-8009/udp -ti -e RUST_LOG=solana_runtime::system_instruction_processor=trace,solana_runtime::message_processor=debug,solana_bpf_loader=debug,solana_rbpf=debug -e NDEBUG=1 --name=solana neonlabsorg/solana:v1.8.12-testnet | grep -v 'Program Vote111111111111111111111111111111111111111'

- Deploy evm_loader (run this command from another terminal) 
> docker run -ti --network host -e SOLANA_URL=http://localhost:8899 --name=evm_loader neonlabsorg/evm_loader:latest bash -c "create-test-accounts.sh 1 && deploy-evm.sh"
