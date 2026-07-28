use std::{env, fs, sync::Arc};

use anyhow::{bail, Context, Result};
use sp1_core_executor::Program;
use sp1_core_executor_runner::MinimalExecutorRunner;

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let elf_path = args.next().context(
        "usage: randomx-executor <elf-path> <expected-public-values-hex> [input-hex ...]",
    )?;
    let expected_hex = args.next().context(
        "usage: randomx-executor <elf-path> <expected-public-values-hex> [input-hex ...]",
    )?;
    let inputs = args
        .map(|input| hex::decode(&input).context("decoding an input argument"))
        .collect::<Result<Vec<_>>>()?;

    let expected = hex::decode(&expected_hex).context("decoding expected public values")?;
    if expected.len() != 32 {
        bail!("expected 32 public bytes, got {}", expected.len());
    }

    let elf = fs::read(&elf_path).with_context(|| format!("reading SP1 ELF from {elf_path}"))?;
    let program = Arc::new(
        Program::from(elf.as_slice())
            .map_err(|error| anyhow::anyhow!("parsing the SP1 ELF failed: {error:#}"))?,
    );
    let mut executor = MinimalExecutorRunner::simple(program);
    for input in &inputs {
        executor.with_input(input);
    }

    while executor
        .try_execute_chunk()
        .context("executing the SP1 guest")?
        .is_some()
    {}

    if !executor.is_done() {
        bail!("the lightweight executor returned before the guest halted");
    }
    if executor.exit_code() != 0 {
        bail!(
            "the SP1 guest halted with exit code {}",
            executor.exit_code()
        );
    }

    let public_values = executor.public_values_stream();
    if public_values.as_slice() != expected {
        bail!(
            "unexpected public values: expected {}, got {}",
            hex::encode(expected),
            hex::encode(public_values)
        );
    }

    println!("SP1 cycles: {}", executor.global_clk());
    println!("public RandomX hash: {}", hex::encode(public_values));
    println!("guest exit code: {}", executor.exit_code());
    Ok(())
}
