use std::convert::TryInto;

use candid::{Nat, Principal};
use ic_ledger_types::Subaccount;
use num_bigint::BigUint;
use num_traits::Zero;

// 获取标准 0
pub fn zero() -> Nat {
    Nat(BigUint::zero())
}

// principal 转换为 Subaccount
pub fn principal_to_subaccount(principal_id: &Principal) -> Subaccount {
    let mut subaccount = [0; std::mem::size_of::<Subaccount>()]; // 定义长度，是其固定长度 32
    let principal_id = principal_id.as_slice(); // 返回一个 u8 的切片 最大长度 29 位
    subaccount[0] = principal_id.len().try_into().unwrap(); // ? 第一个放长度？
    subaccount[1..1 + principal_id.len()].copy_from_slice(principal_id); // 复制对应的位

    Subaccount(subaccount)
}
