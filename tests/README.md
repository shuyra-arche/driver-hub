# Standalone Hub Unit runtime tests

All 65 assertions below are Hub-owned runtime tests in this Unit. They were
copied mechanically from Source `293c15fc97a9d9a4295810d10fe804e4cdec63c4` and
adapted only for the Unit production projection paths. Test names, bodies,
selectors, fixtures, and assertion values are unchanged. All five suite
directories are standalone host workspaces excluded from the runtime workspace.

## Original → Unit file → test names

### `programs/tests/hub-handoff/cases.rs` → `tests/handoff/cases.rs` (14)

`admission_exists_before_embark_is_requested_and_lost_reply_does_not_duplicate`,
`stopping_state_is_not_admitted_and_gets_a_trusted_handoff`,
`trusted_release_errors_and_timeouts_keep_the_same_obligation`,
`target_without_a_launch_authenticates_owner_then_releases`,
`foreign_owner_target_has_no_receipt_and_no_hanging_job`,
`closing_binding_requests_release_before_hardware_finishes`,
`full_return_queue_keeps_responsibility_until_a_slot_is_reliable`,
`closed_expired_and_global_settlement_returns_do_not_prove_cpu_stop`,
`rejected_owner_discovery_keeps_target_proof_while_a_binding_is_open`,
`duplicate_target_does_not_duplicate_the_return`,
`launch_queue_is_bounded_without_consuming_a_report`,
`queue_reservation_is_fallible`,
`authenticated_abort_survives_full_admission_and_return_queues`,
`unseen_target_discovery_timeout_does_not_prove_absence`.

### `programs/tests/hub-handoff/serve/adapter_cases.rs` → `tests/handoff/serve/adapter_cases.rs` (7)

`unrelated_binding_progresses_while_one_driver_reply_is_pending`,
`late_prepare_cannot_reactivate_closing_and_release_waits_for_actual_target`,
`rpc_failure_retries_same_binding_and_first_failure_survives_later_replies`,
`foreign_serial_and_wrong_state_cannot_finish_a_binding`,
`driver_capacity_retry_does_not_displace_the_first_actual_failure`,
`a_full_set_of_pending_bindings_stays_bounded_without_failing`,
`a_matching_key_from_another_source_never_reaches_this_driver`.

### `programs/tests/retention/enrollment.rs` → `tests/retention/enrollment.rs` (12)

`every_registration_boundary_retains_completed_effects_and_hides_pending_devices`,
`lost_replies_reuse_the_same_creation_directory_operation_and_door`,
`repeated_unknown_bind_replies_never_mint_another_door_or_report_ready`,
`conflicting_replays_cannot_change_pending_or_published_registration`,
`pending_devices_share_the_league_and_publish_in_reservation_order`,
`requester_order_cannot_reorder_operator_ids_or_abandon_an_unknown_receipt`,
`capacity_names_and_settlement_are_checked_before_acquiring_resources`,
`machine_entries_have_no_device_identity_and_claim_transitions_preserve_device_origin`,
`zero_source_is_denied_before_any_resource`,
`zero_and_high_key_bits_are_valid_distinct_identities`,
`two_sources_same_key_enroll_claim_and_finish_independently`,
`pending_same_key_retries_exact_source_and_keeps_door_and_ids`.

### `programs/tests/retention/execution.rs` → `tests/retention/execution.rs` (11)

`launch_replay_keeps_identity_and_deadline_without_a_second_record`,
`claim_requires_an_open_admission_and_advances_the_watermark_once`,
`expired_launch_aborts_before_it_can_claim`,
`failed_or_foreign_launch_records_nothing`,
`target_closure_keeps_the_same_binding_and_fault_reason_on_retry`,
`target_without_a_binding_is_resource_done_without_faking_lifecycle`,
`settle_retires_unbound_launches_and_keeps_the_receipt_until_release`,
`target_closure_does_not_close_another_executions_admission`,
`settle_closes_bindings_but_does_not_pretend_the_cpu_stopped`,
`closed_launch_history_is_bounded_and_stops_a_late_replay`,
`admission_capacity_is_bounded_and_full_creates_no_record`.

### `programs/tests/retention/lib.rs` inline `binding` → `tests/retention/lib.rs` (12)

`owner_death_closes_but_does_not_free_the_binding`,
`claim_ids_are_idempotent_and_stale_ids_do_not_revive`,
`ordinary_vacate_does_not_touch_a_dma_binding`,
`close_is_idempotent_and_fault_dominates`,
`first_failure_survives_retries_and_settlement_but_does_not_leak_to_rebinding`,
`settlement_permanently_closes_admission`,
`launch_replay_keeps_deadline_and_settlement_closes_new_launches`,
`repeated_close_does_not_restart_the_stop_chain`,
`closed_history_is_bounded_and_evicted_claims_cannot_revive`,
`binding_serials_continue_beyond_the_former_address_limit_with_bounded_history`,
`history_never_evicts_an_open_binding_or_immediately_discards_its_completion`,
`denied_claims_do_not_advance_the_watermark_and_retired_instances_cannot_reenter`.

### `programs/tests/retention/protocol.rs` → `tests/retention/protocol.rs` (4)

`dma_claim_keeps_its_exact_layout_and_rejects_trailing_bytes`,
`unknown_action_codes_are_refused`,
`close_inspect_settle_round_trip`,
`said_carries_only_binding_progress_source_and_opaque_client`.

### `programs/tests/capability-import/lib.rs` → `tests/capability-import/lib.rs` (5)

`hub_rejects_third_party_and_self_owned_sensors_without_claiming_device`,
`hub_valid_owner_exit_vacates_device_for_next_claim`,
`hub_failed_delivery_preserves_vacant_cell_and_next_request_succeeds`,
`hub_unsupported_resource_kind_and_malformed_kind_do_not_claim`,
`hub_dead_owner_replacement_failure_preserves_old_owner_until_vacate`.

## Projection boundaries

- `handoff/lib.rs` projects `server/src/core/mod.rs`;
  `handoff/serve/mod.rs` projects the launch/return/execution/binding/coordination
  production files.
- `retention/lib.rs` and `capability-import/lib.rs` project
  `server/src/core/mod.rs`.
- `capability-import/hub_serve.rs` projects production
  `server/src/serve/{claim,sweep,grant}.rs`.
- The suites keep their original local transport/resource/System-API fixtures.
  Those fixtures do not transfer ownership of Hub state or assertions to System.
- `tests/hub-api` (9) and `tests/hub-coordination` (10) are the canonical public
  Hub API suites, retained with original bytes. They are distinct from the 65
  runtime assertions, giving 84 distinct cases total. The removed duplicate
  `tests/coordination` copies are not counted or recreated.

## Source retained

`programs/tests/device-recovery` (8 Rust + 126 Python) and the five mixed
`programs/tests/system-shape` selectors remain Source. The removed historical
retention IOMMU ordering/accessor/platform/references selectors are not
recreated.

Driver aggregate intake and Source `Kernel3aa`/`System9001` real DMA/reclaim
acceptance remain pending and Root-owned; host suites are not a substitute for
that integration proof.
