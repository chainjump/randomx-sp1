#include "aes_hash.hpp"
#include "blake2/blake2.h"
#include "program.hpp"

#include <array>
#include <cstdint>
#include <cstdio>
#include <cstring>

namespace {

constexpr std::array<std::uint8_t, 76> kBaseBlob = {
    0x10, 0x10, 0xc5, 0xa2, 0x99, 0xd3, 0x06, 0x5e, 0xd0, 0x66, 0x57, 0x3b, 0x62,
    0xcd, 0xcc, 0x0d, 0x24, 0x3d, 0x8b, 0x71, 0x30, 0xcf, 0x8b, 0xe8, 0x7f, 0xf7,
    0x1e, 0xc3, 0x02, 0xce, 0xdd, 0x31, 0xdb, 0x9f, 0x6f, 0x4f, 0x6e, 0x10, 0xe8,
    0x5d, 0x5a, 0x4c, 0x10, 0x76, 0xf9, 0xef, 0x57, 0xaa, 0xbb, 0x92, 0x00, 0x4f,
    0xaf, 0xeb, 0xc6, 0x8b, 0x9a, 0x54, 0xbc, 0x9d, 0x35, 0x84, 0xec, 0x8f, 0x94,
    0x3e, 0x94, 0x9b, 0xc4, 0xc3, 0x72, 0xa5, 0xf3, 0xb4, 0xe6, 0x1d,
};

void print_hex(const std::uint8_t* bytes, std::size_t size) {
    for (std::size_t i = 0; i < size; ++i) std::printf("%02x", bytes[i]);
}

} // namespace

int main() {
    for (std::uint32_t nonce = 0; nonce < 100000; ++nonce) {
        auto blob = kBaseBlob;
        std::memcpy(blob.data() + blob.size() - sizeof(nonce), &nonce, sizeof(nonce));

        alignas(16) std::uint64_t seed[8];
        if (blake2b(seed, sizeof(seed), blob.data(), blob.size(), nullptr, 0) != 0) return 2;
        randomx::Program program;
        fillAes4Rx4<false>(seed, sizeof(program), &program);

        for (int pc = 0; pc < RANDOMX_PROGRAM_SIZE; ++pc) {
            const auto& instruction = program(pc);
            if (instruction.opcode == 0xef) {
                std::printf("nonce=%u pc=%d src=%u imm=0x%08x blob=", nonce, pc,
                    instruction.src, instruction.getImm32());
                print_hex(blob.data(), blob.size());
                std::printf("\n");
                return 0;
            }
        }
    }
    return 1;
}
