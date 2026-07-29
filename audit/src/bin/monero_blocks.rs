use randomx_sp1_audit::network_fixtures::validate_recent_mainnet_blocks;

fn main() {
    let summary = validate_recent_mainnet_blocks();
    println!(
        "validated {} sequential Monero mainnet blocks {}..={} with reference and optimized RandomX in {:.3?}",
        summary.blocks, summary.first_height, summary.last_height, summary.elapsed
    );
}
