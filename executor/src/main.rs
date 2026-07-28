use std::{env, fs, sync::Arc};

use anyhow::{bail, Context, Result};
use sp1_core_executor::{ExecutionReport, GasEstimatingVMEnum, Program, SP1CoreOpts};
use sp1_core_executor_runner::MinimalExecutorRunner;

const USAGE: &str =
    "usage: randomx-executor [--estimate-gas] <elf-path> <expected-public-values-hex> [input-hex ...]";

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let first = args.next().context(USAGE)?;
    let (estimate_gas, elf_path) = if first == "--estimate-gas" {
        (true, args.next().context(USAGE)?)
    } else {
        (false, first)
    };
    let expected_hex = args.next().context(USAGE)?;
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
    let opts = SP1CoreOpts::default();
    let mut executor = if estimate_gas {
        // The calibrated gas chunk is about 2.2 GiB. A single shared-memory
        // slot keeps the estimator usable on constrained hosts without
        // changing chunk boundaries or the resulting PGU count.
        MinimalExecutorRunner::new(
            program.clone(),
            false,
            Some(opts.gas_trace_chunk_threshold),
            opts.memory_limit,
            1,
        )
    } else {
        MinimalExecutorRunner::simple(program.clone())
    };
    for input in &inputs {
        executor.with_input(input);
    }

    let mut gas_report = ExecutionReport::default();
    let mut gas_chunks = 0usize;
    while let Some(chunk) = executor
        .try_execute_chunk()
        .context("executing the SP1 guest")?
    {
        if estimate_gas {
            let mut gas_vm =
                GasEstimatingVMEnum::new(&chunk, program.clone(), [0u32; 4], opts.clone());
            gas_report += gas_vm.execute().context("estimating the SP1 prover gas")?;
            gas_chunks += 1;
            eprintln!(
                "estimated gas chunk {gas_chunks} through SP1 cycle {}: {} cumulative PGU",
                executor.global_clk(),
                gas_report
                    .gas()
                    .context("the SP1 gas estimator returned no PGU count")?
            );
        }
    }

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
    if estimate_gas {
        println!(
            "SP1 PGU: {}",
            gas_report
                .gas()
                .context("the SP1 gas estimator returned no PGU count")?
        );
        println!("gas trace chunks: {gas_chunks}");
    }
    println!("public RandomX hash: {}", hex::encode(public_values));
    println!("guest exit code: {}", executor.exit_code());
    Ok(())
}
