#![allow(clippy::collapsible_else_if)]

#[macro_use]
extern crate ic_cdk_macros;
#[macro_use]
extern crate serde;

use std::collections::HashSet;
use std::convert::TryFrom;
use std::iter::FromIterator;

use candid::{Encode, Principal};
use ic_cdk::{
    api::{self, call},
    export::candid,
};

mod http;
mod stable;
mod types;

use stable::{DEFAULT_LOGO, MGMT, STATE};
use types::{
    ConstrainedError, Error, ExtendedMetadataResult, InitArgs, InterfaceId, LogoResult,
    MetadataDesc, MintResult, Nft, Result,
};

// 初始化系统
#[init]
fn init(args: InitArgs) {
    STATE.with(|state| {
        let mut state = state.borrow_mut(); // 取得操作对象
        state.custodians = args
            .custodians
            .unwrap_or_else(|| HashSet::from_iter([api::caller()])); // 设置控制人，默认为调用者
        state.name = args.name; // 设置名称
        state.symbol = args.symbol; // 设置符号
        state.logo = args.logo; // 设置 logo
    });
}

// --------------
// base interface
// --------------

// 查询某用户拥有的 nft 数量
#[query(name = "balanceOfDip721")]
fn balance_of(user: Principal) -> u64 {
    STATE.with(|state| {
        state
            .borrow()
            .nfts
            .iter()
            .filter(|n| n.owner == user)
            .count() as u64
    })
}

// 查询某 nft 对应的所有者
#[query(name = "ownerOfDip721")]
fn owner_of(token_id: u64) -> Result<Principal> {
    STATE.with(|state| {
        let owner = state
            .borrow()
            .nfts
            .get(usize::try_from(token_id)?)
            .ok_or(Error::InvalidTokenId)?
            .owner;
        Ok(owner)
    })
}

// 对授权的 nft 进行转账
#[update(name = "transferFromDip721")]
fn transfer_from(from: Principal, to: Principal, token_id: u64) -> Result {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let state = &mut *state;
        let nft = state
            .nfts
            .get_mut(usize::try_from(token_id)?)
            .ok_or(Error::InvalidTokenId)?; // 找到这个 nft
        let caller = api::caller();
        if nft.owner != caller // 该 nft 不属于调用者
            && nft.approved != Some(caller) // 该 nft 没有授权给调用者
            && !state
                .operators
                .get(&from)
                .map(|s| s.contains(&caller))
                .unwrap_or(false) // 该拥有者授权的操作者
            && !state.custodians.contains(&caller)
        // 控制人里面也没有调用者
        {
            Err(Error::Unauthorized)
        } else if nft.owner != from {
            // nft 的所有者必须是 from
            Err(Error::Other)
        } else {
            nft.approved = None; // 移除任何授权
            nft.owner = to; // 所有权给目标
            Ok(state.next_txid()) // 返回交易 id，会递增一个新的交易 id
        }
    })
}

// 安全的转移 nft，额外检查了目标地址不能为空
#[update(name = "safeTransferFromDip721")]
fn safe_transfer_from(from: Principal, to: Principal, token_id: u64) -> Result {
    if to == MGMT {
        Err(Error::ZeroAddress)
    } else {
        transfer_from(from, to, token_id)
    }
}

// ? 支持的接口
#[query(name = "supportedInterfacesDip721")]
fn supported_interfaces() -> &'static [InterfaceId] {
    &[
        InterfaceId::TransferNotification,
        // InterfaceId::Approval, // Psychedelic/DIP721#5
        InterfaceId::Burn,
        InterfaceId::Mint,
    ]
}

// 查询 logo
#[export_name = "canister_query logoDip721"]
fn logo() /* -> &'static LogoResult */
{
    ic_cdk::setup();
    // 这个好像是 http 的返回啊，为啥在这里
    STATE.with(|state| call::reply((state.borrow().logo.as_ref().unwrap_or(&DEFAULT_LOGO),)))
}

// 查询名称
#[query(name = "nameDip721")]
fn name() -> String {
    STATE.with(|state| state.borrow().name.clone())
}

// 查询符号
#[query(name = "symbolDip721")]
fn symbol() -> String {
    STATE.with(|state| state.borrow().symbol.clone())
}

