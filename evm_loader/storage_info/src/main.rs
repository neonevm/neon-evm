mod cmd_args;


fn main() {

    let (evm_loader,
        json_rpc_url,
        operrator
    )
        = cmd_args::parse_program_args();

    
    println!("Hello, world!");
}
