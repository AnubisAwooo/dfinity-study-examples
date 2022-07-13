/**
* Module     : main.rs
* Copyright  : 2021 DFinance Team
* License    : Apache 2.0 with LLVM Exception
* Maintainer : DFinance Team <hello@dfinance.ai>
* Stability  : Experimental
*/
use candid::{candid_method, CandidType, Deserialize, Int, Nat};
use cap_sdk::{handshake, insert, Event, IndefiniteEvent, TypedEvent};
use cap_std::dip20::cap::DIP20Details;
use cap_std::dip20::{Operation, TransactionStatus, TxRecord};
use ic_kit::{ic, Principal};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::convert::Into;
use std::iter::FromIterator;
use std::string::String;

// 交易日志
#[derive(CandidType, Default, Deserialize)]
pub struct TxLog {
    pub ie_records: VecDeque<IndefiniteEvent>,
}

// ? 获得一个可编辑的日志？
pub fn tx_log<'a>() -> &'a mut TxLog {
    ic_kit::ic::get_mut::<TxLog>()
}

// token 的元数据
#[allow(non_snake_case)]
#[derive(Deserialize, CandidType, Clone, Debug)]
struct Metadata {
    logo: String,     // logo
    name: String,     // 名称
    symbol: String,   // 符号
    decimals: u8,     // 精度
    totalSupply: Nat, // 总供应量
    owner: Principal, // 拥有者
    fee: Nat,         // 转账交易费用
}

// ? 状态数据
#[derive(Deserialize, CandidType, Clone, Debug)]
struct StatsData {
    logo: String,
    name: String,
    symbol: String,
    decimals: u8,
    total_supply: Nat,
    owner: Principal,
    fee: Nat,
    fee_to: Principal,   // 交易费转入地址
    history_size: usize, // ? 历史大小
    deploy_time: u64,    // ? 部署时间 创建时间吧
}

// token 信息
#[allow(non_snake_case)]
#[derive(Deserialize, CandidType, Clone, Debug)]
struct TokenInfo {
    metadata: Metadata, // 元数据
    feeTo: Principal,   // 交易费转入地址
    // status info
    historySize: usize,  // ? 历史大小
    deployTime: u64,     // ? 部署时间戳
    holderNumber: usize, // 持有者数量
    cycles: u64,         // ? cycles
}

// 状态数据的默认值
impl Default for StatsData {
    fn default() -> Self {
        StatsData {
            logo: "".to_string(),
            name: "".to_string(),
            symbol: "".to_string(),
            decimals: 0u8,
            total_supply: Nat::from(0),
            owner: Principal::anonymous(),
            fee: Nat::from(0),
            fee_to: Principal::anonymous(),
            history_size: 0,
            deploy_time: 0,
        }
    }
}

type Balances = HashMap<Principal, Nat>; // 每个人的余额
type Allowances = HashMap<Principal, HashMap<Principal, Nat>>; // 每个人允许其他人的授权金额

// 交易的错误类型
#[derive(CandidType, Debug, PartialEq)]
pub enum TxError {
    InsufficientBalance,   // 余额不足
    InsufficientAllowance, // 授权额度不足
    Unauthorized,          // 未授权
    LedgerTrap,            // ? 账簿陷阱 ？
    AmountTooSmall,        // 金额太小
    BlockUsed,             // ? 拉黑用户
    ErrorOperationStyle,   // 操作类型错误
    ErrorTo,               // 错误目标地址
    Other,                 // 其他错误
}
pub type TxReceipt = Result<Nat, TxError>; // 交易收据，要么成功返回 id，要么是错误

// 初始化函数
#[init]
#[candid_method(init)]
fn init(
    logo: String,
    name: String,
    symbol: String,
    decimals: u8,
    total_supply: Nat,
    owner: Principal,
    fee: Nat,
    fee_to: Principal,
    cap: Principal,
) {
    let stats = ic::get_mut::<StatsData>();
    stats.logo = logo;
    stats.name = name;
    stats.symbol = symbol;
    stats.decimals = decimals;
    stats.total_supply = total_supply.clone();
    stats.owner = owner;
    stats.fee = fee;
    stats.fee_to = fee_to;
    stats.history_size = 1;
    stats.deploy_time = ic::time();
    handshake(1_000_000_000_000, Some(cap));
    let balances = ic::get_mut::<Balances>();
    // 先给创建者所有的代币
    balances.insert(owner, total_supply.clone());
    // 增加一条记录
    let _ = add_record(
        owner,
        Operation::Mint,
        owner,
        owner,
        total_supply,
        Nat::from(0),
        ic::time(),
        TransactionStatus::Succeeded,
    );
}

