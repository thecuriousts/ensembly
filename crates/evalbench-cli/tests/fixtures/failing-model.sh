#!/bin/sh
# Synthetic model for tests: fails on t03-stripped.
cat >/dev/null
if [ "$EVALBENCH_TASK_ID" = "t03-stripped" ]; then
  echo "synthetic outage" >&2
  exit 3
fi
printf '{"task_id":"%s","model_id":"synthetic-exec-model","text":"{\\"gaps\\":[]}","tokens_in":100,"tokens_out":1,"cost_micro_usd":0}\n' "$EVALBENCH_TASK_ID"
