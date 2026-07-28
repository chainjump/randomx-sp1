use blake2b_simd::Params;
use rustdom_x::VmMemory;

const MONERO_SEED: [u8; 32] = [
    0x11, 0xc7, 0x98, 0xe5, 0xac, 0x65, 0x15, 0x21, 0x8b, 0xc3, 0xef, 0xcb, 0x54, 0x16, 0xe5, 0xb6,
    0x8c, 0x59, 0x9e, 0x42, 0xa6, 0x1b, 0x86, 0xef, 0xe5, 0x74, 0x6b, 0xb7, 0x8e, 0xb4, 0xbe, 0x8e,
];

fn cache_digest(key: &[u8]) -> String {
    let memory = VmMemory::light(key);
    let mut digest = Params::new().hash_length(32).to_state();
    for block in memory.seed_memory.blocks().iter() {
        digest.update(block.as_u8());
    }
    hex::encode(digest.finalize().as_bytes())
}

#[test]
fn complete_cache_digests_match_frozen_generic_reference() {
    let cases: [(&[u8], &str); 3] = [
        (&MONERO_SEED, "152add6ff4fd241ba703f004dcea77fea6c2d55d8b20100aae1578e7bca88a5c"),
        (&[0u8; 32], "f303edc0c3dc803869f25bb11178193805d767427e11f519bb2ac123ea1ef63e"),
        (&[], "faf16925e389d546a2ebf79d1329ed4f8f217902ba00a5641447773725306d15"),
    ];
    for (key, expected) in cases {
        assert_eq!(cache_digest(key), expected);
    }
}
