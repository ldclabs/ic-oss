export const idlFactory = ({ IDL }) => {
  const UpgradeArgs = IDL.Record({
    'governance_canister' : IDL.Opt(IDL.Principal),
    'name' : IDL.Opt(IDL.Text),
  });
  const InitArgs = IDL.Record({
    'governance_canister' : IDL.Opt(IDL.Principal),
    'name' : IDL.Text,
  });
  const InstallArgs = IDL.Variant({
    'Upgrade' : UpgradeArgs,
    'Init' : InitArgs,
  });
  const Error = IDL.Variant({
    'NotModified' : IDL.Record({ 'path' : IDL.Text, 'error' : IDL.Text }),
    'UnknownConfigurationKey' : IDL.Record({ 'key' : IDL.Text }),
    'NotFound' : IDL.Record({ 'path' : IDL.Text }),
    'PermissionDenied' : IDL.Record({ 'path' : IDL.Text, 'error' : IDL.Text }),
    'Generic' : IDL.Record({ 'error' : IDL.Text }),
    'AlreadyExists' : IDL.Record({ 'path' : IDL.Text }),
    'InvalidPath' : IDL.Record({ 'path' : IDL.Text }),
    'NotSupported' : IDL.Record({ 'error' : IDL.Text }),
    'Precondition' : IDL.Record({ 'path' : IDL.Text, 'error' : IDL.Text }),
    'NotImplemented' : IDL.Null,
    'Unauthenticated' : IDL.Record({ 'path' : IDL.Text, 'error' : IDL.Text }),
  });
  const Result = IDL.Variant({ 'Ok' : IDL.Null, 'Err' : Error });
  const Result_1 = IDL.Variant({ 'Ok' : IDL.Null, 'Err' : IDL.Text });
  const Attribute = IDL.Variant({
    'ContentType' : IDL.Null,
    'Metadata' : IDL.Text,
    'ContentEncoding' : IDL.Null,
    'ContentLanguage' : IDL.Null,
    'CacheControl' : IDL.Null,
    'ContentDisposition' : IDL.Null,
  });
  const PutMultipartOpts = IDL.Record({
    'aes_tags' : IDL.Opt(IDL.Vec(IDL.Vec(IDL.Nat8))),
    'tags' : IDL.Text,
    'attributes' : IDL.Vec(IDL.Tuple(Attribute, IDL.Text)),
    'aes_nonce' : IDL.Opt(IDL.Vec(IDL.Nat8)),
  });
  const UpdateVersion = IDL.Record({
    'e_tag' : IDL.Opt(IDL.Text),
    'version' : IDL.Opt(IDL.Text),
  });
  const Result_2 = IDL.Variant({ 'Ok' : UpdateVersion, 'Err' : Error });
  const Result_3 = IDL.Variant({ 'Ok' : IDL.Text, 'Err' : Error });
  const GetRange = IDL.Variant({
    'Offset' : IDL.Nat64,
    'Bounded' : IDL.Tuple(IDL.Nat64, IDL.Nat64),
    'Suffix' : IDL.Nat64,
  });
  const GetOptions = IDL.Record({
    'if_match' : IDL.Opt(IDL.Text),
    'if_unmodified_since' : IDL.Opt(IDL.Nat64),
    'head' : IDL.Bool,
    'if_modified_since' : IDL.Opt(IDL.Nat64),
    'version' : IDL.Opt(IDL.Text),
    'if_none_match' : IDL.Opt(IDL.Text),
    'range' : IDL.Opt(GetRange),
  });
  const ObjectMeta = IDL.Record({
    'aes_tags' : IDL.Opt(IDL.Vec(IDL.Vec(IDL.Nat8))),
    'size' : IDL.Nat64,
    'e_tag' : IDL.Opt(IDL.Text),
    'version' : IDL.Opt(IDL.Text),
    'last_modified' : IDL.Nat64,
    'aes_nonce' : IDL.Opt(IDL.Vec(IDL.Nat8)),
    'location' : IDL.Text,
  });
  const GetResult = IDL.Record({
    'meta' : ObjectMeta,
    'attributes' : IDL.Vec(IDL.Tuple(Attribute, IDL.Text)),
    'range' : IDL.Tuple(IDL.Nat64, IDL.Nat64),
    'payload' : IDL.Vec(IDL.Nat8),
  });
  const Result_4 = IDL.Variant({ 'Ok' : GetResult, 'Err' : Error });
  const Result_5 = IDL.Variant({ 'Ok' : IDL.Vec(IDL.Nat8), 'Err' : Error });
  const Result_6 = IDL.Variant({
    'Ok' : IDL.Vec(IDL.Vec(IDL.Nat8)),
    'Err' : Error,
  });
  const StateInfo = IDL.Record({
    'next_etag' : IDL.Nat64,
    'managers' : IDL.Vec(IDL.Principal),
    'governance_canister' : IDL.Opt(IDL.Principal),
    'name' : IDL.Text,
    'auditors' : IDL.Vec(IDL.Principal),
    'objects' : IDL.Nat64,
  });
  const Result_7 = IDL.Variant({ 'Ok' : StateInfo, 'Err' : IDL.Text });
  const Result_8 = IDL.Variant({ 'Ok' : ObjectMeta, 'Err' : Error });
  const Result_9 = IDL.Variant({ 'Ok' : IDL.Bool, 'Err' : IDL.Text });
  const Result_10 = IDL.Variant({ 'Ok' : IDL.Vec(ObjectMeta), 'Err' : Error });
  const ListResult = IDL.Record({
    'common_prefixes' : IDL.Vec(IDL.Text),
    'objects' : IDL.Vec(ObjectMeta),
  });
  const Result_11 = IDL.Variant({ 'Ok' : ListResult, 'Err' : Error });
  const PutMode = IDL.Variant({
    'Overwrite' : IDL.Null,
    'Create' : IDL.Null,
    'Update' : UpdateVersion,
  });
  const PutOptions = IDL.Record({
    'aes_tags' : IDL.Opt(IDL.Vec(IDL.Vec(IDL.Nat8))),
    'mode' : PutMode,
    'tags' : IDL.Text,
    'attributes' : IDL.Vec(IDL.Tuple(Attribute, IDL.Text)),
    'aes_nonce' : IDL.Opt(IDL.Vec(IDL.Nat8)),
  });
  const PartId = IDL.Record({ 'content_id' : IDL.Text });
  const Result_12 = IDL.Variant({ 'Ok' : PartId, 'Err' : Error });
  const Result_13 = IDL.Variant({ 'Ok' : IDL.Text, 'Err' : IDL.Text });
  return IDL.Service({
    'abort_multipart' : IDL.Func([IDL.Text, IDL.Text], [Result], []),
    'admin_add_auditors' : IDL.Func([IDL.Vec(IDL.Principal)], [Result_1], []),
    'admin_add_managers' : IDL.Func([IDL.Vec(IDL.Principal)], [Result_1], []),
    'admin_clear' : IDL.Func([], [Result_1], []),
    'admin_remove_auditors' : IDL.Func(
        [IDL.Vec(IDL.Principal)],
        [Result_1],
        [],
      ),
    'admin_remove_managers' : IDL.Func(
        [IDL.Vec(IDL.Principal)],
        [Result_1],
        [],
      ),
    'complete_multipart' : IDL.Func(
        [IDL.Text, IDL.Text, PutMultipartOpts],
        [Result_2],
        [],
      ),
    'copy' : IDL.Func([IDL.Text, IDL.Text], [Result], []),
    'copy_if_not_exists' : IDL.Func([IDL.Text, IDL.Text], [Result], []),
    'create_multipart' : IDL.Func([IDL.Text], [Result_3], []),
    'delete' : IDL.Func([IDL.Text], [Result], []),
    'get_opts' : IDL.Func([IDL.Text, GetOptions], [Result_4], ['query']),
    'get_part' : IDL.Func([IDL.Text, IDL.Nat64], [Result_5], ['query']),
    'get_ranges' : IDL.Func(
        [IDL.Text, IDL.Vec(IDL.Tuple(IDL.Nat64, IDL.Nat64))],
        [Result_6],
        ['query'],
      ),
    'get_state' : IDL.Func([], [Result_7], ['query']),
    'head' : IDL.Func([IDL.Text], [Result_8], ['query']),
    'is_member' : IDL.Func([IDL.Text, IDL.Principal], [Result_9], ['query']),
    'list' : IDL.Func([IDL.Opt(IDL.Text)], [Result_10], ['query']),
    'list_with_delimiter' : IDL.Func(
        [IDL.Opt(IDL.Text)],
        [Result_11],
        ['query'],
      ),
    'list_with_offset' : IDL.Func(
        [IDL.Opt(IDL.Text), IDL.Text],
        [Result_10],
        ['query'],
      ),
    'put_opts' : IDL.Func(
        [IDL.Text, IDL.Vec(IDL.Nat8), PutOptions],
        [Result_2],
        [],
      ),
    'put_part' : IDL.Func(
        [IDL.Text, IDL.Text, IDL.Nat64, IDL.Vec(IDL.Nat8)],
        [Result_12],
        [],
      ),
    'rename' : IDL.Func([IDL.Text, IDL.Text], [Result], []),
    'rename_if_not_exists' : IDL.Func([IDL.Text, IDL.Text], [Result], []),
    'validate_admin_add_auditors' : IDL.Func(
        [IDL.Vec(IDL.Principal)],
        [Result_13],
        [],
      ),
    'validate_admin_add_managers' : IDL.Func(
        [IDL.Vec(IDL.Principal)],
        [Result_13],
        [],
      ),
    'validate_admin_clear' : IDL.Func([], [Result_13], []),
    'validate_admin_remove_auditors' : IDL.Func(
        [IDL.Vec(IDL.Principal)],
        [Result_13],
        [],
      ),
    'validate_admin_remove_managers' : IDL.Func(
        [IDL.Vec(IDL.Principal)],
        [Result_13],
        [],
      ),
  });
};
export const init = ({ IDL }) => {
  const UpgradeArgs = IDL.Record({
    'governance_canister' : IDL.Opt(IDL.Principal),
    'name' : IDL.Opt(IDL.Text),
  });
  const InitArgs = IDL.Record({
    'governance_canister' : IDL.Opt(IDL.Principal),
    'name' : IDL.Text,
  });
  const InstallArgs = IDL.Variant({
    'Upgrade' : UpgradeArgs,
    'Init' : InitArgs,
  });
  return [IDL.Opt(InstallArgs)];
};
