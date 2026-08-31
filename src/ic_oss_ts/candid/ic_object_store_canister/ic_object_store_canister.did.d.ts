import type { Principal } from '@icp-sdk/core/principal';
import type { ActorMethod } from '@icp-sdk/core/agent';
import type { IDL } from '@icp-sdk/core/candid';

export type Attribute = { 'ContentType' : null } |
  { 'Metadata' : string } |
  { 'ContentEncoding' : null } |
  { 'ContentLanguage' : null } |
  { 'CacheControl' : null } |
  { 'ContentDisposition' : null };
export type Error = {
    /**
     * Error when the object at the location isn't modified
     */
    'NotModified' : {
      /**
       * The path to the file
       */
      'path' : string,
      /**
       * The wrapped error
       */
      'error' : string,
    }
  } |
  {
    /**
     * Error when a configuration key is invalid for the store used
     */
    'UnknownConfigurationKey' : {
      /**
       * The configuration key used
       */
      'key' : string,
    }
  } |
  {
    /**
     * Error when the object is not found at given location
     */
    'NotFound' : {
      /**
       * The path to file
       */
      'path' : string,
    }
  } |
  {
    /**
     * Error when the used credentials don't have enough permission
     * to perform the requested operation
     */
    'PermissionDenied' : {
      /**
       * The path to the file
       */
      'path' : string,
      /**
       * The wrapped error
       */
      'error' : string,
    }
  } |
  {
    /**
     * A fallback error type when no variant matches
     */
    'Generic' : {
      /**
       * The wrapped error
       */
      'error' : string,
    }
  } |
  {
    /**
     * Error when the object already exists
     */
    'AlreadyExists' : {
      /**
       * The path to the
       */
      'path' : string,
    }
  } |
  {
    /**
     * Error for invalid path
     */
    'InvalidPath' : {
      /**
       * The wrapped error
       */
      'path' : string,
    }
  } |
  {
    /**
     * Error when the attempted operation is not supported
     */
    'NotSupported' : {
      /**
       * The wrapped error
       */
      'error' : string,
    }
  } |
  {
    /**
     * Error when the required conditions failed for the operation
     */
    'Precondition' : {
      /**
       * The path to the file
       */
      'path' : string,
      /**
       * The wrapped error
       */
      'error' : string,
    }
  } |
  {
    /**
     * Error when an operation is not implemented
     * Error when an operation is not implemented
     */
    'NotImplemented' : {
      /**
       * Which driver this is that hasn't implemented this operation,
       * to aid debugging in contexts that may be using multiple implementations.
       */
      'implementer' : string,
      /**
       * What isn't implemented. Should include at least the method
       * name that was called; could also include other relevant
       * subcontexts.
       */
      'operation' : string,
    }
  } |
  {
    /**
     * Error when the used credentials lack valid authentication
     */
    'Unauthenticated' : {
      /**
       * The path to the file
       */
      'path' : string,
      /**
       * The wrapped error
       */
      'error' : string,
    }
  };
