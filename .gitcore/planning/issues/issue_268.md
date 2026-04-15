## Context
5 vulnerabilities detected by `cargo audit` in transitive dependencies. All from `rustls-webpki`.

## Vulnerabilidades

| ID | Crate | Severity | Fix |
|----|-------|----------|-----|
| RUSTSEC-2026-0098 | rustls-webpki 0.101.7, 0.102.8 | 🔴 High | >=0.103.12 |
| RUSTSEC-2026-0099 | rustls-webpki 0.101.7, 0.102.8 | 🔴 High | >=0.103.12 |
| RUSTSEC-2026-0049 | rustls-webpki 0.102.8 | 🔴 High | >=0.103.10 |

## Root Cause (Transitive)
```
reqwest 0.11.27 → rustls 0.21.12 → rustls-webpki 0.101.7
reqwest 0.11.27 → rustls 0.21.12 → tokio-rustls 0.24.1 → rustls-webpki 0.101.7
```

Cannot upgrade `reqwest` without breaking changes in `oauth2` crate.

## Tareas
1. [ ] Investigate if `reqwest` can be upgraded to a newer version that pulls fixed `rustls-webpki`
2. [ ] Check if `rustls` version can be pinned to override the transitive dep
3. [ ] Consider using `opentelemetry` or `ureq` as alternative HTTP client
4. [ ] Add `cargo update -p rustls-webpki` to override the vulnerable version if possible

## Links
- https://rustsec.org/advisories/RUSTSEC-2026-0098
- https://rustsec.org/advisories/RUSTSEC-2026-0099
- https://rustsec.org/advisories/RUSTSEC-2026-0049
