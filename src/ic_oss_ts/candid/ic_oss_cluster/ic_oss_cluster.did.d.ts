import type { Principal } from '@dfinity/principal';
import type { ActorMethod } from '@dfinity/agent';
import type { IDL } from '@dfinity/candid';

export type ChainArgs = { 'Upgrade' : UpgradeArgs } |
  { 'Init' : InitArgs };
export interface ClusterInfo {
  'ecdsa_token_public_key' : string,
  'ecdsa_key_name' : string,
  'managers' : Array<Principal>,
  'name' : string,
  'token_expiration' : bigint,
}
export interface InitArgs {
  'ecdsa_key_name' : string,
  'name' : string,
  'token_expiration' : bigint,
}
export type Result = { 'Ok' : Uint8Array | number[] } |
  { 'Err' : string };
export type Result_1 = { 'Ok' : null } |
  { 'Err' : string };
export type Result_2 = { 'Ok' : ClusterInfo } |
  { 'Err' : string };
export interface Token {
  'subject' : Principal,
  'audience' : Principal,
  'policies' : string,
}
export interface UpgradeArgs {
  'name' : [] | [string],
  'token_expiration' : [] | [bigint],
}
export interface _SERVICE {
  'access_token' : ActorMethod<[Principal], Result>,
  'admin_attach_policies' : ActorMethod<[Token], Result_1>,
  'admin_detach_policies' : ActorMethod<[Token], Result_1>,
  'admin_set_managers' : ActorMethod<[Array<Principal>], Result_1>,
  'admin_sign_access_token' : ActorMethod<[Token], Result>,
  'get_cluster_info' : ActorMethod<[], Result_2>,
  'validate_admin_set_managers' : ActorMethod<[Array<Principal>], Result_1>,
}
export declare const idlFactory: IDL.InterfaceFactory;
export declare const init: (args: { IDL: typeof IDL }) => IDL.Type[];
