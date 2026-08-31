import type { Principal } from '@icp-sdk/core/principal';
import type { ActorMethod } from '@icp-sdk/core/agent';
import type { IDL } from '@icp-sdk/core/candid';

export interface BucketInfo {
  'status' : number,
  'total_chunks' : bigint,
  'trusted_eddsa_pub_keys' : Array<Uint8Array | number[]>,
  'managers' : Array<Principal>,
  'governance_canister' : [] | [Principal],
  'name' : string,
  'max_custom_data_size' : number,
  'auditors' : Array<Principal>,
  'total_files' : bigint,
  'max_children' : number,
  'enable_hash_index' : boolean,
  'max_file_size' : bigint,
  'folder_id' : number,
  'visibility' : number,
  'max_folder_depth' : number,
  'trusted_ecdsa_pub_keys' : Array<Uint8Array | number[]>,
  'total_folders' : bigint,
  'file_id' : number,
}
export type CanisterArgs = { 'Upgrade' : UpgradeArgs } |
  { 'Init' : InitArgs };
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
export interface CreateFileInput {
  'dek' : [] | [Uint8Array | number[]],
  'status' : [] | [number],
  'content' : [] | [Uint8Array | number[]],
  'custom' : [] | [Array<[string, MetadataValue]>],
  'hash' : [] | [Uint8Array | number[]],
  'name' : string,
  'size' : [] | [bigint],
  'content_type' : string,
  'parent' : number,
}
export interface CreateFileOutput { 'id' : number, 'created_at' : bigint }
export interface CreateFolderInput { 'name' : string, 'parent' : number }
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
export interface FileInfo {
  'ex' : [] | [Array<[string, MetadataValue]>],
  'id' : number,
  'dek' : [] | [Uint8Array | number[]],
  'status' : number,
  'updated_at' : bigint,
  'custom' : [] | [Array<[string, MetadataValue]>],
  'hash' : [] | [Uint8Array | number[]],
  'name' : string,
  'size' : bigint,
  'content_type' : string,
  'created_at' : bigint,
  'filled' : bigint,
  'chunks' : number,
  'parent' : number,
}
export interface FolderInfo {
  'id' : number,
  'files' : Uint32Array | number[],
  'status' : number,
  'updated_at' : bigint,
  'name' : string,
  'folders' : Uint32Array | number[],
  'created_at' : bigint,
  'parent' : number,
}
export interface FolderName { 'id' : number, 'name' : string }
export interface InitArgs {
  'governance_canister' : [] | [Principal],
  'name' : string,
  'max_custom_data_size' : number,
  'max_children' : number,
  'enable_hash_index' : boolean,
  'max_file_size' : bigint,
  'visibility' : number,
  'max_folder_depth' : number,
  'file_id' : number,
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
 * Variant type for the `icrc1_metadata` endpoint values. The corresponding metadata keys are
 * arbitrary Unicode strings and must follow the pattern `<namespace>:<key>`, where `<namespace>`
 * is a string not containing colons. The namespace `icrc1` is reserved for keys defined in the
 * ICRC-1 standard. For more information, see the
 * [documentation of Metadata in the ICRC-1 standard](https://github.com/dfinity/ICRC-1/tree/main/standards/ICRC-1#metadata).
 * Note that the `MetadataValue` type is a subset of the [`icrc_ledger_types::icrc::generic_value::ICRC3Value`] type.
 */
export type MetadataValue = { 'Int' : bigint } |
  { 'Nat' : bigint } |
  { 'Blob' : Uint8Array | number[] } |
  { 'Text' : string };
export interface MoveInput { 'id' : number, 'to' : number, 'from' : number }
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
export type Result = { 'Ok' : null } |
  { 'Err' : string };
export type Result_1 = { 'Ok' : Uint32Array | number[] } |
  { 'Err' : string };
export type Result_10 = { 'Ok' : Array<FileInfo> } |
  { 'Err' : string };
export type Result_11 = { 'Ok' : Array<FolderInfo> } |
  { 'Err' : string };
export type Result_12 = { 'Ok' : UpdateFileOutput } |
  { 'Err' : string };
export type Result_13 = { 'Ok' : UpdateFileChunkOutput } |
  { 'Err' : string };
export type Result_14 = { 'Ok' : string } |
  { 'Err' : string };
export type Result_2 = { 'Ok' : CreateFileOutput } |
  { 'Err' : string };
export type Result_3 = { 'Ok' : boolean } |
  { 'Err' : string };
export type Result_4 = { 'Ok' : BucketInfo } |
  { 'Err' : string };
export type Result_5 = { 'Ok' : CanisterStatusResult } |
  { 'Err' : string };
export type Result_6 = { 'Ok' : Array<FolderName> } |
  { 'Err' : string };
export type Result_7 = { 'Ok' : Array<[number, Uint8Array | number[]]> } |
  { 'Err' : string };
export type Result_8 = { 'Ok' : FileInfo } |
  { 'Err' : string };
export type Result_9 = { 'Ok' : FolderInfo } |
  { 'Err' : string };
export interface UpdateBucketInput {
  'status' : [] | [number],
  'trusted_eddsa_pub_keys' : [] | [Array<Uint8Array | number[]>],
  'name' : [] | [string],
  'max_custom_data_size' : [] | [number],
  'max_children' : [] | [number],
  'enable_hash_index' : [] | [boolean],
  'max_file_size' : [] | [bigint],
  'visibility' : [] | [number],
  'max_folder_depth' : [] | [number],
  'trusted_ecdsa_pub_keys' : [] | [Array<Uint8Array | number[]>],
}
export interface UpdateFileChunkInput {
  'id' : number,
  'chunk_index' : number,
  'content' : Uint8Array | number[],
}
export interface UpdateFileChunkOutput {
  'updated_at' : bigint,
  'filled' : bigint,
}
export interface UpdateFileInput {
  'id' : number,
  'status' : [] | [number],
  'custom' : [] | [Array<[string, MetadataValue]>],
  'hash' : [] | [Uint8Array | number[]],
  'name' : [] | [string],
  'size' : [] | [bigint],
  'content_type' : [] | [string],
}
export interface UpdateFileOutput { 'updated_at' : bigint }
export interface UpdateFolderInput {
  'id' : number,
  'status' : [] | [number],
  'name' : [] | [string],
}
export interface UpgradeArgs {
  'governance_canister' : [] | [Principal],
  'max_custom_data_size' : [] | [number],
  'max_children' : [] | [number],
  'enable_hash_index' : [] | [boolean],
  'max_file_size' : [] | [bigint],
  'max_folder_depth' : [] | [number],
}
export interface _SERVICE {
  'admin_add_auditors' : ActorMethod<[Array<Principal>], Result>,
  'admin_add_managers' : ActorMethod<[Array<Principal>], Result>,
  'admin_remove_auditors' : ActorMethod<[Array<Principal>], Result>,
  'admin_remove_managers' : ActorMethod<[Array<Principal>], Result>,
  'admin_set_auditors' : ActorMethod<[Array<Principal>], Result>,
  'admin_set_managers' : ActorMethod<[Array<Principal>], Result>,
  'admin_update_bucket' : ActorMethod<[UpdateBucketInput], Result>,
  'api_version' : ActorMethod<[], number>,
  'batch_delete_subfiles' : ActorMethod<
    [number, Uint32Array | number[], [] | [Uint8Array | number[]]],
    Result_1
  >,
  'create_file' : ActorMethod<
    [CreateFileInput, [] | [Uint8Array | number[]]],
    Result_2
  >,
  'create_folder' : ActorMethod<
    [CreateFolderInput, [] | [Uint8Array | number[]]],
    Result_2
  >,
  'delete_file' : ActorMethod<[number, [] | [Uint8Array | number[]]], Result_3>,
  'delete_folder' : ActorMethod<
    [number, [] | [Uint8Array | number[]]],
    Result_3
  >,
  'get_bucket_info' : ActorMethod<[[] | [Uint8Array | number[]]], Result_4>,
  'get_canister_status' : ActorMethod<[], Result_5>,
  'get_file_ancestors' : ActorMethod<
    [number, [] | [Uint8Array | number[]]],
    Result_6
  >,
  'get_file_chunks' : ActorMethod<
    [number, number, [] | [number], [] | [Uint8Array | number[]]],
    Result_7
  >,
  'get_file_info' : ActorMethod<
    [number, [] | [Uint8Array | number[]]],
    Result_8
  >,
  'get_file_info_by_hash' : ActorMethod<
    [Uint8Array | number[], [] | [Uint8Array | number[]]],
    Result_8
  >,
  'get_folder_ancestors' : ActorMethod<
    [number, [] | [Uint8Array | number[]]],
    Result_6
  >,
  'get_folder_info' : ActorMethod<
    [number, [] | [Uint8Array | number[]]],
    Result_9
  >,
  'list_files' : ActorMethod<
    [number, [] | [number], [] | [number], [] | [Uint8Array | number[]]],
    Result_10
  >,
  'list_folders' : ActorMethod<
    [number, [] | [number], [] | [number], [] | [Uint8Array | number[]]],
    Result_11
  >,
  'move_file' : ActorMethod<
    [MoveInput, [] | [Uint8Array | number[]]],
    Result_12
  >,
  'move_folder' : ActorMethod<
    [MoveInput, [] | [Uint8Array | number[]]],
    Result_12
  >,
  'update_file_chunk' : ActorMethod<
    [UpdateFileChunkInput, [] | [Uint8Array | number[]]],
    Result_13
  >,
  'update_file_info' : ActorMethod<
    [UpdateFileInput, [] | [Uint8Array | number[]]],
    Result_12
  >,
  'update_folder_info' : ActorMethod<
    [UpdateFolderInput, [] | [Uint8Array | number[]]],
    Result_12
  >,
  'validate2_admin_set_auditors' : ActorMethod<[Array<Principal>], Result_14>,
  'validate2_admin_set_managers' : ActorMethod<[Array<Principal>], Result_14>,
  'validate2_admin_update_bucket' : ActorMethod<[UpdateBucketInput], Result_14>,
  'validate_admin_add_auditors' : ActorMethod<[Array<Principal>], Result_14>,
  'validate_admin_add_managers' : ActorMethod<[Array<Principal>], Result_14>,
  'validate_admin_remove_auditors' : ActorMethod<[Array<Principal>], Result_14>,
  'validate_admin_remove_managers' : ActorMethod<[Array<Principal>], Result_14>,
  'validate_admin_set_auditors' : ActorMethod<[Array<Principal>], Result>,
  'validate_admin_set_managers' : ActorMethod<[Array<Principal>], Result>,
  'validate_admin_update_bucket' : ActorMethod<[UpdateBucketInput], Result>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
