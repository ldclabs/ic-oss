import type { Principal } from '@icp-sdk/core/principal';
import type { ActorMethod } from '@icp-sdk/core/agent';
import type { IDL } from '@icp-sdk/core/candid';

export interface AddWasmInput {
  'wasm' : Uint8Array | number[],
  'description' : string,
}
export interface BucketDeploymentInfo {
  'args' : [] | [Uint8Array | number[]],
  'prev_hash' : Uint8Array | number[],
  'error' : [] | [string],
  'deploy_at' : bigint,
  'canister' : Principal,
  'wasm_hash' : Uint8Array | number[],
}
/**
 * # Canister Settings
 * 
 * For arguments of [`create_canister`](https://internetcomputer.org/docs/current/references/ic-interface-spec/#ic-create_canister),
 * [`update_settings`](https://internetcomputer.org/docs/current/references/ic-interface-spec/#ic-update_settings) and
 * [`provisional_create_canister_with_cycles`](https://internetcomputer.org/docs/current/references/ic-interface-spec/#ic-provisional_create_canister_with_cycles).
 * 
 * All fields are `Option` types, allowing selective settings/updates.
 */
export interface CanisterSettings {
  /**
   * Indicates a length of time in seconds.
   * A canister is considered frozen whenever the IC estimates that the canister would be depleted of cycles
   * before `freezing_threshold` seconds pass, given the canister's current size and the IC's current cost for storage.
   * 
   * Must be a number between 0 and 2<sup>64</sup>-1, inclusively.
   * 
   * Default value: `2_592_000` (approximately 30 days).
   */
  'freezing_threshold' : [] | [bigint],
  /**
   * Indicates the threshold on the remaining wasm memory size of the canister in bytes.
   * 
   * If the remaining wasm memory size of the canister is below the threshold, execution of the "on low wasm memory" hook is scheduled.
   * 
   * Must be a number between 0 and 2<sup>64</sup>-1, inclusively.
   * 
   * Default value: `0` (i.e., the "on low wasm memory" hook is never scheduled).
   */
  'wasm_memory_threshold' : [] | [bigint],
  /**
   * A list of environment variables.
   * 
   * These variables are accessible to the canister during execution
   * and can be used to configure canister behavior without code changes.
   * Each key must be unique.
   * 
   * Default value: `null` (i.e., no environment variables provided).
   */
  'environment_variables' : [] | [Array<EnvironmentVariable>],
  /**
   * A list of at most 10 principals.
   * 
   * The principals in this list become the *controllers* of the canister.
   * 
   * Default value: A list containing only the caller of the `create_canister` call.
   */
  'controllers' : [] | [Array<Principal>],
  /**
   * Indicates the upper limit on [`CanisterStatusResult::reserved_cycles`] of the canister.
   * 
   * Must be a number between 0 and 2<sup>128</sup>-1, inclusively.
   * 
   * Default value: `5_000_000_000_000` (5 trillion cycles).
   */
  'reserved_cycles_limit' : [] | [bigint],
  /**
   * Defines who is allowed to read the canister's logs.
   * 
   * Default value: [`LogVisibility::Controllers`].
   */
  'log_visibility' : [] | [LogVisibility],
  /**
   * Indicates the upper limit on the memory used for canister logs (bytes).
   * 
   * Default value: `4096`.
   */
  'log_memory_limit' : [] | [bigint],
  /**
   * Indicates the upper limit on the WASM heap memory (bytes) consumption of the canister.
   * 
   * Must be a number between 0 and 2<sup>48</sup>-1 (i.e 256TB), inclusively.
   * 
   * Default value: `3_221_225_472` (3 GiB).
   */
  'wasm_memory_limit' : [] | [bigint],
  /**
   * Indicates how much memory (bytes) the canister is allowed to use in total.
   * 
   * If the IC cannot provide the requested allocation,
   * for example because it is oversubscribed, the call will be **rejected**.
   * 
   * If set to 0, then memory growth of the canister will be best-effort and subject to the available memory on the IC.
   * 
   * Must be a number between 0 and 2<sup>48</sup> (i.e 256TB), inclusively.
   * 
   * Default value: `0`
   */
  'memory_allocation' : [] | [bigint],
  /**
   * Indicates how much compute power should be guaranteed to this canister,
   * expressed as a percentage of the maximum compute power that a single canister can allocate.
   * 
   * If the IC cannot provide the requested allocation,
   * for example because it is oversubscribed, the call will be **rejected**.
   * 
   * Must be a number between 0 and 100, inclusively.
   * 
   * Default value: `0`
   */
  'compute_allocation' : [] | [bigint],
}
/**
 * # Canister Status Result
 * 
 * Result type of [`canister_status`](https://internetcomputer.org/docs/current/references/ic-interface-spec/#ic-canister_status).
 */