// 内部调用的转账，未做安全检查
fn _transfer(from: Principal, to: Principal, value: Nat) {
    let balances = ic::get_mut::<Balances>();
    let from_balance = balance_of(from); // 取得原账户余额
    let from_balance_new = from_balance - value.clone(); // 计算原账户新余额
    if from_balance_new != 0 {
        balances.insert(from, from_balance_new); // 不是0 要重新插入
    } else {
        balances.remove(&from); // 已经是 0 了，就移除
    }
    let to_balance = balance_of(to); // 目标账户余额
    let to_balance_new = to_balance + value; // 目标账户新的余额
    if to_balance_new != 0 {
        balances.insert(to, to_balance_new); // 插入目标账户余额
    }
}

// 扣除转账费用
fn _charge_fee(user: Principal, fee_to: Principal, fee: Nat) {
    let stats = ic::get::<StatsData>(); // 获取状态数据
    if stats.fee > Nat::from(0) {
        // 如果需要收取转账费用，则向收集地址转入费用
        _transfer(user, fee_to, fee);
    }
}

// 转账方法
#[update(name = "transfer")]
#[candid_method(update)]
async fn transfer(to: Principal, value: Nat) -> TxReceipt {
    let from = ic::caller(); // 原账户，即调用者
    let stats = ic::get_mut::<StatsData>(); // 获取系统状态数据
    if balance_of(from) < value.clone() + stats.fee.clone() {
        // 如果原账户余额不足，则返回错误
        return Err(TxError::InsufficientBalance);
    }
    _charge_fee(from, stats.fee_to, stats.fee.clone()); // 先扣除费用
    _transfer(from, to, value.clone()); // 进行转账
    stats.history_size += 1; // 历史记录数量加 1

    // 增加记录
    add_record(
        from,
        Operation::Transfer,
        from,
        to,
        value,
        stats.fee.clone(),
        ic::time(),
        TransactionStatus::Succeeded,
    )
    .await
}

// 从允许额度中转账
#[update(name = "transferFrom")]
#[candid_method(update, rename = "transferFrom")]
async fn transfer_from(from: Principal, to: Principal, value: Nat) -> TxReceipt {
    let owner = ic::caller(); // 取得调用者
    let from_allowance = allowance(from, owner); // 取得原账户给调用者的额度
    let stats = ic::get_mut::<StatsData>(); // 取得系统数据
    if from_allowance < value.clone() + stats.fee.clone() {
        // 允许的额度不够转账和手续费，则返回错误
        return Err(TxError::InsufficientAllowance);
    }
    let from_balance = balance_of(from); // 取得原账户余额
    if from_balance < value.clone() + stats.fee.clone() {
        // 如果原账户余额不足，则返回错误
        return Err(TxError::InsufficientBalance);
    }
    _charge_fee(from, stats.fee_to, stats.fee.clone()); // 扣除费用
    _transfer(from, to, value.clone()); // 转账
    let allowances = ic::get_mut::<Allowances>();
    match allowances.get(&from) {
        Some(inner) => {
            let result = inner.get(&owner).unwrap().clone(); // 获取原额度
            let mut temp = inner.clone();
            if result.clone() - value.clone() - stats.fee.clone() != 0 {
                // 剩下额度不为 0，要更新
                temp.insert(owner, result.clone() - value.clone() - stats.fee.clone());
                allowances.insert(from, temp);
            } else {
                temp.remove(&owner); // 剩下额度为 0，则移除
                if temp.len() == 0 {
                    // 若无其他人的额度，则移除整个
                    allowances.remove(&from);
                } else {
                    // 插入新的状态
                    allowances.insert(from, temp);
                }
            }
        }
        None => {
            assert!(false);
        }
    }
    stats.history_size += 1; // 转账历史加 1

    // 增加转账记录
    add_record(
        owner,
        Operation::TransferFrom,
        from,
        to,
        value,
        stats.fee.clone(),
        ic::time(),
        TransactionStatus::Succeeded,
    )
    .await
}