// 查询总供应量
#[query(name = "totalSupplyDip721")]
fn total_supply() -> u64 {
    STATE.with(|state| state.borrow().nfts.len() as u64)
}

// 查询元数据
#[export_name = "canister_query getMetadataDip721"]
fn get_metadata(/* token_id: u64 */) /* -> Result<&'static MetadataDesc> */
{
    ic_cdk::setup();
    let token_id = call::arg_data::<(u64,)>().0; // 尝试取得第一个参数
    let res: Result<()> = STATE.with(|state| {
        let state = state.borrow();
        let metadata = &state
            .nfts
            .get(usize::try_from(token_id)?)
            .ok_or(Error::InvalidTokenId)?
            .metadata;
        call::reply((Ok::<_, Error>(metadata),));
        Ok(())
    });
    if let Err(e) = res {
        call::reply((Err::<MetadataDesc, _>(e),));
    }
}

// 取得某用户的元数据
#[export_name = "canister_update getMetadataForUserDip721"]
fn get_metadata_for_user(/* user: Principal */) /* -> Vec<ExtendedMetadataResult> */
{
    ic_cdk::setup();
    let user = call::arg_data::<(Principal,)>().0; // 取得第一个参数
    STATE.with(|state| {
        let state = state.borrow();
        let metadata: Vec<_> = state
            .nfts
            .iter()
            .filter(|n| n.owner == user)
            .map(|n| ExtendedMetadataResult {
                metadata_desc: &n.metadata,
                token_id: n.id,
            })
            .collect();
        call::reply((metadata,));
    });
}

// ----------------------
// notification interface
// ----------------------

// 转账带通知
#[update(name = "transferFromNotifyDip721")]
fn transfer_from_notify(from: Principal, to: Principal, token_id: u64, data: Vec<u8>) -> Result {
    let res = transfer_from(from, to, token_id)?;
    // 不知道这个 Encode 宏是干嘛用的
    if let Ok(arg) = Encode!(&api::caller(), &from, &token_id, &data) {
        // Using call_raw ensures we don't need to await the future for the call to be executed.
        // Calling an arbitrary function like this means that a malicious recipient could call
        // transferFromNotifyDip721 in their onDIP721Received function, resulting in an infinite loop.
        // This will trap eventually, but the transfer will have already been completed and the state-change persisted.
        // That means the original transfer must reply before that happens, or the caller will be
        // convinced that the transfer failed when it actually succeeded. So we don't await the call,
        // so that we'll reply immediately regardless of how long the notification call takes.
        let _ = api::call::call_raw(to, "onDIP721Received", &arg, 0); // 通知对方接收到 nft
    }
    Ok(res)
}

// 转账带通知 额外检查目标地址
#[update(name = "safeTransferFromNotifyDip721")]
fn safe_transfer_from_notify(
    from: Principal,
    to: Principal,
    token_id: u64,
    data: Vec<u8>,
) -> Result {
    if to == MGMT {
        Err(Error::ZeroAddress)
    } else {
        transfer_from_notify(from, to, token_id, data)
    }
}

// ------------------
// approval interface
// ------------------

// 授权 1 个 nft
#[update(name = "approveDip721")]
fn approve(user: Principal, token_id: u64) -> Result {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let state = &mut *state; // 取得系统状态数据
        let caller = api::caller(); // 调用者
                                    // 取得该 nft
        let nft = state
            .nfts
            .get_mut(usize::try_from(token_id)?)
            .ok_or(Error::InvalidTokenId)?;
        if nft.owner != caller // 该 nft 不属于调用者
            && nft.approved != Some(caller) // 该 nft 没有被授权
            && !state // 调用者 也不在所有授权列表
                .operators
                .get(&user)
                .map(|s| s.contains(&caller))
                .unwrap_or(false)
            && !state.custodians.contains(&caller)
        // 调用者 不是安全管理员
        {
            Err(Error::Unauthorized) // 未授权错误
        } else {
            nft.approved = Some(user); // 把该 nft 授权给 user
            Ok(state.next_txid())
        }
    })
}

