# Labelling template (empty)

This sheet is for the operator's labels. It ships with a header only.
Do not commit filled rows that contain personal data or a real hostname.

## Question (fixed)

Should this posting be labelled apply?

Allowed decisions: `apply` or `skip`. Write a reason tag. Do not use a
blanket apply. Rows without your own decision stay unlabelled; nothing
infers a label.

## Split (already frozen)

The split is not chosen after you label. For each row, the bucket is
`assign_split(link_key)` with seed `e3-2026-11-13`:

- 0–59 train
- 60–79 val
- 80–99 held-out

The threshold is tuned on train (and inspected on val) only. Held-out
precision and accuracy, with a Wilson interval, are reported only when
every held-out row in the file is labelled.

## How to fill

1. Start from a local posting list (gitignored). The CLI
   `ensembly decision template --from POSTINGS.jsonl --out sheet.csv`
   writes the columns below and leaves `decision` and `reason_tag` blank.
2. Fill `decision` (`apply` or `skip`) and `reason_tag`.
3. Set `provenance` to `operator-label`.
4. Keep phone numbers, emails, home paths and real hostnames out of the
   sheet. Privacy and host-alias scans must stay clean if any of this is
   committed. Prefer a local, gitignored file.

Columns: `id`, `link_key`, `company`, `role`, `location`, `url`,
`posting_text`, `split`, `decision`, `reason_tag`, `outcome`, `provenance`.
