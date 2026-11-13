# ensembly-opportunity-finder

Optional pack: application ledger, company audit gate, tracker export, and `record apps` import/compare.

The default `ensembly-kernel` build does not include this code. Enable it with:

```bash
cargo build -p ensembly-kernel --features opportunity-finder
```

CLI: `ensembly apps …` and `ensembly record apps …` appear when the binary is built with that feature.

Docs: [`docs/APPLICATION-LEDGER.md`](docs/APPLICATION-LEDGER.md), [`docs/COMPANY-AUDIT.md`](docs/COMPANY-AUDIT.md).