// 授权所有的 nft 权限
#[update(name = "setApprovalForAllDip721")]
fn set_approval_for_all(operator: Principal, is_approved: bool) -> Result {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let caller = api::caller(); // 调用者
        if operator != caller {
            // 授权目标不能是调用者自己
            let operators = state.operators.entry(caller).or_default();
            if operator == MGMT {
                // 授权目标不能是空的
                if !is_approved {
                    // 如果是取消授权，则清空所有的授权用户
                    operators.clear();
                } else {
                    // cannot enable everyone as an operator
                    // 不能把所有人都设置为授权用户
                }
            } else {
                if is_approved {
                    operators.insert(operator); // 如果是授权，则加入授权列表
                } else {
                    operators.remove(&operator); // 如果是取消授权，则从授权列表中移除
                }
            }
        }
        Ok(state.next_txid())
    })
}

// 获取授权信息
// #[query(name = "getApprovedDip721")] // Psychedelic/DIP721#5
fn _get_approved(token_id: u64) -> Result<Principal> {
    STATE.with(|state| {
        let approved = state
            .borrow()
            .nfts
            .get(usize::try_from(token_id)?)
            .ok_or(Error::InvalidTokenId)?
            .approved
            .unwrap_or_else(api::caller);
        Ok(approved)
    })
}

// 调用者检查是否对该用户授权了所有 nft
#[query(name = "isApprovedForAllDip721")]
fn is_approved_for_all(operator: Principal) -> bool {
    STATE.with(|state| {
        state
            .borrow()
            .operators
            .get(&api::caller())
            .map(|s| s.contains(&operator))
            .unwrap_or(false)
    })
}

// --------------
// mint interface
// --------------

// 铸币
#[update(name = "mintDip721")]
fn mint(
    to: Principal,          // 目标用户
    metadata: MetadataDesc, // 元数据信息
    blob_content: Vec<u8>,  // 元数据内容
) -> Result<MintResult, ConstrainedError> {
    let (txid, tkid) = STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.custodians.contains(&api::caller()) {
            // 调用者不在管理员列表，则报错
            return Err(ConstrainedError::Unauthorized);
        }
        let new_id = state.nfts.len() as u64; // 之前的长度就是新的 id
        let nft = Nft {
            owner: to,
            approved: None,
            id: new_id,
            metadata,
            content: blob_content,
        };
        state.nfts.push(nft); // 插入
        Ok((state.next_txid(), new_id))
    })?;
    http::add_hash(tkid); // 调用 add_hash 方法，把新的 nft 添加到哈希表中
    Ok(MintResult {
        id: txid,
        token_id: tkid,
    })
}

// --------------
// burn interface
// --------------

// 销毁 nft
#[update(name = "burnDip721")]
fn burn(token_id: u64) -> Result {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let nft = state
            .nfts
            .get_mut(usize::try_from(token_id)?)
            .ok_or(Error::InvalidTokenId)?;
        if nft.owner != api::caller() {
            // 调用者不是所有者，不能销毁
            Err(Error::Unauthorized)
        } else {
            nft.owner = MGMT; // 所有者设置成空地址，就是销毁了？？
            Ok(state.next_txid())
        }
    })
}

// 设置名称
#[update]
fn set_name(name: String) -> Result<()> {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.custodians.contains(&api::caller()) {
            state.name = name;
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    })
}

// 设置符号
#[update]
fn set_symbol(sym: String) -> Result<()> {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.custodians.contains(&api::caller()) {
            state.symbol = sym;
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    })
}

// 设置 logo
#[update]
fn set_logo(logo: Option<LogoResult>) -> Result<()> {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.custodians.contains(&api::caller()) {
            state.logo = logo;
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    })
}

// 设置管理员
#[update]
fn set_custodian(user: Principal, custodian: bool) -> Result<()> {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.custodians.contains(&api::caller()) {
            // 管理员才能调用这个方法
            if custodian {
                state.custodians.insert(user); // 如果是设置管理员，插入这个用户
            } else {
                state.custodians.remove(&user); // 如果是取消管理员，移除这个用户
            }
            Ok(())
        } else {
            Err(Error::Unauthorized)
        }
    })
}

// 查询某个地址是不是管理员
#[query]
fn is_custodian(principal: Principal) -> bool {
    STATE.with(|state| state.borrow().custodians.contains(&principal))
}
