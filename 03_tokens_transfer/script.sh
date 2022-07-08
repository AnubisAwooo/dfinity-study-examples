dfx identity new minter
dfx identity use minter
export MINT_ACC=$(dfx ledger account-id)

dfx identity use default
export LEDGER_ACC=$(dfx ledger account-id)

# dfx deploy ledger --argument '(record {minting_account = "'${MINT_ACC}'"; initial_values = vec { record { "'${LEDGER_ACC}'"; record { e8s=100_000_000_000 } }; }; send_whitelist = vec {}})'

dfx identity use default
export ARCHIVE_CONTROLLER=$(dfx identity get-principal)
dfx deploy ledger --argument '(record {minting_account = "'${MINT_ACC}'"; initial_values = vec { record { "'${LEDGER_ACC}'"; record { e8s=100_000_000_000 } }; }; send_whitelist = vec {}; archive_options = opt record { trigger_threshold = 2000; num_blocks_to_archive = 1000; controller_id = principal "'${ARCHIVE_CONTROLLER}'" }})'

dfx canister call ledger account_balance '(record { account = '$(python3 -c 'print("vec{" + ";".join([str(b) for b in bytes.fromhex("'$LEDGER_ACC'")]) + "}")')' })'

# 上面部署好本地的 lodger

# 更新本地 ledger 的配置，主要设置挖矿账户和初始化账户
read -r -d '' ARGS <<EOM
(record {
     minting_account="${MINTING_ACCOUNT_ID_HEX}";
     initial_values=vec { record { "${YOUR_ACCOUNT_ID_HEX}"; record { e8s=10_000_000_000 } }; };
     send_whitelist=vec {};
 }, )
EOM
dfx deploy --argument "${ARGS}" ledger

# 利用 ledger 的 canister id，部署转账模块
LEDGER_ID="$(dfx canister id ledger)"
read -r -d '' ARGS <<EOM
(record {
  ledger_canister_id=principal "${LEDGER_ID}";
  transaction_fee=record { e8s=10_000 };
  subaccount=null
}, )
EOM
dfx deploy --argument "${ARGS}" tokens_transfer

# 获取 转账 canister 的 account id
export TOKENS_TRANSFER_ACCOUNT_ID="$(dfx ledger account-id --of-canister tokens_transfer)"
TOKENS_TRANSFER_ACCOUNT_ID_BYTES="$(python3 -c 'print("vec{" + ";".join([str(b) for b in bytes.fromhex("'$TOKENS_TRANSFER_ACCOUNT_ID'")]) + "}")')" 
# 先查看余额
dfx canister call ledger account_balance '(record { account = '$(python3 -c 'print("vec{" + ";".join([str(b) for b in bytes.fromhex("'$TOKENS_TRANSFER_ACCOUNT_ID'")]) + "}")')' })'
# python3 -c 'print("vec{" + ";".join([str(b) for b in bytes.fromhex("'$TOKENS_TRANSFER_ACCOUNT_ID'")]) + "}")'
# 调用 ledger 的转账方法，给 canister 的账户转账
read -r -d '' ARGS <<EOM
(record {
  to=${TOKENS_TRANSFER_ACCOUNT_ID_BYTES};
  amount=record { e8s=100_000 };
  fee=record { e8s=10_000 };
  memo=0:nat64;
}, )
EOM
dfx canister call ledger transfer "${ARGS}"
# 在查看余额
dfx canister call ledger account_balance '(record { account = '$(python3 -c 'print("vec{" + ";".join([str(b) for b in bytes.fromhex("'$TOKENS_TRANSFER_ACCOUNT_ID'")]) + "}")')' })'

export YOUR_PRINCIPAL="$(dfx identity get-principal)"
read -r -d '' ARGS <<EOM
(record {
  amount=record { e8s=5 };
  to_principal=principal "${YOUR_PRINCIPAL}"
},)
EOM
dfx canister call tokens_transfer transfer "${ARGS}"