export interface CanisterStatusResult {
  /**
   * The detailed metrics on the memory consumption of the canister.
   */
  'memory_metrics' : MemoryMetrics,
  /**
   * Status of the canister.
   */
  'status' : CanisterStatusType,
  /**
   * The memory size taken by the canister.
   */
  'memory_size' : bigint,
  /**
   * Indicates whether a stopped canister is ready to be migrated to another subnet
   * (i.e., whether it has empty queues and flushed streams).
   */
  'ready_for_migration' : boolean,
  /**
   * The canister version.
   */
  'version' : bigint,
  /**
   * The cycle balance of the canister.
   */
  'cycles' : bigint,
  /**
   * Canister settings in effect.
   */
  'settings' : DefiniteCanisterSettings,
  /**
   * Query statistics.
   */
  'query_stats' : QueryStats,
  /**
   * Amount of cycles burned per day.
   */
  'idle_cycles_burned_per_day' : bigint,
  /**
   * A SHA256 hash of the module installed on the canister. This is null if the canister is empty.
   */
  'module_hash' : [] | [Uint8Array | number[]],
  /**
   * The reserved cycles balance of the canister.
   * 
   * These are cycles that are reserved by the resource reservation mechanism on storage allocation.
   * See also the [`CanisterSettings::reserved_cycles_limit`] parameter in canister settings.
   */
  'reserved_cycles' : bigint,
}
/**
 * # Canister Status Type
 * 
 * Status of a canister.
 * 
 * See [`CanisterStatusResult::status`].
 */
export type CanisterStatusType = {
    /**
     * The canister is stopped.
     */
    'stopped' : null
  } |
  {
    /**
     * The canister is stopping.
     */
    'stopping' : null
  } |
  {
    /**
     * The canister is running.
     */
    'running' : null
  };
export type ChainArgs = { 'Upgrade' : UpgradeArgs } |
  { 'Init' : InitArgs };
export interface ClusterInfo {
  'ecdsa_token_public_key' : string,
  'schnorr_ed25519_token_public_key' : string,
  'bucket_wasm_total' : bigint,
  'ecdsa_key_name' : string,
  'managers' : Array<Principal>,
  'governance_canister' : [] | [Principal],
  'name' : string,
  'bucket_deployed_total' : bigint,
  'token_expiration' : bigint,
  'weak_ed25519_token_public_key' : string,
  'bucket_latest_version' : Uint8Array | number[],
  'schnorr_key_name' : string,
  'bucket_deployment_logs' : bigint,
  'subject_authz_total' : bigint,
  'committers' : Array<Principal>,
}
/**
 * # Definite Canister Settings
 * 
 * Represents the actual settings in effect.
 * 
 * For return of [`canister_status`](https://internetcomputer.org/docs/current/references/ic-interface-spec/#ic-canister_status).
 */
export interface DefiniteCanisterSettings {
  /**
   * Time in seconds after which the canister is considered frozen.
   */
  'freezing_threshold' : bigint,
  /**
   * Threshold on the remaining wasm memory size of the canister in bytes.
   */
  'wasm_memory_threshold' : bigint,
  /**
   * A list of environment variables.
   */
  'environment_variables' : Array<EnvironmentVariable>,
  /**
   * Controllers of the canister.
   */
  'controllers' : Array<Principal>,
  /**
   * Upper limit on [`CanisterStatusResult::reserved_cycles`] of the canister.
   */
  'reserved_cycles_limit' : bigint,
  /**
   * Visibility of canister logs.
   */
  'log_visibility' : LogVisibility,
  /**
   * Upper limit on the memory used for canister logs (bytes).
   */
  'log_memory_limit' : bigint,
  /**
   * Upper limit on the WASM heap memory (bytes) consumption of the canister.
   */
  'wasm_memory_limit' : bigint,
  /**
   * Total memory (bytes) the canister is allowed to use.
   */
  'memory_allocation' : bigint,
  /**
   * Guaranteed compute allocation as a percentage of the maximum compute power that a single canister can allocate.
   */
  'compute_allocation' : bigint,
}
export interface DeployWasmInput {
  'args' : [] | [Uint8Array | number[]],
  'canister' : Principal,
}
/**
 * # Environment Variable.
 */
export interface EnvironmentVariable {
  /**
   * Value of the environment variable.
   */
  'value' : string,
  /**
   * Name of the environment variable.
   */
  'name' : string,
}
export interface InitArgs {
  'ecdsa_key_name' : string,
  'governance_canister' : [] | [Principal],
  'name' : string,
  'token_expiration' : bigint,
  'bucket_topup_threshold' : bigint,
  'bucket_topup_amount' : bigint,
  'schnorr_key_name' : string,
}
/**
 * # Log Visibility.
 */
