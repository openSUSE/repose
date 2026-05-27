from typing import Literal, TypeAlias

# Process exit code aggregated from per-target task results.
#
# 0 — all targets succeeded.
# 1 — partial failure: at least one target succeeded and at least one
#     target failed.
# 2 — hard failure: every target failed (or no targets were reachable
#     once the run reached aggregation).
ExitCode: TypeAlias = Literal[0, 1, 2]