export interface GetOptions {
  /**
   * Request will succeed if the `ObjectMeta::e_tag` matches
   * otherwise returning [`Error::Precondition`]
   * 
   * See <https://datatracker.ietf.org/doc/html/rfc9110#name-if-match>
   * 
   * Examples:
   * 
   * ```text
   * If-Match: "xyzzy"
   * If-Match: "xyzzy", "r2d2xxxx", "c3piozzzz"
   * If-Match: *
   * ```
   */
  'if_match' : [] | [string],
  /**
   * Request will succeed if the object has not been modified since
   * otherwise returning [`Error::Precondition`]
   * 
   * Some stores, such as S3, will only return `NotModified` for exact
   * timestamp matches, instead of for any timestamp greater than or equal.
   * 
   * <https://datatracker.ietf.org/doc/html/rfc9110#section-13.1.4>
   */
  'if_unmodified_since' : [] | [bigint],
  /**
   * Request transfer of no content
   * 
   * <https://datatracker.ietf.org/doc/html/rfc9110#name-head>
   */
  'head' : boolean,
  /**
   * Request will succeed if the object has been modified since
   * 
   * <https://datatracker.ietf.org/doc/html/rfc9110#section-13.1.3>
   */
  'if_modified_since' : [] | [bigint],
  /**
   * Request a particular object version
   */
  'version' : [] | [string],
  /**
   * Request will succeed if the `ObjectMeta::e_tag` does not match
   * otherwise returning [`Error::NotModified`]
   * 
   * See <https://datatracker.ietf.org/doc/html/rfc9110#section-13.1.2>
   * 
   * Examples:
   * 
   * ```text
   * If-None-Match: "xyzzy"
   * If-None-Match: "xyzzy", "r2d2xxxx", "c3piozzzz"
   * If-None-Match: *
   * ```
   */
  'if_none_match' : [] | [string],
  /**
   * Request transfer of only the specified range of bytes
   * otherwise returning [`Error::NotModified`]
   * 
   * <https://datatracker.ietf.org/doc/html/rfc9110#name-range>
   */
  'range' : [] | [GetRange],
}
export type GetRange = {
    /**
     * Request all bytes starting from a given byte offset
     */
    'Offset' : bigint
  } |
  {
    /**
     * Request a specific range of bytes
     * 
     * If the given range is zero-length or starts after the end of the object,
     * an error will be returned. Additionally, if the range ends after the end
     * of the object, the entire remainder of the object will be returned.
     * Otherwise, the exact requested range will be returned.
     */
    'Bounded' : [bigint, bigint]
  } |
  {
    /**
     * Request up to the last n bytes
     */
    'Suffix' : bigint
  };
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
  /**
   * Prefixes that are common (like directories)
   */
  'common_prefixes' : Array<string>,
  /**
   * Object metadata for the listing
   */
  'objects' : Array<ObjectMeta>,
}
export interface ObjectMeta {
  /**
   * A set of tags with AES256-GCM encryption
   * Each part of the object has its own tag
   */
  'aes_tags' : [] | [Array<Uint8Array | number[]>],
  /**
   * The size in bytes of the object
   */
  'size' : bigint,
  /**
   * The unique identifier for the object
   * 
   * <https://datatracker.ietf.org/doc/html/rfc9110#name-etag>
   */
  'e_tag' : [] | [string],
  /**
   * A version indicator for this object
   */
  'version' : [] | [string],
  /**
   * The last modified time
   */
  'last_modified' : bigint,
  /**
   * A nonce with AES256-GCM encryption
   */
  'aes_nonce' : [] | [Uint8Array | number[]],
  /**
   * The full path to the object
   */
  'location' : string,
}
export interface PartId {
  /**
   * Id of this part
   */
  'content_id' : string,
}
export type PutMode = {
    /**
     * Perform an atomic write operation, overwriting any object present at the provided path
     */
    'Overwrite' : null
  } |
  {
    /**
     * Perform an atomic write operation, returning [`Error::AlreadyExists`] if an
     * object already exists at the provided path
     */
    'Create' : null
  } |
  {
    /**
     * Perform an atomic write operation if the current version of the object matches the
     * provided [`UpdateVersion`], returning [`Error::Precondition`] otherwise
     */
    'Update' : UpdateVersion
  };
export interface PutMultipartOptions {
  /**
   * A set of tags with AES256-GCM encryption
   * Each part of the object has its own tag
   */
  'aes_tags' : [] | [Array<Uint8Array | number[]>],
  /**
   * Provide a [`TagSet`] for this object
   * 
   * Implementations that don't support object tagging should ignore this
   */
  'tags' : string,
  /**
   * Provide a set of [`Attributes`]
   * 
   * Implementations that don't support an attribute should return an error
   */
  'attributes' : Array<[Attribute, string]>,
  /**
   * A nonce with AES256-GCM encryption
   */
  'aes_nonce' : [] | [Uint8Array | number[]],
}
export interface PutOptions {
  /**
   * A set of tags with AES256-GCM encryption
   * Each part of the object has its own tag
   */
  'aes_tags' : [] | [Array<Uint8Array | number[]>],
  /**
   * Configure the [`PutMode`] for this operation
   */
  'mode' : PutMode,
  /**
   * Provide a [`TagSet`] for this object
   * 
   * Implementations that don't support object tagging should ignore this
   */
  'tags' : string,
  /**
   * Provide a set of [`Attributes`]
   * 
   * Implementations that don't support an attribute should return an error
   */
  'attributes' : Array<[Attribute, string]>,
  /**
   * A nonce with AES256-GCM encryption
   */
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
  /**
   * The unique identifier for the newly created object
   * 
   * <https://datatracker.ietf.org/doc/html/rfc9110#name-etag>
   */
  'e_tag' : [] | [string],
  /**
   * A version indicator for the newly created object
   */
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
    [string, string, PutMultipartOptions],
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
