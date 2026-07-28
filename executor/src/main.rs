use std::{
    env, fs,
    sync::{mpsc, Arc, Mutex},
    thread,
};

use anyhow::{bail, Context, Result};
use sp1_core_executor::{ExecutionReport, GasEstimatingVMEnum, Program, SP1CoreOpts};
use sp1_core_executor_runner::MinimalExecutorRunner;

const USAGE: &str =
    "usage: randomx-executor [--estimate-gas|--estimate-gas-fast] <elf-path> <expected-public-values-hex> [input-hex ...]";

#[derive(Clone, Copy, PartialEq, Eq)]
enum GasMode {
    Off,
    Calibrated,
    Fast,
}

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let first = args.next().context(USAGE)?;
    let (gas_mode, elf_path) = match first.as_str() {
        "--estimate-gas" => (GasMode::Calibrated, args.next().context(USAGE)?),
        "--estimate-gas-fast" => (GasMode::Fast, args.next().context(USAGE)?),
        _ => (GasMode::Off, first),
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
    let gas_chunk_range = env::var("SP1_GAS_CHUNK_RANGE")
        .ok()
        .map(|range| {
            let (start, end) = range
                .split_once(':')
                .context("SP1_GAS_CHUNK_RANGE must have the form START:END")?;
            let start = start
                .parse::<usize>()
                .context("parsing the first SP1 gas chunk")?;
            let end = end
                .parse::<usize>()
                .context("parsing the last SP1 gas chunk")?;
            if start == 0 || start > end {
                bail!("SP1_GAS_CHUNK_RANGE must be a nonempty one-based inclusive range");
            }
            Ok((start, end))
        })
        .transpose()?
        .unwrap_or((1, usize::MAX));
    let gas_workers = thread::available_parallelism()
        .map(|parallelism| parallelism.get().saturating_sub(1).clamp(1, 3))
        .unwrap_or(1);
    let (gas_trace_threshold, gas_trace_slots, gas_workers) = match gas_mode {
        GasMode::Off => (0, 0, 0),
        // The calibrated gas chunk is about 2.2 GiB. One slot fits constrained
        // hosts without changing chunk boundaries or the resulting PGU count.
        GasMode::Calibrated => (opts.gas_trace_chunk_threshold, 1, 1),
        // Smaller chunks let independent gas VMs run concurrently. This is a
        // quick estimate: extra chunk boundaries slightly increase PGU.
        GasMode::Fast => (
            opts.gas_trace_chunk_threshold / 8,
            gas_workers + 2,
            gas_workers,
        ),
    };
    let mut executor = if gas_mode != GasMode::Off {
        MinimalExecutorRunner::new(
            program.clone(),
            false,
            Some(gas_trace_threshold),
            opts.memory_limit,
            gas_trace_slots,
        )
    } else {
        MinimalExecutorRunner::simple(program.clone())
    };
    for input in &inputs {
        executor.with_input(input);
    }

    let mut gas_report = ExecutionReport::default();
    let mut gas_chunks = 0usize;
    let mut estimated_gas_chunks = 0usize;
    if gas_mode == GasMode::Off {
        while executor
            .try_execute_chunk()
            .context("executing the SP1 guest")?
            .is_some()
        {}
    } else {
        let (trace_sender, trace_receiver) = mpsc::channel();
        let trace_receiver = Arc::new(Mutex::new(trace_receiver));
        let (report_sender, report_receiver) = mpsc::channel();
        let mut workers = Vec::with_capacity(gas_workers);
        for _ in 0..gas_workers {
            let trace_receiver = trace_receiver.clone();
            let report_sender = report_sender.clone();
            let program = program.clone();
            let opts = opts.clone();
            workers.push(thread::spawn(move || loop {
                let task = trace_receiver
                    .lock()
                    .expect("locking gas work queue")
                    .recv();
                let Ok((chunk_index, end_cycle, chunk)) = task else {
                    break;
                };
                let mut gas_vm =
                    GasEstimatingVMEnum::new(&chunk, program.clone(), [0u32; 4], opts.clone());
                let result = gas_vm
                    .execute()
                    .map_err(|error| format!("gas chunk {chunk_index} failed: {error}"));
                if let Ok(report) = &result {
                    eprintln!(
                        "estimated gas chunk {chunk_index} through SP1 cycle {end_cycle}: {} PGU",
                        report.gas().unwrap_or_default()
                    );
                }
                if report_sender.send(result).is_err() {
                    break;
                }
            }));
        }
        drop(report_sender);

        while let Some(chunk) = executor
            .try_execute_chunk()
            .context("executing the SP1 guest")?
        {
            gas_chunks += 1;
            if (gas_chunk_range.0..=gas_chunk_range.1).contains(&gas_chunks) {
                estimated_gas_chunks += 1;
                let end_cycle = executor.global_clk();
                trace_sender
                    .send((gas_chunks, end_cycle, chunk))
                    .map_err(|_| anyhow::anyhow!("all SP1 gas workers stopped early"))?;
            }
        }
        drop(trace_sender);
        for worker in workers {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("an SP1 gas worker panicked"))?;
        }
        for _ in 0..estimated_gas_chunks {
            gas_report += report_receiver
                .recv()
                .context("an SP1 gas worker returned no report")?
                .map_err(anyhow::Error::msg)?;
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
    if gas_mode != GasMode::Off {
        let gas = gas_report
            .gas()
            .context("the SP1 gas estimator returned no PGU count")?;
        if estimated_gas_chunks == gas_chunks {
            println!("SP1 PGU: {gas}");
        } else {
            println!("SP1 PGU subtotal: {gas}");
            println!(
                "selected gas trace chunks: {}:{}",
                gas_chunk_range.0,
                gas_chunk_range.1.min(gas_chunks)
            );
        }
        println!("gas trace chunks: {gas_chunks}");
        println!("estimated gas trace chunks: {estimated_gas_chunks}");
        println!("gas trace threshold: {gas_trace_threshold}");
        println!("gas estimator workers: {gas_workers}");
    }
    println!("public RandomX hash: {}", hex::encode(public_values));
    println!("guest exit code: {}", executor.exit_code());
    Ok(())
}
