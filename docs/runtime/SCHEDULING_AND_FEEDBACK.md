# Accelerator admission, preparation and observed feedback

## Accelerator scheduling

`AcceleratorScheduler` is bounded memory-only admission and placement for GPU
and NPU jobs. Backends register discovered `DeviceCapabilityMatrix` values and
explicit concurrency slots; vendor names do not imply support. Jobs are ordered
by priority, earliest expiry and insertion order. Duplicate IDs, exhausted
capacity, expired jobs and invalid hints are rejected. Temperature must be
available and below any configured thermal limit. Admission reserves a slot;
lease drop releases only that admission slot, not hardware state.

`AcceleratorLease::apply` rechecks expiry, matching capability/device permit
and current temperature. A lease dispatches at most once through
`ISafeHardwareActuator`. Model-only permits remain Skipped and absent real
backends remain NotSupported. Production GPU/NPU workload placement and native
thermal/power certification require an independently verified backend; this
framework does not invent one from the fixture's absent devices.

## Application pre-task preparation

The typed IPC `PrepareUpcomingTask { contract, starts_at_unix_ms }` request
requires a registered authenticated application, per-action `prepare_task`
authorization where operator pins are configured, and enabled personalization
consent. Only retained history for that application and UTC hour is considered;
at least three samples and 0.8 pattern confidence are required. A requested
explicit workload must match the pattern. The accepted start horizon is
1..300 seconds. `PreparationScheduled` returns a separate preparation task ID,
predicted workload and ready time (30 seconds before start, clamped to now).

Queued preparation is unavailable before its ready window and expires before
the upcoming start. Cancellation/expiry/termination release its authorization
guard. Immediately before apply, the guard rechecks process identity, application
owner, registration, artifact/SID pins, consent and expiry. Targets are only the
authenticated application process (or explicit model/simulator targets).
`ExecutionTargets.process_only` prevents system power execution, and preparation
does not invoke GPU/NPU runtime binding. Completion describes resource preparation,
not completion or launch of the application's actual task. No application is
started automatically. The fast apply side rechecks task state/deadline after
the immutable slow-side handoff; cancellation cannot reverse an operation
already issued to a platform setter.

## Feedback to policy generation

`FeedbackPolicyEngine` decorates the existing AI policy engine and rule fallback.
Fresh, ordered observed comparisons must identify the last successfully applied
policy to enter `AdaptiveLoop::record_effect`. Model/simulator loops reject this
observed ingestion. Paired metrics are assessed using existing workload/source
consistency and overhead-budget checks. A verified regression suppresses that
same resource-policy shape for its workload for up to one hour and selects the
existing rule fallback; improved evidence for the same shape clears suppression.
IDs/timestamps do not evade shape matching. All subsequent proposals still pass
the policy validator. No priority escalation is inferred from a gain.

This is a trusted in-process collector hook, not an IPC endpoint accepting
client-declared performance values. General automatic per-application workload
collection and energy measurements remain unavailable. The explicit
`owned_process_probe` example exercises a real controlled child workload,
readback, feedback ingestion and priority restoration. Its one comparison is
not a statistically controlled effectiveness or energy benchmark. Its memory
budget uses an observed parent+child endpoint RSS delta in MiB through the
unified `ITelemetryProvider::collect_process` interface, not peak memory. The
Windows adapter implements this targeted read; other collectors return explicit
NotSupported by default. The native example is not executed by `cargo test`.