// 授权额度
#[update(name = "approve")]
#[candid_method(update)]
async fn approve(spender: Principal, value: Nat) -> TxReceipt {
    let owner = ic::caller(); // 原账户，即调用者
    let stats = ic::get_mut::<StatsData>(); // 获取系统状态数据
    if balance_of(owner) < stats.fee.clone() {
        // 余额甚至不足以扣除手续费，则返回错误
        return Err(TxError::InsufficientBalance);
    }
    _charge_fee(owner, stats.fee_to, stats.fee.clone()); // 授权也要扣除手续费啊
    let v = value.clone() + stats.fee.clone(); // 实际授权额度应该是授权额度加上手续费
    let allowances = ic::get_mut::<Allowances>();
    match allowances.get(&owner) {
        Some(inner) => {
            let mut temp = inner.clone();
            if v.clone() != 0 {
                temp.insert(spender, v.clone()); // 插入新的授权额度
                allowances.insert(owner, temp);
            } else {
                temp.remove(&spender); // 新的授权额度为 0，则移除，，这里有个 bug 啊，明明已经加收手续费，还要判断是不是 0，明显不是 0 啊
                if temp.len() == 0 {
                    allowances.remove(&owner);
                } else {
                    allowances.insert(owner, temp);
                }
            }
        }
        None => {
            // 之前未授权过额度
            if v.clone() != 0 {
                let mut inner = HashMap::new();
                inner.insert(spender, v.clone());
                let allowances = ic::get_mut::<Allowances>();
                allowances.insert(owner, inner);
            }
        }
    }
    stats.history_size += 1; // 历史记录加 1

    // 增加记录
    add_record(
        owner,
        Operation::Approve,
        owner,
        spender,
        v,
        stats.fee.clone(),
        ic::time(),
        TransactionStatus::Succeeded,
    )
    .await
}

// 铸币
#[update(name = "mint")]
#[candid_method(update, rename = "mint")]
async fn mint(to: Principal, amount: Nat) -> TxReceipt {
    let caller = ic::caller(); // 调用者
    let stats = ic::get_mut::<StatsData>();
    if caller != stats.owner {
        // 非所有人，不允许铸币
        return Err(TxError::Unauthorized);
    }
    let to_balance = balance_of(to); // 目标地址余额
    let balances = ic::get_mut::<Balances>();
    balances.insert(to, to_balance + amount.clone()); // 插入新的余额
    stats.total_supply += amount.clone(); // 修改总供应量
    stats.history_size += 1; // 历史记录加 1

    add_record(
        caller,
        Operation::Mint,
        caller,
        to,
        amount,
        Nat::from(0),
        ic::time(),
        TransactionStatus::Succeeded,
    )
    .await
}

// 销毁
#[update(name = "burn")]
#[candid_method(update, rename = "burn")]
async fn burn(amount: Nat) -> TxReceipt {
    let caller = ic::caller(); // 调用者
    let stats = ic::get_mut::<StatsData>();
    let caller_balance = balance_of(caller); // 调用者余额
    if caller_balance.clone() < amount.clone() {
        // 余额不足，则返回错误
        return Err(TxError::InsufficientBalance);
    }
    let balances = ic::get_mut::<Balances>();
    balances.insert(caller, caller_balance - amount.clone()); // 插入剩下的余额
    stats.total_supply -= amount.clone(); // 减少总供应量
    stats.history_size += 1; // 历史记录加 1

    // 增加记录
    add_record(
        caller,
        Operation::Burn,
        caller,
        caller,
        amount,
        Nat::from(0),
        ic::time(),
        TransactionStatus::Succeeded,
    )
    .await
}

// 设置 token 名字
#[update(name = "setName")]
#[candid_method(update, rename = "setName")]
fn set_name(name: String) {
    let stats = ic::get_mut::<StatsData>();
    assert_eq!(ic::caller(), stats.owner); // 只有所有者才能设置名字
    stats.name = name;
}

