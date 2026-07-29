use std::{env, fs, path::Path, str::FromStr, time::Duration};

use alloy_primitives::{keccak256, Address, Bytes, B256};
use alloy_sol_types::{sol, SolCall};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sp1_sdk::{
    network::{
        proto::GetProofRequestParamsResponse as ParamsResponse, signer::NetworkSigner,
        NetworkClient, NetworkMode,
    },
    Elf, HashableKey, NetworkProver, ProveRequest, Prover, ProvingKey, SP1Proof, SP1ProofMode,
    SP1ProofWithPublicValues, SP1Stdin,
};

const MAINNET_RPC_URL: &str = "https://rpc.mainnet.succinct.xyz";
const ETHEREUM_MAINNET_CHAIN_ID: u64 = 1;
const GROTH16_GATEWAY: &str = "0x397A5f7f3dBd538f23DE225B51f532c34448dA9B";
const DEFAULT_PGU_LIMIT: u64 = 8_500_000_000;
const DEFAULT_AUCTION_TIMEOUT_SECS: u64 = 600;
const MIN_REQUEST_TIMEOUT_SECS: u64 = 600;
const PROVE_WEI: u128 = 1_000_000_000_000_000_000;
const APPROVED_ELF_SHA256: Option<&str> = None;
const USAGE: &str = "usage:
  randomx-sp1-network-prover account <private-key-file> [pgu-limit]
  randomx-sp1-network-prover prove <private-key-file> <elf> <request-id-file> <proof-file> <vkey-file>
  randomx-sp1-network-prover resume <private-key-file> <elf> <request-id-file> <proof-file> <vkey-file>
  EVM_RPC_URL=<url> randomx-sp1-network-prover evm-verify <elf> <vkey-file> <proof-file> [gateway]";

sol! {
    interface ISP1Verifier {
        function verifyProof(
            bytes32 programVKey,
            bytes calldata publicValues,
            bytes calldata proofBytes
        ) external view;
    }
}

#[derive(Debug)]
struct ProofInput {
    network: &'static str,
    height: u64,
    block_id: &'static str,
    prev_hash: &'static str,
    timestamp: u64,
    wide_difficulty: &'static str,
    seed_height: u64,
    seed_hash: &'static str,
    hashing_blob: &'static str,
    pow_hash: &'static str,
    cycle_limit: u64,
    gas_limit: u64,
    request_timeout_seconds: u64,
}

const SELECTED_BLOCK: ProofInput = ProofInput {
    network: "monero-mainnet",
    height: 3_727_837,
    block_id: "fd20c878bddf0302867fcc5f7ce6b01e6e8d61ee0a4351879232793a8665f6af",
    prev_hash: "df66d34b58d9c65ee20ca8e7c307608db0f7c4e7c6b450bc38e3348d2778f51b",
    timestamp: 1_785_253_434,
    wide_difficulty: "0x9e0ea93a72",
    seed_height: 3_727_360,
    seed_hash: "0e3b4521acd1982c62a99b6b76ad8504eaa80e164d8e9df3f047b1cf6607f2bd",
    hashing_blob: "1010ba9ca3d306df66d34b58d9c65ee20ca8e7c307608db0f7c4e7c6b450bc38e3348d2778f51b4940173c7c0f26941324afc7aa4e30ffa1b2cd80a84ebbfc464833d6222bee72886d3a9d8a01",
    pow_hash: "5cff906139956eb646100adef11db2e00464ffabfdf4d5a194d54f0000000000",
    cycle_limit: 6_500_000_000,
    gas_limit: 8_000_000_000,
    request_timeout_seconds: 3_600,
};

#[derive(Debug)]
struct Quote {
    balance_wei: u128,
    market_price: u128,
    market_as_of: u64,
    base_fee: u64,
    raw_price: u64,
    tick_size: u64,
    sdk_price_cap: u64,
    cost_cap: u128,
}

