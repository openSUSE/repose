# Structural validator for the compact, committed baseline summary
# (tests/performance/baselines/<runner-class>.json): one `runner` object
# shared by every workload it covers (P0.1 step 7 measures `mock` and
# `ssh` alike in the same runner container), plus a `workloads` array of
# comparable-subset entries — see scripts/validate-performance-comparable.jq
# for what each entry must satisfy (no `wall_time_ns`, so no percentile
# recomputation there).
#
# Usage:
#   jq -L scripts -e -f scripts/validate-performance-summary.jq summary.json

include "validate-performance-comparable";

def check(val; pred; msg):
  if (val | pred) then . else error("invalid summary: " + msg) end;

. as $s
| check($s.contract_version; . == 2; "contract_version must be 2")
| check($s.runner; type == "object"; "runner must be an object")
| check($s.generated_at; type == "string" and length > 0; "generated_at must be a non-empty string")
| check($s.workloads; type == "array" and length > 0; "workloads must be a non-empty array")
| check(
    $s.workloads | map(.workload_id);
    . == (. | unique);
    "workloads must have unique workload_id values"
  )
| ($s.workloads | map(. + {runner: $s.runner, contract_version: $s.contract_version} | check_comparable)) as $_
| $s
