use serde::{Deserialize, Serialize};
use tiny_keccak::{Hasher, Keccak};

pub const HASH_BYTES: usize = 32;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RpcBlockRecord {
    pub height: u64,
    pub block_id: String,
    pub major_version: u64,
    pub minor_version: u64,
    pub timestamp: u64,
    pub prev_hash: String,
    pub nonce: u32,
    pub miner_tx_hash: String,
    pub tx_hashes: Vec<String>,
    pub wide_difficulty: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MoneroBlockFixtures {
    pub network: String,
    pub rpc_endpoint: String,
    pub seed_height: u64,
    pub seed_hash: String,
    pub blocks: Vec<MoneroBlockFixture>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MoneroBlockFixture {
    pub height: u64,
    pub block_id: String,
    pub prev_hash: String,
    pub timestamp: u64,
    pub wide_difficulty: String,
    pub hashing_blob: String,
    pub pow_hash: String,
    pub cfround_counts: [u64; 4],
}

pub fn decode_hex<const N: usize>(value: &str) -> Result<[u8; N], String> {
    if value.len() != N * 2 {
        return Err(format!(
            "expected {} hex characters, got {}",
            N * 2,
            value.len()
        ));
    }

    let mut bytes = [0u8; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = u8::from_str_radix(&value[offset..offset + 2], 16)
            .map_err(|_| format!("invalid hex at byte {index}: {value}"))?;
    }
    Ok(bytes)
}

pub fn decode_hex_vec(value: &str) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 {
        return Err(format!("hex string has odd length: {}", value.len()));
    }

    value
        .as_bytes()
        .chunks_exact(2)
        .enumerate()
        .map(|(index, pair)| {
            let pair = std::str::from_utf8(pair).expect("hex input is ASCII-compatible");
            u8::from_str_radix(pair, 16)
                .map_err(|_| format!("invalid hex at byte {index}: {value}"))
        })
        .collect()
}

pub fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

pub fn cn_fast_hash(input: &[u8]) -> [u8; HASH_BYTES] {
    let mut hasher = Keccak::v256();
    let mut output = [0u8; HASH_BYTES];
    hasher.update(input);
    hasher.finalize(&mut output);
    output
}

/// Hashes a `blobdata` as Monero's `get_object_hash(blobdata)` does. The
/// binary serializer prefixes the raw blob with its varint byte length.
pub fn blob_object_hash(blob: &[u8]) -> [u8; HASH_BYTES] {
    let mut serialized = Vec::with_capacity(blob.len() + 10);
    append_varint(blob.len() as u64, &mut serialized);
    serialized.extend_from_slice(blob);
    cn_fast_hash(&serialized)
}

/// Monero's 64-bit-difficulty PoW check: the little-endian 256-bit hash times
/// the difficulty must not overflow 256 bits.
pub fn meets_difficulty(hash: &[u8; HASH_BYTES], difficulty: u64) -> bool {
    let mut carry = 0u128;
    for chunk in hash.chunks_exact(8) {
        let limb = u64::from_le_bytes(chunk.try_into().unwrap());
        let product = limb as u128 * difficulty as u128 + carry;
        carry = product >> 64;
    }
    carry == 0
}

pub fn parse_wide_difficulty(value: &str) -> Result<u64, String> {
    let digits = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    u64::from_str_radix(digits, 16).map_err(|_| format!("invalid 64-bit difficulty: {value}"))
}

fn hash_pair(left: &[u8; HASH_BYTES], right: &[u8; HASH_BYTES]) -> [u8; HASH_BYTES] {
    let mut pair = [0u8; HASH_BYTES * 2];
    pair[..HASH_BYTES].copy_from_slice(left);
    pair[HASH_BYTES..].copy_from_slice(right);
    cn_fast_hash(&pair)
}

/// Monero/CryptoNote tree hash, including its non-power-of-two layout.
pub fn tree_hash(hashes: &[[u8; HASH_BYTES]]) -> [u8; HASH_BYTES] {
    assert!(
        !hashes.is_empty(),
        "a block must contain a miner transaction"
    );
    match hashes.len() {
        1 => return hashes[0],
        2 => return hash_pair(&hashes[0], &hashes[1]),
        _ => {}
    }

    let count = hashes.len();
    let mut level_len = 1usize;
    while level_len * 2 < count {
        level_len *= 2;
    }

    let direct = 2 * level_len - count;
    let mut level = vec![[0u8; HASH_BYTES]; level_len];
    level[..direct].copy_from_slice(&hashes[..direct]);

    let mut source = direct;
    for destination in direct..level_len {
        level[destination] = hash_pair(&hashes[source], &hashes[source + 1]);
        source += 2;
    }
    assert_eq!(source, count);

    while level_len > 2 {
        level_len /= 2;
        for destination in 0..level_len {
            level[destination] = hash_pair(&level[2 * destination], &level[2 * destination + 1]);
        }
    }
    hash_pair(&level[0], &level[1])
}

pub fn append_varint(mut value: u64, output: &mut Vec<u8>) {
    while value >= 0x80 {
        output.push((value as u8) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
}

/// Reconstructs `cryptonote::get_block_hashing_blob` from an RPC block record.
pub fn hashing_blob(block: &RpcBlockRecord) -> Result<Vec<u8>, String> {
    let mut tx_hashes = Vec::with_capacity(block.tx_hashes.len() + 1);
    tx_hashes.push(decode_hex::<HASH_BYTES>(&block.miner_tx_hash)?);
    for hash in &block.tx_hashes {
        tx_hashes.push(decode_hex::<HASH_BYTES>(hash)?);
    }

    let mut blob = Vec::with_capacity(80);
    append_varint(block.major_version, &mut blob);
    append_varint(block.minor_version, &mut blob);
    append_varint(block.timestamp, &mut blob);
    blob.extend_from_slice(&decode_hex::<HASH_BYTES>(&block.prev_hash)?);
    blob.extend_from_slice(&block.nonce.to_le_bytes());
    blob.extend_from_slice(&tree_hash(&tx_hashes));
    append_varint(tx_hashes.len() as u64, &mut blob);
    Ok(blob)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_boundaries() {
        let cases = [
            (0, "00"),
            (127, "7f"),
            (128, "8001"),
            (16_383, "ff7f"),
            (16_384, "808001"),
            (u64::MAX, "ffffffffffffffffff01"),
        ];
        for (value, expected) in cases {
            let mut encoded = Vec::new();
            append_varint(value, &mut encoded);
            assert_eq!(encode_hex(&encoded), expected);
        }
    }

    #[test]
    fn tree_hash_shapes() {
        let hashes: Vec<_> = (0..5).map(|value| [value; HASH_BYTES]).collect();
        assert_eq!(tree_hash(&hashes[..1]), hashes[0]);
        assert_eq!(tree_hash(&hashes[..2]), hash_pair(&hashes[0], &hashes[1]));

        let pair_12 = hash_pair(&hashes[1], &hashes[2]);
        assert_eq!(tree_hash(&hashes[..3]), hash_pair(&hashes[0], &pair_12));

        let pair_01 = hash_pair(&hashes[0], &hashes[1]);
        let pair_23 = hash_pair(&hashes[2], &hashes[3]);
        assert_eq!(tree_hash(&hashes[..4]), hash_pair(&pair_01, &pair_23));

        let pair_34 = hash_pair(&hashes[3], &hashes[4]);
        let upper_right = hash_pair(&hashes[2], &pair_34);
        assert_eq!(tree_hash(&hashes), hash_pair(&pair_01, &upper_right));
    }

    #[test]
    fn difficulty_checks_full_256_bit_product() {
        assert!(meets_difficulty(&[0; HASH_BYTES], u64::MAX));
        assert!(meets_difficulty(&[u8::MAX; HASH_BYTES], 1));
        assert!(!meets_difficulty(&[u8::MAX; HASH_BYTES], 2));

        let mut high_limb = [0u8; HASH_BYTES];
        high_limb[24..].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(!meets_difficulty(&high_limb, 2));
    }
}