#[tokio::main]
async fn main() -> Result<()> {
    // SP1's network feature contains both the Ring and optional AWS-LC stacks.
    // Select the same provider as SP1's own `NetworkProver` constructor before
    // establishing a direct read-only `NetworkClient` connection.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let mut args = env::args().skip(1);
    let command = args.next().context(USAGE)?;
    match command.as_str() {
        "account" => {
            let key_path = args.next().context(USAGE)?;
            let pgu_limit = args
                .next()
                .map(|value| value.parse().context("parsing PGU limit"))
                .transpose()?
                .unwrap_or(DEFAULT_PGU_LIMIT);
            ensure_no_more_args(args)?;
            print_account(Path::new(&key_path), pgu_limit).await
        }
        "prove" | "resume" => {
            let key_path = args.next().context(USAGE)?;
            let elf_path = args.next().context(USAGE)?;
            let request_id_path = args.next().context(USAGE)?;
            let proof_path = args.next().context(USAGE)?;
            let vkey_path = args.next().context(USAGE)?;
            ensure_no_more_args(args)?;
            prove_or_resume(
                command == "prove",
                Path::new(&key_path),
                Path::new(&elf_path),
                Path::new(&request_id_path),
                Path::new(&proof_path),
                Path::new(&vkey_path),
            )
            .await
        }
        "evm-verify" => {
            let elf_path = args.next().context(USAGE)?;
            let vkey_path = args.next().context(USAGE)?;
            let proof_path = args.next().context(USAGE)?;
            let gateway = args.next().unwrap_or_else(|| GROTH16_GATEWAY.to_owned());
            ensure_no_more_args(args)?;
            evm_verify(
                Path::new(&elf_path),
                Path::new(&vkey_path),
                Path::new(&proof_path),
                &gateway,
            )
            .await
        }
        _ => bail!(USAGE),
    }
}

fn ensure_no_more_args(mut args: impl Iterator<Item = String>) -> Result<()> {
    if args.next().is_some() {
        bail!(USAGE);
    }
    Ok(())
}

async fn print_account(key_path: &Path, pgu_limit: u64) -> Result<()> {
    let signer = read_signer(key_path)?;
    let address = signer.address();
    let client = NetworkClient::new(signer, MAINNET_RPC_URL, NetworkMode::Mainnet);
    let quote = query_quote(&client, pgu_limit).await?;

    println!("requester address: {address}");
    print_quote(&quote, pgu_limit);
    Ok(())
}

async fn prove_or_resume(
    submit: bool,
    key_path: &Path,
    elf_path: &Path,
    request_id_path: &Path,
    proof_path: &Path,
    vkey_path: &Path,
) -> Result<()> {
    let input = &SELECTED_BLOCK;
    validate_input(input)?;
    let expected = decode_32(input.pow_hash, "PoW hash")?;
    let seed = decode_32(input.seed_hash, "RandomX seed hash")?.to_vec();
    let blob = hex::decode(input.hashing_blob).context("decoding hashing blob")?;
    let elf = fs::read(elf_path)
        .with_context(|| format!("reading SP1 ELF from {}", elf_path.display()))?;
    validate_elf(&elf)?;

    if submit && request_id_path.exists() {
        bail!(
            "refusing to submit a duplicate request because {} already exists; use resume",
            request_id_path.display()
        );
    }
    if submit && proof_path.exists() {
        bail!(
            "refusing to submit because proof output already exists: {}",
            proof_path.display()
        );
    }

    let signer = read_signer(key_path)?;
    let address = signer.address();
    let network_client = NetworkClient::new(signer.clone(), MAINNET_RPC_URL, NetworkMode::Mainnet);
    let quote = query_quote(&network_client, input.gas_limit).await?;
    if submit && quote.balance_wei < quote.cost_cap {
        bail!(
            "requester {address} has {} PROVE but the request cap is {} PROVE",
            format_prove(quote.balance_wei),
            format_prove(quote.cost_cap)
        );
    }

    let prover = NetworkProver::new(signer, MAINNET_RPC_URL, NetworkMode::Mainnet).await;
    let pk = prover
        .setup(Elf::from(elf))
        .await
        .context("setting up the SP1 program")?;
    let vkey = pk.verifying_key().bytes32();
    write_or_verify_text(vkey_path, &vkey, "program vkey")?;
    println!("program vkey: {vkey}");
    println!("saved program vkey: {}", vkey_path.display());
    println!("requester address: {address}");
    print_quote(&quote, input.gas_limit);

    let request_id = if submit {
        let mut stdin = SP1Stdin::new();
        stdin.write_vec(seed);
        stdin.write_vec(blob);
        let request_id = prover
            .prove(&pk, stdin)
            .groth16()
            .cycle_limit(input.cycle_limit)
            .gas_limit(input.gas_limit)
            .skip_simulation(true)
            .timeout(Duration::from_secs(input.request_timeout_seconds))
            .min_auction_period(15)
            .max_price_per_pgu(quote.sdk_price_cap)
            .request()
            .await
            .context("submitting Groth16 proof request")?;
        write_text(request_id_path, &format!("{request_id}\n"))?;
        println!("request id: {request_id}");
        request_id
    } else {
        let value = fs::read_to_string(request_id_path)
            .with_context(|| format!("reading request ID from {}", request_id_path.display()))?;
        let request_id = B256::from_str(value.trim()).context("parsing request ID")?;
        println!("request id: {request_id}");
        request_id
    };

    let proof = prover
        .wait_proof(
            request_id,
            Some(Duration::from_secs(input.request_timeout_seconds)),
            Some(Duration::from_secs(DEFAULT_AUCTION_TIMEOUT_SECS)),
        )
        .await
        .context("waiting for Groth16 proof")?;
    verify_and_save(&prover, &pk, &proof, &expected, proof_path)?;
    Ok(())
}