export type LogVisibility = {
    /**
     * Controllers.
     */
    'controllers' : null
  } |
  {
    /**
     * Public.
     */
    'public' : null
  } |
  {
    /**
     * Allowed viewers.
     */
    'allowed_viewers' : Array<Principal>
  };
/**
 * # Memory Metrics
 * 
 * Memory metrics of a canister.
 * 
 * See [`CanisterStatusResult::memory_metrics`].
 */
export interface MemoryMetrics {
  /**
   * Represents the memory occupied by the Wasm binary that is currently installed on the canister.
   */
  'wasm_binary_size' : bigint,
  /**
   * Represents the memory used by the canister's log store.
   */
  'log_memory_store_size' : bigint,
  /**
   * Represents the memory used by the Wasm chunk store of the canister.
   */
  'wasm_chunk_store_size' : bigint,
  /**
   * Represents the memory used for storing the canister's history.
   */
  'canister_history_size' : bigint,
  /**
   * Represents the stable memory usage of the canister.
   */
  'stable_memory_size' : bigint,
  /**
   * Represents the memory consumed by all snapshots that belong to this canister.
   */
  'snapshots_size' : bigint,
  /**
   * Represents the Wasm memory usage of the canister, i.e. the heap memory used by the canister's WebAssembly code.
   */
  'wasm_memory_size' : bigint,
  /**
   * Represents the memory usage of the global variables that the canister is using.
   */
  'global_memory_size' : bigint,
  /**
   * Represents the memory used by custom sections defined by the canister.
   */
  'custom_sections_size' : bigint,
}
/**
 * # Query Stats
 * 
 * Query statistics.
 * 
 * See [`CanisterStatusResult::query_stats`].
 */
export interface QueryStats {
  /**
   * Total number of payload bytes use for query call responses.
   */
  'response_payload_bytes_total' : bigint,
  /**
   * Total number of instructions executed by query calls.
   */
  'num_instructions_total' : bigint,
  /**
   * Total number of query calls.
   */
  'num_calls_total' : bigint,
  /**
   * Total number of payload bytes use for query call requests.
   */
  'request_payload_bytes_total' : bigint,
}
export type Result = { 'Ok' : Uint8Array | number[] } |
  { 'Err' : string };
export type Result_1 = { 'Ok' : null } |
  { 'Err' : string };
export type Result_10 = { 'Ok' : Array<[Principal, string]> } |
  { 'Err' : string };
export type Result_11 = { 'Ok' : string } |
  { 'Err' : string };
export type Result_2 = { 'Ok' : Array<Uint8Array | number[]> } |
  { 'Err' : string };
export type Result_3 = { 'Ok' : Principal } |
  { 'Err' : string };
export type Result_4 = { 'Ok' : bigint } |
  { 'Err' : string };
export type Result_5 = { 'Ok' : Array<BucketDeploymentInfo> } |
  { 'Err' : string };
export type Result_6 = { 'Ok' : WasmInfo } |
  { 'Err' : string };
export type Result_7 = { 'Ok' : Array<Principal> } |
  { 'Err' : string };
export type Result_8 = { 'Ok' : CanisterStatusResult } |
  { 'Err' : string };
export type Result_9 = { 'Ok' : ClusterInfo } |
  { 'Err' : string };
export interface Token {
  'subject' : Principal,
  'audience' : Principal,
  'policies' : string,
}
/**
 * Argument type of [`update_settings`]
 * 
 * # Note
 * 
 * This type is a reduced version of [`ic_management_canister_types::UpdateSettingsArgs`].
 * 
 * The `sender_canister_version` field is removed as it is set automatically in [`update_settings`].
 */
