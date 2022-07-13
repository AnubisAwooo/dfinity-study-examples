use candid::{CandidType, Nat, Principal};

// 订单 id
pub type OrderId = u32;

// 订单的数据结构
#[allow(non_snake_case)]
#[derive(CandidType, Clone)]
pub struct Order {
    pub id: OrderId,      // 订单 id
    pub owner: Principal, // 订单的拥有者
    pub from: Principal,  // 交出的币种
    pub fromAmount: Nat,  // 交出的数量
    pub to: Principal,    // 获取的币种
    pub toAmount: Nat,    // 获取的数量
}

// 账户余额模型
#[derive(CandidType)]
pub struct Balance {
    pub owner: Principal, // 余额所属人
    pub token: Principal, // 目标币种
    pub amount: Nat,      // 余额
}

// 取消订单的收据
pub type CancelOrderReceipt = Result<OrderId, CancelOrderErr>;

// 取消订单的错误类型
#[derive(CandidType)]
pub enum CancelOrderErr {
    NotAllowed,       // 不允许取消订单
    NotExistingOrder, // 不存在的订单
}

// 存款的收据
pub type DepositReceipt = Result<Nat, DepositErr>;

#[derive(CandidType)]
pub enum DepositErr {
    BalanceLow,      // 余额不足
    TransferFailure, // 转账失败
}

// 下单收据
pub type OrderPlacementReceipt = Result<Option<Order>, OrderPlacementErr>;

#[derive(CandidType)]
pub enum OrderPlacementErr {
    InvalidOrder,  // 无效的订单
    OrderBookFull, // 订单薄已满
}

// 提现收据
pub type WithdrawReceipt = Result<Nat, WithdrawErr>;

#[derive(CandidType)]
pub enum WithdrawErr {
    BalanceLow,      // 余额不足
    TransferFailure, // 转账失败
}
