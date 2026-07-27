# RustDom-X
A fully rust RandomX implementation without any hardware calls, compatible with all architectures that have rust compiler targets built for them.

## Why?
RandomX is designed to be a CPU first algorithm, it leverages many low level concepts, mostly from the x86_64 architecture. RustDom-X is designed to remove these system calls to be compatible on all devices without any extra compilation. RustDom-X fills a gap, cryptocurrency light nodes, that don't require a fast RandomX hash implementation can use RustDom-X to ensure complete platform compatibility.

## Usage
```rust
use rustdom_x::VmMemory;
use rustdom_x::new_vm;
use std::sync::Arc;

fn main() {
    // Pass in a RandomX key
    let cache = Arc::new(VmMemory::full(b"RandomX\x03")); // or light for a smaller memory footprint
    let mut vm = new_vm(cache);
    
    // Hash some data
    let hash = vm.compute_hash(b"some data");
    println("{}", hash.to_hex());
}
```

## Credit
This project is mostly forked from @Ragnaroek/mithril, removing all architecture specific syscalls

## VC
Available primarily on GitHub and Crates.io
https://github.com/snap-coin/rustdom-x
https://crates.io/rustdom-x
