#!/bin/sh
# Synthetic model for tests: always answers no gaps. Offline, no keys.
cat >/dev/null
printf '{"task_id":"%s","model_id":"synthetic-exec-model","text":"{\\"gaps\\":[]}","tokens_in":100,"tokens_out":1,"cost_micro_usd":0}\n' "$EVALBENCH_TASK_ID"