async fn evm_verify(
    elf_path: &Path,
    vkey_path: &Path,
    proof_path: &Path,
    gateway: &str,
) -> Result<()> {
    let input = &SELECTED_BLOCK;
    validate_input(input)?;
    let elf = fs::read(elf_path)
        .with_context(|| format!("reading SP1 ELF from {}", elf_path.display()))?;
    validate_elf(&elf)?;

    let expected = decode_32(input.pow_hash, "PoW hash")?;
    let vkey_text = fs::read_to_string(vkey_path)
        .with_context(|| format!("reading program vkey from {}", vkey_path.display()))?;
    let vkey = decode_32(vkey_text.trim(), "program vkey")?;
    let proof = SP1ProofWithPublicValues::load(proof_path)
        .with_context(|| format!("loading SP1 proof from {}", proof_path.display()))?;
    if !matches!(&proof.proof, SP1Proof::Groth16(_)) {
        bail!("EVM verification requires a Groth16 proof");
    }
    if proof.public_values.as_slice() != expected {
        bail!(
            "proof public values mismatch: expected {}, got {}",
            hex::encode(expected),
            hex::encode(proof.public_values.as_slice())
        );
    }

    let gateway = Address::from_str(gateway).context("parsing Groth16 gateway address")?;
    let call = ISP1Verifier::verifyProofCall {
        programVKey: B256::from(vkey),
        publicValues: Bytes::copy_from_slice(proof.public_values.as_slice()),
        proofBytes: Bytes::from(proof.bytes()),
    };
    let calldata = call.abi_encode();
    let rpc_url = env::var("EVM_RPC_URL").context("EVM_RPC_URL must be set")?;
    let http = reqwest::Client::builder()
        .build()
        .context("building EVM JSON-RPC client")?;
    let chain_id = json_rpc(&http, &rpc_url, "eth_chainId", json!([])).await?;
    let chain_id = parse_json_rpc_quantity(&chain_id, "chain ID")?;
    if chain_id != ETHEREUM_MAINNET_CHAIN_ID {
        bail!(
            "EVM_RPC_URL must be Ethereum mainnet (chain ID {ETHEREUM_MAINNET_CHAIN_ID}), got {chain_id}"
        );
    }
    let result = json_rpc(
        &http,
        &rpc_url,
        "eth_call",
        json!([{
            "to": format!("{gateway:#x}"),
            "data": format!("0x{}", hex::encode(calldata)),
        }, "latest"]),
    )
    .await?;
    let return_data = result
        .as_str()
        .context("eth_call result was not hex data")?;
    if return_data != "0x" {
        bail!("successful void verifier call must return 0x, got {return_data}");
    }

    println!("EVM verification simulation: true (eth_call did not revert)");
    println!("EVM transaction broadcast: no");
    println!("chain: Ethereum mainnet ({chain_id})");
    println!("Groth16 gateway: {gateway:#x}");
    println!("program vkey: 0x{}", hex::encode(vkey));
    println!("public values: {}", hex::encode(expected));
    Ok(())
}

