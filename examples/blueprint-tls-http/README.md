# yaoi blueprint TLS HTTP Server example

yaoi Example of Blueprint with TLS and HTTP Server on top of it

It requires Linux kernel 6.12 or newer and free 2MB HugeTLBs.

# Usage

Generate the required TLS certificate & key material
```ignore
cd test_certs; make prime256v1
```

Cargo run
```ignore
cargo run --release
```

## Continuous Buffers

We leverage continuous ring buffers (on HugeTLB) which require a kernel version 6.12 or newer.

Earlier versions of the kernel consumed whole buffer when a single read was completed where as continuous reading re-uses the buffer until it is full before it will be recycled.

## Hugepages (HugeTLB)

This example leverages and thus requires 2MB Linux hugepages (HugeTLB)

You should allocate them ideally near system startup:
```
echo 30 > /proc/sys/vm/nr_hugepages
```

If the system has been running for long the memory may be too fragmented.

Appropriate number depends on concurrent clients required and other workloads present.