// 设置 token logo
#[update(name = "setLogo")]
#[candid_method(update, rename = "setLogo")]
fn set_logo(logo: String) {
    let stats = ic::get_mut::<StatsData>();
    assert_eq!(ic::caller(), stats.owner); // 只有所有者才能设置 logo
    stats.logo = logo;
}

// 设置转账费用
#[update(name = "setFee")]
#[candid_method(update, rename = "setFee")]
fn set_fee(fee: Nat) {
    let stats = ic::get_mut::<StatsData>();
    assert_eq!(ic::caller(), stats.owner); // 只有所有者才能设置费用
    stats.fee = fee;
}

// 设置费用接收地址
#[update(name = "setFeeTo")]
#[candid_method(update, rename = "setFeeTo")]
fn set_fee_to(fee_to: Principal) {
    let stats = ic::get_mut::<StatsData>();
    assert_eq!(ic::caller(), stats.owner); // 只有所有者才能设置费用接收地址
    stats.fee_to = fee_to;
}

// 设置所有者
#[update(name = "setOwner")]
#[candid_method(update, rename = "setOwner")]
fn set_owner(owner: Principal) {
    let stats = ic::get_mut::<StatsData>();
    assert_eq!(ic::caller(), stats.owner); // 只有所有者才能设置费用接收地址
    stats.owner = owner;
}

// 取得某账户余额
#[query(name = "balanceOf")]
#[candid_method(query, rename = "balanceOf")]
fn balance_of(id: Principal) -> Nat {
    let balances = ic::get::<Balances>();
    match balances.get(&id) {
        Some(balance) => balance.clone(),
        None => Nat::from(0),
    }
}

// 查询原账户给调用者的额度
#[query(name = "allowance")]
#[candid_method(query)]
fn allowance(owner: Principal, spender: Principal) -> Nat {
    let allowances = ic::get::<Allowances>(); // 获取许可表
    match allowances.get(&owner) {
        Some(inner) => match inner.get(&spender) {
            Some(value) => value.clone(),
            None => Nat::from(0),
        },
        None => Nat::from(0),
    }
}

// 获取 token 的 logo
#[query(name = "logo")]
#[candid_method(query, rename = "logo")]
fn get_logo() -> String {
    let stats = ic::get::<StatsData>();
    stats.logo.clone()
}

// 获取 token 的名字
#[query(name = "name")]
#[candid_method(query)]
fn name() -> String {
    let stats = ic::get::<StatsData>();
    stats.name.clone()
}

// 获取 token 的 symbol
#[query(name = "symbol")]
#[candid_method(query)]
fn symbol() -> String {
    let stats = ic::get::<StatsData>();
    stats.symbol.clone()
}

// 获取 token 的精度
#[query(name = "decimals")]
#[candid_method(query)]
fn decimals() -> u8 {
    let stats = ic::get::<StatsData>();
    stats.decimals
}

// 获取 token 的总供应量
#[query(name = "totalSupply")]
#[candid_method(query, rename = "totalSupply")]
fn total_supply() -> Nat {
    let stats = ic::get::<StatsData>();
    stats.total_supply.clone()
}

// 获取 token 的所有者
#[query(name = "owner")]
#[candid_method(query)]
fn owner() -> Principal {
    let stats = ic::get::<StatsData>();
    stats.owner
}

// 获取 token 的元数据
#[query(name = "getMetadata")]
#[candid_method(query, rename = "getMetadata")]
fn get_metadata() -> Metadata {
    let s = ic::get::<StatsData>().clone();
    Metadata {
        logo: s.logo,
        name: s.name,
        symbol: s.symbol,
        decimals: s.decimals,
        totalSupply: s.total_supply,
        owner: s.owner,
        fee: s.fee,
    }
}

// 获取 token 的历史变动数量
#[query(name = "historySize")]
#[candid_method(query, rename = "historySize")]
fn history_size() -> usize {
    let stats = ic::get::<StatsData>();
    stats.history_size
}

// 获取 token 信息  还有其他一些信息，费用接收地址，历史数量大小，创建时间，持有者数量 cycles？这个啥意思不知道
#[query(name = "getTokenInfo")]
#[candid_method(query, rename = "getTokenInfo")]
fn get_token_info() -> TokenInfo {
    let stats = ic::get::<StatsData>().clone();
    let balance = ic::get::<Balances>();

    return TokenInfo {
        metadata: get_metadata(),
        feeTo: stats.fee_to,
        historySize: stats.history_size,
        deployTime: stats.deploy_time,
        holderNumber: balance.len(),
        cycles: ic::balance(),
    };
}