async fn json_rpc(
    client: &reqwest::Client,
    rpc_url: &str,
    method: &str,
    params: Value,
) -> Result<Value> {
    let response = client
        .post(rpc_url)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        }))
        .send()
        .await
        .with_context(|| format!("sending {method} JSON-RPC request"))?
        .error_for_status()
        .with_context(|| format!("{method} JSON-RPC HTTP failure"))?;
    let body: Value = response
        .json()
        .await
        .with_context(|| format!("decoding {method} JSON-RPC response"))?;
    if let Some(error) = body.get("error") {
        bail!("{method} JSON-RPC error: {error}");
    }
    body.get("result")
        .cloned()
        .with_context(|| format!("{method} JSON-RPC response omitted result"))
}

fn verify_and_save(
    prover: &NetworkProver,
    pk: &sp1_sdk::SP1ProvingKey,
    proof: &SP1ProofWithPublicValues,
    expected: &[u8; 32],
    proof_path: &Path,
) -> Result<()> {
    if proof.public_values.as_slice() != expected {
        bail!(
            "proof public values mismatch: expected {}, got {}",
            hex::encode(expected),
            hex::encode(proof.public_values.as_slice())
        );
    }
    prover
        .verify(proof, pk.verifying_key(), None)
        .context("verifying downloaded Groth16 proof with SP1")?;
    ensure_parent(proof_path)?;
    proof
        .save(proof_path)
        .with_context(|| format!("saving proof to {}", proof_path.display()))?;
    println!("SP1 verification: passed");
    println!(
        "public values: {}",
        hex::encode(proof.public_values.as_slice())
    );
    println!("onchain proof bytes: {}", hex::encode(proof.bytes()));
    println!("saved proof: {}", proof_path.display());
    Ok(())
}

async fn query_quote(client: &NetworkClient, pgu_limit: u64) -> Result<Quote> {
    let balance = client
        .get_balance()
        .await
        .context("querying requester balance")?;
    let balance_wei = balance
        .to_string()
        .parse::<u128>()
        .context("requester balance does not fit u128")?;
    let market = client
        .get_market_price_per_pgu()
        .await
        .context("querying current PGU market price")?;
    let params = client
        .get_proof_request_params(SP1ProofMode::Groth16.into())
        .await
        .context("querying Groth16 request parameters")?;
    let ParamsResponse::Auction(params) = params else {
        bail!("mainnet RPC did not return auction parameters");
    };

    let raw_price = parse_u64(&params.max_price_per_pgu, "max price per PGU")?;
    let sdk_price_cap = align_to_tick(raw_price.saturating_mul(120) / 100, params.tick_size);
    let base_fee = parse_u64(&params.base_fee, "Groth16 base fee")?;
    let cost_cap = u128::from(base_fee)
        .checked_add(u128::from(sdk_price_cap) * u128::from(pgu_limit))
        .context("quote cap overflow")?;
    Ok(Quote {
        balance_wei,
        market_price: market.wei,
        market_as_of: market.as_of,
        base_fee,
        raw_price,
        tick_size: params.tick_size,
        sdk_price_cap,
        cost_cap,
    })
}

