use std::{env, fs, sync::Arc};

use anyhow::{bail, Context, Result};
use sp1_core_executor::Program;
use sp1_core_executor_runner::MinimalExecutorRunner;

fn main() -> Result<()> {
    let elf_path = env::args()
        .nth(1)
        .context("usage: randomx-softfp-runner <guest.elf>")?;
    let elf = fs::read(&elf_path).with_context(|| format!("reading SP1 ELF {elf_path}"))?;
    let program = Arc::new(
        Program::from(elf.as_slice())
            .map_err(|error| anyhow::anyhow!("parsing the SP1 ELF failed: {error:#}"))?,
    );
    let mut executor = MinimalExecutorRunner::simple(program);

    while executor
        .try_execute_chunk()
        .context("executing the SP1 guest")?
        .is_some()
    {}
    if !executor.is_done() {
        bail!("executor returned before the guest halted");
    }
    if executor.exit_code() != 0 {
        bail!("guest halted with exit code {}", executor.exit_code());
    }

    println!("SP1 total cycles: {}", executor.global_clk());
    let mut phases: Vec<_> = executor.take_cycle_tracker_totals().into_iter().collect();
    phases.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    for (label, cycles) in phases {
        println!(
            "{label}: {cycles} total, {:.3} per two-lane op",
            cycles as f64 / 1_024.0
        );
    }
    println!("checksum: {}", hex::encode(executor.public_values_stream()));
    Ok(())
}
