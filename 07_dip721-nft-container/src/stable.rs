use std::borrow::Cow;
use std::cell::RefCell;
use std::mem;

use ic_certified_map::{Hash, RbTree};

use candid::Principal;
use ic_cdk::{export::candid, storage};

use crate::types::{LogoResult, StableState};

use include_base64::include_base64;

use crate::types::State;

// ? 不知道干嘛的
// 目前来看，是当做空地址使用的
pub const MGMT: Principal = Principal::from_slice(&[]);

// 默认 logo ？
pub const DEFAULT_LOGO: LogoResult = LogoResult {
    data: Cow::Borrowed(include_base64!("logo.png")), // 把本地的logo.png加载进来 base64编码
    logo_type: Cow::Borrowed("image/png"),            // logo 类型
};

thread_local! {
   pub static STATE: RefCell<State> = RefCell::default(); // 存储系统数据
}

thread_local! {
    // sha256("Total NFTs: 0") = 83d0f670865c367ce95f595959abec46ed7b64033ecee9ed772e78793f3bc10f
    pub static HASHES: RefCell<RbTree<String, Hash>> = RefCell::new(RbTree::from_iter([("/".to_string(), *b"\x83\xd0\xf6\x70\x86\x5c\x36\x7c\xe9\x5f\x59\x59\x59\xab\xec\x46\xed\x7b\x64\x03\x3e\xce\xe9\xed\x77\x2e\x78\x79\x3f\x3b\xc1\x0f")]));
}

#[pre_upgrade]
fn pre_upgrade() {
    // ? 看不明白这个写法是干嘛的 意思是把里面的东西拿走，并把一个空的放进去 空的怎么初始化的？
    let state = STATE.with(|state| mem::take(&mut *state.borrow_mut()));
    // 拿到 hash 记录
    let hashes = HASHES.with(|hashes| mem::take(&mut *hashes.borrow_mut()));
    // hash 变成数组
    let hashes = hashes.iter().map(|(k, v)| (k.clone(), *v)).collect();
    let stable_state = StableState { state, hashes }; // 构建升级对象
    storage::stable_save((stable_state,)).unwrap();
}

#[post_upgrade]
fn post_upgrade() {
    let (StableState { state, hashes },) = storage::stable_restore().unwrap();
    STATE.with(|state0| *state0.borrow_mut() = state); // 恢复系统数据
    let hashes = hashes.into_iter().collect();
    HASHES.with(|hashes0| *hashes0.borrow_mut() = hashes); // 恢复 hash 数据
}