fn print_quote(quote: &Quote, pgu_limit: u64) {
    println!("balance (PROVE wei): {}", quote.balance_wei);
    println!("balance (PROVE): {}", format_prove(quote.balance_wei));
    println!("market price (PROVE wei/PGU): {}", quote.market_price);
    println!("market price timestamp: {}", quote.market_as_of);
    println!("Groth16 base fee (PROVE wei): {}", quote.base_fee);
    println!("RPC max price (PROVE wei/PGU): {}", quote.raw_price);
    println!("auction tick (PROVE wei/PGU): {}", quote.tick_size);
    println!(
        "SDK 120% price cap (PROVE wei/PGU): {}",
        quote.sdk_price_cap
    );
    println!("quoted PGU limit: {pgu_limit}");
    println!("maximum request cost (PROVE wei): {}", quote.cost_cap);
    println!(
        "maximum request cost (PROVE): {}",
        format_prove(quote.cost_cap)
    );
}

fn read_signer(path: &Path) -> Result<NetworkSigner> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("reading private-key metadata from {}", path.display()))?;
    if !metadata.is_file() {
        bail!("private-key path is not a file: {}", path.display());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            bail!(
                "private-key file must not be accessible by group or others: {}",
                path.display()
            );
        }
    }
    let private_key = fs::read_to_string(path)
        .with_context(|| format!("reading private key from {}", path.display()))?;
    NetworkSigner::local(private_key.trim()).context("parsing requester private key")
}

fn validate_input(input: &ProofInput) -> Result<()> {
    if input.network != "monero-mainnet" {
        bail!("unsupported network: {}", input.network);
    }
    let block_id = decode_32(input.block_id, "block ID")?;
    decode_32(input.prev_hash, "previous block hash")?;
    decode_32(input.seed_hash, "RandomX seed hash")?;
    let pow_hash = decode_32(input.pow_hash, "PoW hash")?;
    let blob = hex::decode(input.hashing_blob).context("decoding hashing blob")?;
    if blob.len() != 77 {
        bail!(
            "selected Monero hashing blob must be 77 bytes, got {}",
            blob.len()
        );
    }
    let mut serialized_blob = Vec::with_capacity(blob.len() + 10);
    append_varint(blob.len() as u64, &mut serialized_blob);
    serialized_blob.extend_from_slice(&blob);
    if keccak256(&serialized_blob).as_slice() != block_id {
        bail!("hashing blob does not produce the declared Monero block ID");
    }
    let expected_seed_height = randomx_seed_height(input.height);
    if input.seed_height != expected_seed_height {
        bail!(
            "wrong RandomX seed height: expected {expected_seed_height}, got {}",
            input.seed_height
        );
    }
    let difficulty = parse_difficulty(input.wide_difficulty)?;
    if !meets_difficulty(&pow_hash, difficulty) {
        bail!("PoW hash does not meet the declared Monero difficulty");
    }
    if input.request_timeout_seconds < MIN_REQUEST_TIMEOUT_SECS {
        bail!("request timeout must be at least {MIN_REQUEST_TIMEOUT_SECS} seconds");
    }
    println!(
        "Monero block: {} ({}, timestamp {})",
        input.height, input.block_id, input.timestamp
    );
    println!("RandomX seed height: {}", input.seed_height);
    println!("cycle limit: {}", input.cycle_limit);
    println!("gas limit: {}", input.gas_limit);
    Ok(())
}

fn validate_elf(elf: &[u8]) -> Result<()> {
    let expected = APPROVED_ELF_SHA256.context(
        "no ELF identity is approved; complete and record the reproducible build before proving",
    )?;
    let actual = hex::encode(Sha256::digest(elf));
    let expected = expected.trim_start_matches("0x");
    if actual != expected {
        bail!("ELF SHA-256 mismatch: expected {expected}, got {actual}");
    }
    Ok(())
}

fn randomx_seed_height(height: u64) -> u64 {
    const EPOCH_BLOCKS: u64 = 2048;
    const SEED_LAG: u64 = 64;
    height
        .saturating_sub(SEED_LAG)
        .checked_div(EPOCH_BLOCKS)
        .unwrap()
        * EPOCH_BLOCKS
}

fn parse_difficulty(value: &str) -> Result<u64> {
    let digits = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    u64::from_str_radix(digits, 16).with_context(|| format!("parsing difficulty: {value}"))
}

