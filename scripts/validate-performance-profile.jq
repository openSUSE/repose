# Structural validator for the P0.4 profile-manifest contract
# (tests/performance/README.md documents each field).
#
# Usage: jq -e -f scripts/validate-performance-profile.jq manifest.json
#
# Checks shape and provenance only: that ranked hotspots trace back to a
# named tool/command/artifact, not that the numbers are good. Rejects a
# manifest with no captured classes (every class must be either present
# with data, or explicitly `"status": "skipped"` with a `"reason"`).

def check(val; pred; msg):
  if (val | pred) then . else error("invalid profile manifest: " + msg) end;

def is_nonempty_string: type == "string" and length > 0;
def is_nonneg_int: type == "number" and (. == (. | floor)) and . >= 0;

def check_class(name):
  . as $c
  | if ($c.status == "skipped") then
      check($c.reason; is_nonempty_string; name + ".reason required when skipped")
    else
      check($c.tool; is_nonempty_string; name + ".tool")
      | check($c.raw_artifact; is_nonempty_string; name + ".raw_artifact path")
    end;

. as $m
| check($m.contract_version; . == 1; "contract_version must be 1")
| check($m.workload_id; is_nonempty_string; "workload_id")
| check($m.platform; type == "object"; "platform object")
| check($m.platform.os; is_nonempty_string; "platform.os")
| check($m.platform.arch; is_nonempty_string; "platform.arch")
| check($m.measured_command; is_nonempty_string; "measured_command")
| check($m.measured_pid; is_nonneg_int; "measured_pid")
| check($m.setup_excluded; type == "boolean"; "setup_excluded boolean")
| check($m.generated_at; is_nonempty_string; "generated_at")
| check($m.classes; type == "object" and (length > 0); "classes: at least one profile class")
| ($m.classes | to_entries[] | . as $entry | ($entry.value | check_class($entry.key)))
| .
