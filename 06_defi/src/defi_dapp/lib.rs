use std::cell::RefCell;
use std::convert::TryInto;

use candid::{candid_method, export_service, Nat, Principal};
use ic_cdk::caller;
use ic_cdk_macros::*;
use ic_ledger_types::{
    AccountIdentifier, Memo, Tokens, DEFAULT_SUBACCOUNT, MAINNET_LEDGER_CANISTER_ID,
};

mod dip20;
mod exchange;
mod stable;
mod types;
mod utils;

use dip20::DIP20;
use exchange::Exchange;
use types::*;
use utils::principal_to_subaccount;

// ICP 费用
const ICP_FEE: u64 = 10_000;

// 系统状态
#[derive(Default)]
pub struct State {
    owner: Option<Principal>,
    ledger: Option<Principal>,
    exchange: Exchange,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

// 存钱
#[update]
#[candid_method(update)]
pub async fn deposit(token_canister_id: Principal) -> DepositReceipt {
    let caller = caller(); // 获取调用者
    let ledger_canister_id = STATE // 获取 icp 所在 canister id
        .with(|s| s.borrow().ledger)
        .unwrap_or(MAINNET_LEDGER_CANISTER_ID);

    let amount = if token_canister_id == ledger_canister_id {
        deposit_icp(caller).await? // 如果是 icp 转账，则调用 icp 存钱接口
    } else {
        deposit_token(caller, token_canister_id).await? // 其他 canister 转账
    };
    STATE.with(|s| {
        s.borrow_mut()
            .exchange
            .balances
            .add_balance(&caller, &token_canister_id, amount.to_owned()) // 调用者对该币种存入金额
    });
    DepositReceipt::Ok(amount) // 返回存入金额
}

// 把账户里所有的 icp 都存入
async fn deposit_icp(caller: Principal) -> Result<Nat, DepositErr> {
    let canister_id = ic_cdk::api::id(); // 当前 canister id
    let ledger_canister_id = STATE
        .with(|s| s.borrow().ledger)
        .unwrap_or(MAINNET_LEDGER_CANISTER_ID);

    let account = AccountIdentifier::new(&canister_id, &principal_to_subaccount(&caller));

    // 获取余额参数
    let balance_args = ic_ledger_types::AccountBalanceArgs { account };
    // 获取余额
    let balance = ic_ledger_types::account_balance(ledger_canister_id, balance_args)
        .await
        .map_err(|_| DepositErr::TransferFailure)?;

    if balance.e8s() < ICP_FEE {
        // 余额小于费用
        return Err(DepositErr::BalanceLow);
    }

    let transfer_args = ic_ledger_types::TransferArgs {
        memo: Memo(0),
        amount: balance - Tokens::from_e8s(ICP_FEE), // 转入扣除费用后的所有余额
        fee: Tokens::from_e8s(ICP_FEE),
        from_subaccount: Some(principal_to_subaccount(&caller)),
        to: AccountIdentifier::new(&canister_id, &DEFAULT_SUBACCOUNT), // 转入本 canister
        created_at_time: None,
    };
    ic_ledger_types::transfer(ledger_canister_id, transfer_args)
        .await
        .map_err(|_| DepositErr::TransferFailure)?
        .map_err(|_| DepositErr::TransferFailure)?;

    ic_cdk::println!(
        "Deposit of {} ICP in account {:?}",
        balance - Tokens::from_e8s(ICP_FEE),
        &account
    );

    Ok((balance.e8s() - ICP_FEE).into())
}

// 把账户里所有的 token 都存入
async fn deposit_token(caller: Principal, token: Principal) -> Result<Nat, DepositErr> {
    let token = DIP20::new(token); // 构造 token 对象
    let dip_fee = token.get_metadata().await.fee; // 获取转账费用

    let allowance = token.allowance(caller, ic_cdk::api::id()).await; // 获取允许本 canister id 的额度

    let available = allowance - dip_fee; // 允许额度减去费用

    token
        .transfer_from(caller, ic_cdk::api::id(), available.to_owned())
        .await
        .map_err(|_| DepositErr::TransferFailure)?;

    Ok(available)
}

// 获取调用者在本交易所某币种的余额
#[query(name = "getBalance")]
#[candid_method(query, rename = "getBalance")]
pub fn get_balance(token_canister_id: Principal) -> Nat {
    STATE.with(|s| s.borrow().exchange.get_balance(token_canister_id))
}

// 获取调用者在本交易所的所有代币余额
#[query(name = "getBalances")]
#[candid_method(query, rename = "getBalances")]
pub fn get_balances() -> Vec<Balance> {
    STATE.with(|s| s.borrow().exchange.get_balances())
}

// 获取所有人的余额
#[query(name = "getAllBalances")]
#[candid_method(query, rename = "getAllBalances")]
pub fn get_all_balances() -> Vec<Balance> {
    STATE.with(|s| s.borrow().exchange.get_all_balances())
}

// 查找订单
#[update(name = "getOrder")]
#[candid_method(update, rename = "getOrder")]
pub fn get_order(order: OrderId) -> Option<Order> {
    STATE.with(|s| s.borrow().exchange.get_order(order))
}

// 获取所有订单
#[update(name = "getOrders")]
#[candid_method(update, rename = "getOrders")]
pub fn get_orders() -> Vec<Order> {
    STATE.with(|s| s.borrow().exchange.get_all_orders())
}

// 获取存款地址
#[update(name = "getDepositAddress")]
#[candid_method(update, rename = "getDepositAddress")]
pub fn get_deposit_address() -> AccountIdentifier {
    let canister_id = ic_cdk::api::id(); // 当前 canister id
    let subaccount = principal_to_subaccount(&caller()); // ? 当前调用者的子账户

    AccountIdentifier::new(&canister_id, &subaccount) // ? 不知道这个能干啥用
}

// 获取代币的符号
#[update(name = "getSymbol")]
#[candid_method(update, rename = "getSymbol")]
pub async fn get_symbol(token_canister_id: Principal) -> String {
    let ledger_canister_id = STATE
        .with(|s| s.borrow().ledger)
        .unwrap_or(MAINNET_LEDGER_CANISTER_ID);

    if token_canister_id == ledger_canister_id {
        "ICP".to_string()
    } else {
        DIP20::new(token_canister_id).get_metadata().await.symbol
    }
}

// 下单
#[update(name = "placeOrder")]
#[candid_method(update, rename = "placeOrder")]
pub fn place_order(
    from_token_canister_id: Principal,
    from_amount: Nat,
    to_token_canister_id: Principal,
    to_amount: Nat,
) -> OrderPlacementReceipt {
    STATE.with(|s| {
        s.borrow_mut().exchange.place_order(
            from_token_canister_id,
            from_amount,
            to_token_canister_id,
            to_amount,
        )
    })
}

// 取消订单
#[update(name = "cancelOrder")]
#[candid_method(update, rename = "cancelOrder")]
pub fn cancel_order(order: OrderId) -> CancelOrderReceipt {
    STATE.with(|s| s.borrow_mut().exchange.cancel_order(order))
}

// 提现
#[update]
#[candid_method(update)]
pub async fn withdraw(
    token_canister_id: Principal,
    amount: Nat,
    address: Principal,
) -> WithdrawReceipt {
    let caller = caller();
    let ledger_canister_id = STATE
        .with(|s| s.borrow().ledger)
        .unwrap_or(MAINNET_LEDGER_CANISTER_ID);

    // Close all currently open orders to avoid completing orders
    // without funds.
    STATE.with(|s| {
        s.borrow_mut()
            .exchange
            .orders
            .retain(|_, v| v.owner != caller);
    });

    if token_canister_id == ledger_canister_id {
        let account_id = AccountIdentifier::new(&address, &DEFAULT_SUBACCOUNT);
        withdraw_icp(&amount, account_id).await
    } else {
        withdraw_token(token_canister_id, &amount, address).await
    }
}

// 提现 icp
async fn withdraw_icp(amount: &Nat, account_id: AccountIdentifier) -> Result<Nat, WithdrawErr> {
    let caller = caller();
    let ledger_canister_id = STATE
        .with(|s| s.borrow().ledger)
        .unwrap_or(MAINNET_LEDGER_CANISTER_ID);

    let sufficient_balance = STATE.with(|s| {
        s.borrow_mut().exchange.balances.subtract_balance(
            &caller,
            &ledger_canister_id,
            amount.to_owned() + ICP_FEE,
        )
    });
    if !sufficient_balance {
        return Err(WithdrawErr::BalanceLow);
    }

    let transfer_amount = Tokens::from_e8s(
        (amount.to_owned() + ICP_FEE)
            .0
            .try_into()
            .map_err(|_| WithdrawErr::TransferFailure)?,
    );

    let transfer_args = ic_ledger_types::TransferArgs {
        memo: Memo(0),
        amount: transfer_amount,
        fee: Tokens::from_e8s(ICP_FEE),
        from_subaccount: Some(DEFAULT_SUBACCOUNT),
        to: account_id,
        created_at_time: None,
    };
    let icp_receipt = ic_ledger_types::transfer(ledger_canister_id, transfer_args)
        .await
        .map_err(|_| WithdrawErr::TransferFailure)
        .and_then(|v| v.map_err(|_| WithdrawErr::TransferFailure));

    if let Err(e) = icp_receipt {
        STATE.with(|s| {
            s.borrow_mut().exchange.balances.add_balance(
                &caller,
                &ledger_canister_id,
                amount.to_owned() + ICP_FEE,
            )
        });

        return Err(e);
    }

    ic_cdk::println!("Withdrawal of {} ICP to account {:?}", amount, &account_id);

    Ok(amount.to_owned() + ICP_FEE)
}

// 提现代币
async fn withdraw_token(
    token: Principal,
    amount: &Nat,
    address: Principal,
) -> Result<Nat, WithdrawErr> {
    let caller = caller();
    let dip = DIP20::new(token);
    let dip_fee = dip.get_metadata().await.fee;

    let sufficient_balance = STATE.with(|s| {
        s.borrow_mut().exchange.balances.subtract_balance(
            &caller,
            &token,
            amount.to_owned() + dip_fee.to_owned(),
        )
    });
    if !sufficient_balance {
        return Err(WithdrawErr::BalanceLow);
    }

    let tx_receipt = dip
        .transfer(address, amount.to_owned() + dip_fee.to_owned())
        .await
        .map_err(|_| WithdrawErr::TransferFailure);

    if let Err(e) = tx_receipt {
        STATE.with(|s| {
            s.borrow_mut().exchange.balances.add_balance(
                &caller,
                &token,
                amount.to_owned() + dip_fee.to_owned(),
            )
        });

        return Err(e);
    }

    Ok(amount.to_owned() + dip_fee)
}

// 调用者 id
#[query]
#[candid_method(query)]
pub fn whoami() -> Principal {
    caller()
}

// For testing
#[update]
#[candid_method(oneway)]
pub fn credit(user: Principal, token_canister_id: Principal, amount: Nat) {
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        let owner = state.owner.unwrap();

        ic_cdk::println!("credit {} {}", caller(), owner);
        assert!(owner == caller());
        state
            .exchange
            .balances
            .add_balance(&user, &token_canister_id, amount);
    })
}

