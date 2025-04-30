import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export type Attribute = { 'ContentType' : null } |
  { 'Metadata' : string } |
  { 'ContentEncoding' : null } |
  { 'ContentLanguage' : null } |
  { 'CacheControl' : null } |
  { 'ContentDisposition' : null };
export type Error = { 'NotModified' : { 'path' : string, 'error' : string } } |
  { 'UnknownConfigurationKey' : { 'key' : string } } |
  { 'NotFound' : { 'path' : string } } |
  { 'PermissionDenied' : { 'path' : string, 'error' : string } } |
  { 'Generic' : { 'error' : string } } |
  { 'AlreadyExists' : { 'path' : string } } |
  { 'InvalidPath' : { 'path' : string } } |
  { 'NotSupported' : { 'error' : string } } |
  { 'Precondition' : { 'path' : string, 'error' : string } } |
  { 'NotImplemented' : null } |
  { 'Unauthenticated' : { 'path' : string, 'error' : string } };
export interface GetOptions {
  'if_match' : [] | [string],
  'if_unmodified_since' : [] | [bigint],
  'head' : boolean,
  'if_modified_since' : [] | [bigint],
  'version' : [] | [string],
  'if_none_match' : [] | [string],
  'range' : [] | [GetRange],
}
export type GetRange = { 'Offset' : bigint } |
  { 'Bounded' : [bigint, bigint] } |
  { 'Suffix' : bigint };
export interface GetResult {
  'meta' : ObjectMeta,
  'attributes' : Array<[Attribute, string]>,
  'range' : [bigint, bigint],
  'payload' : Uint8Array | number[],
}
export interface InitArgs {
  'governance_canister' : [] | [Principal],
  'name' : string,
}
export type InstallArgs = { 'Upgrade' : UpgradeArgs } |
  { 'Init' : InitArgs };
export interface ListResult {
  'common_prefixes' : Array<string>,
  'objects' : Array<ObjectMeta>,
}
export interface ObjectMeta {
  'aes_tags' : [] | [Array<Uint8Array | number[]>],
  'size' : bigint,
  'e_tag' : [] | [string],
  'version' : [] | [string],
  'last_modified' : bigint,
  'aes_nonce' : [] | [Uint8Array | number[]],
  'location' : string,
}
export interface PartId { 'content_id' : string }
export type PutMode = { 'Overwrite' : null } |
  { 'Create' : null } |
  { 'Update' : UpdateVersion };
export interface PutMultipartOpts {
  'aes_tags' : [] | [Array<Uint8Array | number[]>],
  'tags' : string,
  'attributes' : Array<[Attribute, string]>,
  'aes_nonce' : [] | [Uint8Array | number[]],
}
export interface PutOptions {
  'aes_tags' : [] | [Array<Uint8Array | number[]>],
  'mode' : PutMode,
  'tags' : string,
  'attributes' : Array<[Attribute, string]>,
  'aes_nonce' : [] | [Uint8Array | number[]],
}
export type Result = { 'Ok' : null } |
  { 'Err' : Error };
export type Result_1 = { 'Ok' : null } |
  { 'Err' : string };
export type Result_10 = { 'Ok' : Array<ObjectMeta> } |
  { 'Err' : Error };
export type Result_11 = { 'Ok' : ListResult } |
  { 'Err' : Error };
export type Result_12 = { 'Ok' : PartId } |
  { 'Err' : Error };
export type Result_13 = { 'Ok' : string } |
  { 'Err' : string };
export type Result_2 = { 'Ok' : UpdateVersion } |
  { 'Err' : Error };
export type Result_3 = { 'Ok' : string } |
  { 'Err' : Error };
export type Result_4 = { 'Ok' : GetResult } |
  { 'Err' : Error };
export type Result_5 = { 'Ok' : Uint8Array | number[] } |
  { 'Err' : Error };
export type Result_6 = { 'Ok' : Array<Uint8Array | number[]> } |
  { 'Err' : Error };
export type Result_7 = { 'Ok' : StateInfo } |
  { 'Err' : string };
export type Result_8 = { 'Ok' : ObjectMeta } |
  { 'Err' : Error };
export type Result_9 = { 'Ok' : boolean } |
  { 'Err' : string };
export interface StateInfo {
  'next_etag' : bigint,
  'managers' : Array<Principal>,
  'governance_canister' : [] | [Principal],
  'name' : string,
  'auditors' : Array<Principal>,
  'objects' : bigint,
}
export interface UpdateVersion {
  'e_tag' : [] | [string],
  'version' : [] | [string],
}
export interface UpgradeArgs {
  'governance_canister' : [] | [Principal],
  'name' : [] | [string],
}
export interface _SERVICE {
  'abort_multipart' : ActorMethod<[string, string], Result>,
  'admin_add_auditors' : ActorMethod<[Array<Principal>], Result_1>,
  'admin_add_managers' : ActorMethod<[Array<Principal>], Result_1>,
  'admin_clear' : ActorMethod<[], Result_1>,
  'admin_remove_auditors' : ActorMethod<[Array<Principal>], Result_1>,
  'admin_remove_managers' : ActorMethod<[Array<Principal>], Result_1>,
  'complete_multipart' : ActorMethod<
    [string, string, PutMultipartOpts],
    Result_2
  >,
  'copy' : ActorMethod<[string, string], Result>,
  'copy_if_not_exists' : ActorMethod<[string, string], Result>,
  'create_multipart' : ActorMethod<[string], Result_3>,
  'delete' : ActorMethod<[string], Result>,
  'get_opts' : ActorMethod<[string, GetOptions], Result_4>,
  'get_part' : ActorMethod<[string, bigint], Result_5>,
  'get_ranges' : ActorMethod<[string, Array<[bigint, bigint]>], Result_6>,
  'get_state' : ActorMethod<[], Result_7>,
  'head' : ActorMethod<[string], Result_8>,
  'is_member' : ActorMethod<[string, Principal], Result_9>,
  'list' : ActorMethod<[[] | [string]], Result_10>,
  'list_with_delimiter' : ActorMethod<[[] | [string]], Result_11>,
  'list_with_offset' : ActorMethod<[[] | [string], string], Result_10>,
  'put_opts' : ActorMethod<
    [string, Uint8Array | number[], PutOptions],
    Result_2
  >,
  'put_part' : ActorMethod<
    [string, string, bigint, Uint8Array | number[]],
    Result_12
  >,
  'rename' : ActorMethod<[string, string], Result>,
  'rename_if_not_exists' : ActorMethod<[string, string], Result>,
  'validate_admin_add_auditors' : ActorMethod<[Array<Principal>], Result_13>,
  'validate_admin_add_managers' : ActorMethod<[Array<Principal>], Result_13>,
  'validate_admin_clear' : ActorMethod<[], Result_13>,
  'validate_admin_remove_auditors' : ActorMethod<[Array<Principal>], Result_13>,
  'validate_admin_remove_managers' : ActorMethod<[Array<Principal>], Result_13>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
