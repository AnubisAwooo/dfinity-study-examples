use candid::{candid_method, CandidType, Principal};
use std::cell::RefCell;
use std::hash::Hash;

use ic_cdk_macros::*;
use ic_ledger_types::{
    AccountIdentifier, BlockIndex, Memo, Subaccount, Tokens, DEFAULT_SUBACCOUNT,
    MAINNET_LEDGER_CANISTER_ID,
};
use serde::{Deserialize, Serialize};

// 配置信息结构体
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, Hash, PartialEq)]
pub struct Conf {
    ledger_canister_id: Principal,
    // The subaccount of the account identifier that will be used to withdraw tokens and send them
    // to another account identifier. If set to None then the default subaccount will be used.
    // See the [Ledger doc](https://smartcontracts.org/docs/integration/ledger-quick-start.html#_accounts).
    subaccount: Option<Subaccount>,
    transaction_fee: Tokens,
}

// 默认的配置信息
impl Default for Conf {
    fn default() -> Self {
        Conf {
            ledger_canister_id: MAINNET_LEDGER_CANISTER_ID, // 默认为 主网 ledger canister id
            subaccount: None,
            transaction_fee: Tokens::from_e8s(10_000), // 默认手续费
        }
    }
}

thread_local! {
    static CONF: RefCell<Conf> = RefCell::new(Conf::default()); // 默认配置信息
}

#[init]
#[candid_method(init)]
fn init(conf: Conf) {
    CONF.with(|c| c.replace(conf)); // 初始化，替换配置信息
}

// 转账参数
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, Hash)]
pub struct TransferArgs {
    amount: Tokens,
    to_principal: Principal,
    to_subaccount: Option<Subaccount>,
}

#[update]
#[candid_method(update)]
async fn transfer(args: TransferArgs) -> Result<BlockIndex, String> {
    ic_cdk::println!(
        "Transferring {} tokens to principal {} subaccount {:?}",
        &args.amount,
        &args.to_principal,
        &args.to_subaccount
    );
    let ledger_canister_id = CONF.with(|conf| conf.borrow().ledger_canister_id); // ledger 的 canister id
    let to_subaccount = args.to_subaccount.unwrap_or(DEFAULT_SUBACCOUNT); // 若未指定子账户，则选取默认子账户 [0;32]
    let transfer_args = CONF.with(|conf| {
        let conf = conf.borrow();
        ic_ledger_types::TransferArgs {
            memo: Memo(0),
            amount: args.amount,
            fee: conf.transaction_fee,
            from_subaccount: conf.subaccount,
            to: AccountIdentifier::new(&args.to_principal, &to_subaccount),
            created_at_time: None,
        }
    });
    ic_ledger_types::transfer(ledger_canister_id, transfer_args)
        .await
        .map_err(|e| format!("failed to call ledger: {:?}", e))?
        .map_err(|e| format!("ledger transfer error {:?}", e))
}
