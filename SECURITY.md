# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.1.x   | ✅ Yes             |
| < 0.1   | ❌ No              |

## Reporting a Vulnerability

**Please do NOT report security vulnerabilities through public GitHub issues.**

If you discover a security vulnerability in rlsl, please report it responsibly:

1. **Email**: Send a description to the maintainers at **security@rlsl.org**
2. **Include**:
   - Description of the vulnerability
   - Steps to reproduce or proof of concept
   - Affected versions
   - Potential impact assessment
   - Suggested fix (if any)

## Response Timeline

- **Acknowledgment**: Within 48 hours of report
- **Initial assessment**: Within 1 week
- **Fix development**: Within 2 weeks for critical issues
- **Disclosure**: Coordinated with reporter; typically 90 days

## Scope

The following are in scope for security reports:

- **Protocol vulnerabilities**: Malformed packets causing crashes, memory corruption,
  or denial of service in the LSL protocol parser (`sample.rs`, `tcp_server.rs`, `udp_server.rs`)
- **Buffer overflows**: In sample deserialization, XML parsing, or raw data handling
- **Denial of service**: Resource exhaustion via crafted network traffic
- **C ABI safety**: Undefined behavior in `rlsl-sys` extern functions
- **Python binding safety**: Memory safety issues in `rlsl-py` PyO3 bindings
- **WASM bridge**: WebSocket message handling vulnerabilities

## Security Considerations

### Network Protocol

rlsl implements the LSL protocol which is designed for **local network** (LAN)
use in research environments. It does **not** provide:

- Authentication or authorization
- Encryption of data in transit
- Protection against man-in-the-middle attacks

**Do not expose LSL streams to untrusted networks.**

### C ABI (`rlsl-sys`)

The C ABI layer uses `unsafe` Rust to interface with C callers. All pointer
parameters are validated for null, but callers must ensure:

- Buffer sizes match declared lengths
- Handles are not used after `lsl_destroy_*`
- Thread-safety requirements are respected

### Fuzzing

We maintain fuzz targets in `crates/rlsl-fuzz/` covering:

- Protocol 1.00 and 1.10 sample deserialization
- XML DOM parsing
- StreamInfo query matching
- XDF chunk parsing

Run fuzz tests with:
```sh
cargo +nightly fuzz run fuzz_sample_110 -- -max_total_time=300
```

## Acknowledgments

We gratefully acknowledge security researchers who responsibly disclose vulnerabilities.
Contributors will be credited in the CHANGELOG (with permission).