fn meets_difficulty(hash: &[u8; 32], difficulty: u64) -> bool {
    let mut carry = 0u128;
    for chunk in hash.chunks_exact(8) {
        let limb = u64::from_le_bytes(chunk.try_into().unwrap());
        let product = limb as u128 * difficulty as u128 + carry;
        carry = product >> 64;
    }
    carry == 0
}

fn append_varint(mut value: u64, output: &mut Vec<u8>) {
    while value >= 0x80 {
        output.push((value as u8) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

fn decode_32(value: &str, name: &str) -> Result<[u8; 32]> {
    let digits = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    let bytes = hex::decode(digits).with_context(|| format!("decoding {name}"))?;
    bytes
        .try_into()
        .map_err(|bytes: Vec<u8>| anyhow::anyhow!("{name} must be 32 bytes, got {}", bytes.len()))
}

fn parse_u64(value: &str, name: &str) -> Result<u64> {
    value
        .parse()
        .with_context(|| format!("parsing {name}: {value}"))
}

fn parse_json_rpc_quantity(value: &Value, name: &str) -> Result<u64> {
    let text = value
        .as_str()
        .with_context(|| format!("{name} JSON-RPC result was not a string"))?;
    let digits = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .with_context(|| format!("{name} JSON-RPC result was not a hex quantity: {text}"))?;
    if digits.is_empty() {
        bail!("{name} JSON-RPC result was an empty hex quantity");
    }
    u64::from_str_radix(digits, 16)
        .with_context(|| format!("parsing {name} JSON-RPC quantity: {text}"))
}

fn align_to_tick(value: u64, tick: u64) -> u64 {
    if tick <= 1 {
        value
    } else {
        value - value % tick
    }
}

fn format_prove(wei: u128) -> String {
    let whole = wei / PROVE_WEI;
    let fractional = wei % PROVE_WEI;
    format!("{whole}.{fractional:018}")
}

fn write_text(path: &Path, contents: &str) -> Result<()> {
    ensure_parent(path)?;
    fs::write(path, contents).with_context(|| format!("writing {}", path.display()))
}

fn write_or_verify_text(path: &Path, value: &str, name: &str) -> Result<()> {
    if path.exists() {
        let existing = fs::read_to_string(path)
            .with_context(|| format!("reading existing {name} from {}", path.display()))?;
        if existing.trim() != value {
            bail!(
                "existing {name} in {} does not match computed value",
                path.display()
            );
        }
        return Ok(());
    }
    write_text(path, &format!("{value}\n"))
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating output directory {}", parent.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_monero_input_is_self_consistent() {
        validate_input(&SELECTED_BLOCK).unwrap();
    }

    #[test]
    fn proving_is_disabled_without_an_approved_elf() {
        assert!(validate_elf(b"not an ELF").is_err());
    }

    #[test]
    fn verifier_call_uses_the_canonical_selector() {
        assert_eq!(
            hex::encode(ISP1Verifier::verifyProofCall::SELECTOR),
            "41493c60"
        );
    }

    #[test]
    fn ethereum_chain_id_quantity_is_mainnet() {
        assert_eq!(
            parse_json_rpc_quantity(&Value::String("0x1".to_owned()), "chain ID").unwrap(),
            ETHEREUM_MAINNET_CHAIN_ID
        );
        assert!(parse_json_rpc_quantity(&Value::String("1".to_owned()), "chain ID").is_err());
    }

    #[test]
    fn fixed_width_hex_accepts_standard_prefix() {
        let expected = [0xabu8; 32];
        assert_eq!(decode_32(&hex::encode(expected), "test").unwrap(), expected);
        assert_eq!(
            decode_32(&format!("0x{}", hex::encode(expected)), "test").unwrap(),
            expected
        );
    }

    #[test]
    fn duplicate_vkey_must_match() {
        let path = env::temp_dir().join(format!(
            "randomx-sp1-network-prover-vkey-{}",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        write_or_verify_text(&path, "0x01", "program vkey").unwrap();
        write_or_verify_text(&path, "0x01", "program vkey").unwrap();
        assert!(write_or_verify_text(&path, "0x02", "program vkey").is_err());
        fs::remove_file(path).unwrap();
    }
}