// 获取持有者及其余额 排序偏移查询
#[query(name = "getHolders")]
#[candid_method(query, rename = "getHolders")]
fn get_holders(start: usize, limit: usize) -> Vec<(Principal, Nat)> {
    let mut balance = Vec::new();
    for (k, v) in ic::get::<Balances>().clone() {
        balance.push((k, v));
    }
    balance.sort_by(|a, b| b.1.cmp(&a.1));
    let limit: usize = if start + limit > balance.len() {
        // 如果超出总数量，则限制到最后
        balance.len() - start
    } else {
        limit
    };
    balance[start..start + limit].to_vec()
}

// 获取总的授权数量大小
#[query(name = "getAllowanceSize")]
#[candid_method(query, rename = "getAllowanceSize")]
fn get_allowance_size() -> usize {
    let mut size = 0;
    let allowances = ic::get::<Allowances>();
    for (_, v) in allowances.iter() {
        size += v.len();
    }
    size
}

// 获取某个地址授权给其他地址的信息
#[query(name = "getUserApprovals")]
#[candid_method(query, rename = "getUserApprovals")]
fn get_user_approvals(who: Principal) -> Vec<(Principal, Nat)> {
    let allowances = ic::get::<Allowances>();
    match allowances.get(&who) {
        Some(allow) => return Vec::from_iter(allow.clone().into_iter()),
        None => return Vec::new(),
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    candid::export_service!();
    std::print!("{}", __export_service());
}

// 升级前数据处理
#[pre_upgrade]
fn pre_upgrade() {
    ic::stable_store((
        ic::get::<StatsData>().clone(), // 系统信息
        ic::get::<Balances>(),          // 每个人余额信息
        ic::get::<Allowances>(),        // 每个地址的授权信息
        tx_log(),                       // 交易日志
    ))
    .unwrap();
}

// 升级后恢复数据
#[post_upgrade]
fn post_upgrade() {
    let (metadata_stored, balances_stored, allowances_stored, tx_log_stored): (
        StatsData,
        Balances,
        Allowances,
        TxLog,
    ) = ic::stable_restore().unwrap();
    let stats = ic::get_mut::<StatsData>();
    *stats = metadata_stored; // 设置系统信息

    let balances = ic::get_mut::<Balances>();
    *balances = balances_stored; // 设置账户余额信息

    let allowances = ic::get_mut::<Allowances>();
    *allowances = allowances_stored; // 设置地址的授权信息

    let tx_log = tx_log();
    *tx_log = tx_log_stored; // 恢复交易日志
}

// 增加一条记录，返回交易收据
async fn add_record(
    caller: Principal,         // 调用者
    op: Operation,             // 行为
    from: Principal,           // 从
    to: Principal,             // 到
    amount: Nat,               // 数量
    fee: Nat,                  // 费用
    timestamp: u64,            // 时间戳
    status: TransactionStatus, // 交易状态
) -> TxReceipt {
    insert_into_cap(Into::<IndefiniteEvent>::into(Into::<Event>::into(Into::<
        TypedEvent<DIP20Details>,
    >::into(
        TxRecord {
            caller: Some(caller),
            index: Nat::from(0),
            from,
            to,
            amount: Nat::from(amount),
            fee: Nat::from(fee),
            timestamp: Int::from(timestamp),
            status,
            operation: op,
        },
    ))))
    .await
}

pub async fn insert_into_cap(ie: IndefiniteEvent) -> TxReceipt {
    let tx_log = tx_log();
    if let Some(failed_ie) = tx_log.ie_records.pop_front() {
        let _ = insert_into_cap_priv(failed_ie).await;
    }
    insert_into_cap_priv(ie).await
}

async fn insert_into_cap_priv(ie: IndefiniteEvent) -> TxReceipt {
    let insert_res = insert(ie.clone())
        .await
        .map(|tx_id| Nat::from(tx_id))
        .map_err(|_| TxError::Other);

    if insert_res.is_err() {
        tx_log().ie_records.push_back(ie.clone());
    }

    insert_res
}
