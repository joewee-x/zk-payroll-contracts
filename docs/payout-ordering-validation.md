# Payout Ordering Validation (#815)

Payroll operators can commit a canonical recipient ordering for a run nonce
before execution. When a commitment exists, `prepare_payroll_run` and
`batch_process_payroll*` reject any payout array that does not match that
ordering exactly (same addresses in the same positions).

The guard is **opt-in**: batches with no prior commitment execute unchanged.

## Why this matters

Without ordering validation, a rogue operator or a software bug could silently
reorder recipients between approval and execution, sending payments to the
wrong destinations. Committing the ordering on-chain closes that gap.

## Entry points

| Function | Mutates? | Behavior |
| --- | --- | --- |
| `commit_recipient_ordering(admin, nonce, recipients)` | Yes | Stores the canonical ordering for `nonce`. Admin-only. Rejects empty lists, oversized batches, and duplicate commitments for the same nonce. |
| `check_payout_ordering(nonce, payout_recipients)` | No | Read-only preflight. Returns a `PayoutOrderingCheck` snapshot. Does **not** consume the commitment. |
| Execution paths (`prepare_payroll_run`, `batch_process_payroll*`) | Yes (consumes) | When a commitment exists, asserts positional equality then deletes the commitment. |

For `batch_process_payroll_bounded`, the check runs only on the first chunk
(`start_index == 0`) so the commitment is not consumed mid-resume.

## Errors

| Code | Name | Meaning |
| --- | --- | --- |
| `8150` | `EmptyRecipients` | Commitment list is empty. |
| `8151` | `TooManyRecipients` | Commitment exceeds `MAX_ORDERING_SIZE` (50). |
| `8152` | `CommitmentAlreadyExists` | A commitment for this nonce already exists. |
| `8153` | `NoCommitmentFound` | Preflight only: no commitment for this nonce. |
| `8154` | `LengthMismatch` | Payout length differs from the commitment. |
| `8155` | `OrderMismatch` | At least one recipient is in the wrong position. |

Failure messages report only indices and counts — never addresses, salaries, or
proof material.

## Events

| Topics | Data | When |
| --- | --- | --- |
| `("payroll", "ordering_committed")` | `(nonce_ref, count, committed_at)` | After a successful commit. |
| `("payroll", "ordering_validated")` | `(nonce_ref, count, validated_at)` | After a successful match at execution. |

`nonce_ref` is the first 8 bytes of the nonce interpreted as a big-endian
`u64`, so the raw nonce is not surfaced in events.

## Privacy

* Storage holds only ordered opaque addresses — no amounts or proofs.
* Events carry only an opaque nonce reference, a count, and a timestamp.
* Errors never include recipient addresses.