// For testing.
#[update]
#[candid_method(oneway)]
pub fn clear() {
    STATE.with(|s| {
        let mut state = s.borrow_mut();

        assert!(state.owner.unwrap() == caller());
        state.exchange.orders.clear();
        state.exchange.balances.0.clear();
    })
}

#[init]
fn init(ledger: Option<Principal>) {
    ic_cdk::setup();
    STATE.with(|s| {
        s.borrow_mut().owner = Some(caller());
        s.borrow_mut().ledger = ledger;
    });
}

// NOTE: Converting and storing state like this should not be used in production.
// If the state becomes too large, it can prevent future upgrades. This
// is left in as a tool during development. If removed, native types
// can be used throughout, instead.
#[pre_upgrade]
fn pre_upgrade() {
    let state = STATE.with(|s| s.take());

    // Transform into stable state
    let stable_state: stable::StableState = state.into();

    ic_cdk::storage::stable_save((stable_state,)).expect("failed to save stable state");
}

// NOTE: Converting and storing state like this should not be used in production.
// If the state becomes too large, it can prevent future upgrades. This
// is left in as a tool during development. If removed, native types
// can be used throughout, instead.
#[post_upgrade]
fn post_upgrade() {
    let (stable_state,): (stable::StableState,) =
        ic_cdk::storage::stable_restore().expect("failed to restore stable state");

    // Transform from stable state
    let state = stable_state.into();

    STATE.with(|s| {
        s.replace(state);
    });
}

export_service!();

#[ic_cdk_macros::query(name = "__get_candid_interface_tmp_hack")]
fn export_candid() -> String {
    __export_service()
}
