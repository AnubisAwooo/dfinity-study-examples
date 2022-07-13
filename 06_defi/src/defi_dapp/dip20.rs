use candid::{CandidType, Deserialize, Nat, Principal};

// DIP20 标准的代币
pub struct DIP20 {
    principal: Principal,
}

// 代币转账错误类型
#[derive(CandidType, Debug, PartialEq, Deserialize)]
pub enum TxError {
    InsufficientBalance,   // 余额不足
    InsufficientAllowance, // 授权不足
    Unauthorized,          // 未授权
    LedgerTrap,            // 账本陷阱？
    AmountTooSmall,        // 数量太小
    BlockUsed,             // 黑名单用户？
    ErrorOperationStyle,   // 错误操作类型
    ErrorTo,               // 错误目标地址
    Other,                 // 其他原因
}
// 转账收据
pub type TxReceipt = Result<Nat, TxError>;

// 币种元数据
#[allow(non_snake_case)]
#[derive(CandidType, Clone, Debug, Deserialize)]
pub struct Metadata {
    pub logo: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub totalSupply: Nat,
    pub owner: Principal,
    pub fee: Nat,
}

impl DIP20 {
    pub fn new(principal: Principal) -> Self {
        DIP20 { principal }
    }

    pub async fn transfer(&self, target: Principal, amount: Nat) -> TxReceipt {
        let call_result: Result<(TxReceipt,), _> =
            ic_cdk::api::call::call(self.principal, "transfer", (target, amount)).await;

        call_result.unwrap().0
    }

    pub async fn transfer_from(
        &self,
        source: Principal,
        target: Principal,
        amount: Nat,
    ) -> TxReceipt {
        let call_result: Result<(TxReceipt,), _> =
            ic_cdk::api::call::call(self.principal, "transferFrom", (source, target, amount)).await;

        call_result.unwrap().0
    }

    pub async fn allowance(&self, owner: Principal, spender: Principal) -> Nat {
        let call_result: Result<(Nat,), _> =
            ic_cdk::api::call::call(self.principal, "allowance", (owner, spender)).await;

        call_result.unwrap().0
    }

    pub async fn get_metadata(&self) -> Metadata {
        let call_result: Result<(Metadata,), _> =
            ic_cdk::api::call::call(self.principal, "getMetadata", ()).await;

        call_result.unwrap().0
    }
}
