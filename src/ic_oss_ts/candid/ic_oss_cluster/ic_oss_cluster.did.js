export const idlFactory = ({ IDL }) => {
  const UpgradeArgs = IDL.Record({
    'name' : IDL.Opt(IDL.Text),
    'token_expiration' : IDL.Opt(IDL.Nat64),
  });
  const InitArgs = IDL.Record({
    'ecdsa_key_name' : IDL.Text,
    'name' : IDL.Text,
    'token_expiration' : IDL.Nat64,
  });
  const ChainArgs = IDL.Variant({ 'Upgrade' : UpgradeArgs, 'Init' : InitArgs });
  const Result = IDL.Variant({ 'Ok' : IDL.Vec(IDL.Nat8), 'Err' : IDL.Text });
  const Token = IDL.Record({
    'subject' : IDL.Principal,
    'audience' : IDL.Principal,
    'policies' : IDL.Text,
  });
  const Result_1 = IDL.Variant({ 'Ok' : IDL.Null, 'Err' : IDL.Text });
  const ClusterInfo = IDL.Record({
    'ecdsa_token_public_key' : IDL.Text,
    'ecdsa_key_name' : IDL.Text,
    'managers' : IDL.Vec(IDL.Principal),
    'name' : IDL.Text,
    'token_expiration' : IDL.Nat64,
  });
  const Result_2 = IDL.Variant({ 'Ok' : ClusterInfo, 'Err' : IDL.Text });
  return IDL.Service({
    'access_token' : IDL.Func([IDL.Principal], [Result], []),
    'admin_attach_policies' : IDL.Func([Token], [Result_1], []),
    'admin_detach_policies' : IDL.Func([Token], [Result_1], []),
    'admin_set_managers' : IDL.Func([IDL.Vec(IDL.Principal)], [Result_1], []),
    'admin_sign_access_token' : IDL.Func([Token], [Result], []),
    'get_cluster_info' : IDL.Func([], [Result_2], ['query']),
    'validate_admin_set_managers' : IDL.Func(
        [IDL.Vec(IDL.Principal)],
        [Result_1],
        ['query'],
      ),
  });
};
export const init = ({ IDL }) => {
  const UpgradeArgs = IDL.Record({
    'name' : IDL.Opt(IDL.Text),
    'token_expiration' : IDL.Opt(IDL.Nat64),
  });
  const InitArgs = IDL.Record({
    'ecdsa_key_name' : IDL.Text,
    'name' : IDL.Text,
    'token_expiration' : IDL.Nat64,
  });
  const ChainArgs = IDL.Variant({ 'Upgrade' : UpgradeArgs, 'Init' : InitArgs });
  return [IDL.Opt(ChainArgs)];
};