export interface UpdateSettingsArgs {
  /**
   * Canister ID.
   */
  'canister_id' : Principal,
  /**
   * See [`CanisterSettings`].
   */
  'settings' : CanisterSettings,
}
export interface UpgradeArgs {
  'governance_canister' : [] | [Principal],
  'name' : [] | [string],
  'token_expiration' : [] | [bigint],
  'bucket_topup_threshold' : [] | [bigint],
  'bucket_topup_amount' : [] | [bigint],
}
export interface WasmInfo {
  'hash' : Uint8Array | number[],
  'wasm' : Uint8Array | number[],
  'description' : string,
  'created_at' : bigint,
  'created_by' : Principal,
}
export interface _SERVICE {
  'access_token' : ActorMethod<[Principal], Result>,
  'admin_add_committers' : ActorMethod<[Array<Principal>], Result_1>,
  'admin_add_managers' : ActorMethod<[Array<Principal>], Result_1>,
  'admin_add_wasm' : ActorMethod<
    [AddWasmInput, [] | [Uint8Array | number[]]],
    Result_1
  >,
  'admin_attach_policies' : ActorMethod<[Token], Result_1>,
  'admin_batch_call_buckets' : ActorMethod<
    [Array<Principal>, string, [] | [Uint8Array | number[]]],
    Result_2
  >,
  'admin_create_bucket' : ActorMethod<
    [[] | [CanisterSettings], [] | [Uint8Array | number[]]],
    Result_3
  >,
  'admin_create_bucket_on' : ActorMethod<
    [Principal, [] | [CanisterSettings], [] | [Uint8Array | number[]]],
    Result_3
  >,
  'admin_deploy_bucket' : ActorMethod<
    [DeployWasmInput, [] | [Uint8Array | number[]]],
    Result_1
  >,
  'admin_detach_policies' : ActorMethod<[Token], Result_1>,
  'admin_ed25519_access_token' : ActorMethod<[Token], Result>,
  'admin_remove_committers' : ActorMethod<[Array<Principal>], Result_1>,
  'admin_remove_managers' : ActorMethod<[Array<Principal>], Result_1>,
  'admin_set_managers' : ActorMethod<[Array<Principal>], Result_1>,
  'admin_sign_access_token' : ActorMethod<[Token], Result>,
  'admin_topup_all_buckets' : ActorMethod<[], Result_4>,
  'admin_update_bucket_canister_settings' : ActorMethod<
    [UpdateSettingsArgs],
    Result_1
  >,
  'admin_upgrade_all_buckets' : ActorMethod<
    [[] | [Uint8Array | number[]]],
    Result_1
  >,
  'admin_weak_access_token' : ActorMethod<[Token, bigint, bigint], Result>,
  'bucket_deployment_logs' : ActorMethod<
    [[] | [bigint], [] | [bigint]],
    Result_5
  >,
  'ed25519_access_token' : ActorMethod<[Principal], Result>,
  'get_bucket_wasm' : ActorMethod<[Uint8Array | number[]], Result_6>,
  'get_buckets' : ActorMethod<[], Result_7>,
  'get_canister_status' : ActorMethod<[[] | [Principal]], Result_8>,
  'get_cluster_info' : ActorMethod<[], Result_9>,
  'get_deployed_buckets' : ActorMethod<[], Result_5>,
  'get_subject_policies' : ActorMethod<[Principal], Result_10>,
  'get_subject_policies_for' : ActorMethod<[Principal, Principal], Result_11>,
  'validate2_admin_add_wasm' : ActorMethod<
    [AddWasmInput, [] | [Uint8Array | number[]]],
    Result_11
  >,
  'validate2_admin_batch_call_buckets' : ActorMethod<
    [Array<Principal>, string, [] | [Uint8Array | number[]]],
    Result_11
  >,
  'validate2_admin_deploy_bucket' : ActorMethod<
    [DeployWasmInput, [] | [Uint8Array | number[]]],
    Result_11
  >,
  'validate2_admin_set_managers' : ActorMethod<[Array<Principal>], Result_11>,
  'validate2_admin_upgrade_all_buckets' : ActorMethod<
    [[] | [Uint8Array | number[]]],
    Result_11
  >,
  'validate_admin_add_committers' : ActorMethod<[Array<Principal>], Result_11>,
  'validate_admin_add_managers' : ActorMethod<[Array<Principal>], Result_11>,
  'validate_admin_add_wasm' : ActorMethod<
    [AddWasmInput, [] | [Uint8Array | number[]]],
    Result_1
  >,
  'validate_admin_batch_call_buckets' : ActorMethod<
    [Array<Principal>, string, [] | [Uint8Array | number[]]],
    Result_2
  >,
  'validate_admin_create_bucket' : ActorMethod<
    [[] | [CanisterSettings], [] | [Uint8Array | number[]]],
    Result_11
  >,
  'validate_admin_create_bucket_on' : ActorMethod<
    [Principal, [] | [CanisterSettings], [] | [Uint8Array | number[]]],
    Result_11
  >,
  'validate_admin_deploy_bucket' : ActorMethod<
    [DeployWasmInput, [] | [Uint8Array | number[]]],
    Result_1
  >,
  'validate_admin_remove_committers' : ActorMethod<
    [Array<Principal>],
    Result_11
  >,
  'validate_admin_remove_managers' : ActorMethod<[Array<Principal>], Result_11>,
  'validate_admin_set_managers' : ActorMethod<[Array<Principal>], Result_1>,
  'validate_admin_update_bucket_canister_settings' : ActorMethod<
    [UpdateSettingsArgs],
    Result_11
  >,
  'validate_admin_upgrade_all_buckets' : ActorMethod<
    [[] | [Uint8Array | number[]]],
    Result_1
  >,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
