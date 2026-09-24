#!/usr/bin/env bash
set -euo pipefail

# Reviewed 2026-09-24: keep platform write tests intact for a separately
# authorized machine stage. Linux GPU release may write sysfs on AMD hosts.
# All remaining workspace targets run with their normal assertions.
cd "$(dirname "$0")/../.."

cargo test --workspace --all-targets --locked -- \
  --skip linux_provider_collects_snapshot \
  --skip get_affinity_for_self_returns_at_least_one_core \
  --skip set_affinity_for_self_succeeds \
  --skip set_affinity_rejects_empty_mask \
  --skip set_affinity_rejects_out_of_range_core \
  --skip affinity_returns_contract_error_on_bad_pid \
  --skip controller_handles_bad_pid_gracefully \
  --skip get_profile_does_not_panic \
  --skip executor_runs_and_reports_outcomes \
  --skip full_loop_runs_and_records_metrics \
  --skip history_disabled_by_default_means_no_records \
  --skip history_enabled_records_tick \
  --skip loop_starts_in_normal_mode \
  --skip smoke_runs_without_panic \
  --skip smoke_trace_ids_unique \
  --skip smoke_mode_is_valid_enum_value \
  --skip release_does_not_panic \
  --skip linux_unknown_device_not_supported \
  --skip linux_bad_id_not_supported \
  --skip l0_observe_returns_snapshot \
  --skip l0_real_developer_workload_inferred \
  --skip l0_real_cpu_workload_observed \
  --skip l0_three_workload_evidence_recorded \
  --skip collect_then_normalize \
  --skip collect_returns_snapshot_without_panic \
  --skip temperature_is_optional \
  --skip gpu_is_optional \
  --skip npu_is_optional \
  --skip linux_reports_extended_capabilities \
  --skip two_collects_return_different_timestamps \
  --skip collected_at_is_set \
  --skip snapshot_has_cpu_cores \
  --skip snapshot_has_memory \
  --skip snapshot_has_disks_or_is_empty \
  --skip telemetry_to_context_pipeline \
  --skip telemetry_to_context_to_policy_to_validator \
  --skip full_pipeline_with_ai_assisted_policy \
  --skip full_pipeline_without_ai_still_works \
  --skip linux_enumerates_gpu_devices_without_panic \
  --skip linux_sample_gpu_returns_result_or_error \
  --skip linux_sample_unknown_gpu_fails \
  --skip linux_unknown_gpu_unsupported_identity \
  --skip linux_unknown_npu_unsupported_identity
