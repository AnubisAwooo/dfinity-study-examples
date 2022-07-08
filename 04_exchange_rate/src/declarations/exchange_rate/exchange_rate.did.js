export const idlFactory = ({ IDL }) => {
  const TimeRange = IDL.Record({ 'end' : IDL.Nat64, 'start' : IDL.Nat64 });
  const RatesWithInterval = IDL.Record({
    'interval' : IDL.Nat64,
    'rates' : IDL.Vec(IDL.Tuple(IDL.Nat64, IDL.Float32)),
  });
  return IDL.Service({
    'get_rates' : IDL.Func([TimeRange], [RatesWithInterval], []),
    'get_rates2' : IDL.Func([], [IDL.Text], []),
  });
};
export const init = ({ IDL }) => { return []; };
