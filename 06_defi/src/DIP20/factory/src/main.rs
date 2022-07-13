use ic_kit::{
    candid::{candid_method, encode_args, CandidType, Deserialize, Nat},
    ic,
    interfaces::{management, Method},
    macros::*,
    Principal, RejectionCode,
};

#[derive(CandidType, Deserialize)]
pub enum TokenType {
    DIP20Motoko,
}

// 错误类型
#[derive(CandidType, Deserialize)]
pub enum FactoryError {
    CreateCanisterError, // 创建 canister 错误
    CanisterStatusNotAvailableError, // 合约状态不可用错误
    EncodeError, // 编码错误
    CodeAlreadyInstalled, // 代码已安装
    InstallCodeError, // 按照代码错误
}

// 读取目标 canister 的二进制代码
const WASM: &[u8] = include_bytes!("../../motoko/.dfx/local/canisters/token/token.wasm");

// 创建 canister
#[update]
#[candid_method(update)]
pub async fn create(
    logo: String, // logo
    name: String, // 名称
    symbol: String, // 符号
    decimals: u8, // 小数位数
    total_supply: Nat, // 总供应量
    owner: Principal, // 拥有者
    mut controllers: Vec<Principal>, // 控制者列表，控制者可以修改状态，修改代码
    cycles: u64, // 不知道这是啥
    fee: Nat, // 转账费用
    _token_type: TokenType, // 代币类型
) -> Result<Principal, FactoryError> {
    // 所有者必须是调用者
    assert_eq!(
        ic_kit::ic::caller(),
        owner,
        "only the owner of this contract can call the create method"
    );

    // create canister
    controllers.push(ic_kit::ic::id()); // 先把自己的 canister id 加入到控制者里面
    // 创建设置
    let create_settings = management::CanisterSettings {
        controllers: Some(controllers),
        compute_allocation: None,
        memory_allocation: None,
        freezing_threshold: None,
    };
    use management::{CanisterStatus, InstallMode, WithCanisterId};

    // 创建 canister 参数
    let arg = management::CreateCanisterArgument {
        settings: Some(create_settings),
    };
    // 创建一个空的 canister
    let (res,) = match management::CreateCanister::perform_with_payment(
        Principal::management_canister(),
        (arg,),
        cycles, // 创建 canister 会扣除 cycles 吗？
    )
    .await
    {
        Err(_) => return Err(FactoryError::CreateCanisterError),
        Ok(res) => res,
    };

    // 取得创建 canister 的 id
    let canister_id = res.canister_id;

    // 取得 canister 的状态
    // install code
    let (response,) = match CanisterStatus::perform(
        Principal::management_canister(),
        (WithCanisterId { canister_id },),
    )
    .await
    {
        Err(_) => return Err(FactoryError::CanisterStatusNotAvailableError),
        Ok(res) => res,
    };

    if response.module_hash.is_some() {
        return Err(FactoryError::CodeAlreadyInstalled);
    }

    #[derive(CandidType, Deserialize)]
    struct InstallCodeArgumentBorrowed<'a> {
        mode: InstallMode,
        canister_id: Principal,
        #[serde(with = "serde_bytes")]
        wasm_module: &'a [u8],
        arg: Vec<u8>,
    }

    // 编码参数
    let arg = match encode_args((logo, name, symbol, decimals, total_supply, owner, fee)) {
        Err(_) => return Err(FactoryError::EncodeError),
        Ok(res) => res,
    };

    // 按照配置
    let install_config = InstallCodeArgumentBorrowed {
        mode: InstallMode::Install, // 安装方式
        canister_id, // 木匾 canister id
        wasm_module: WASM, // 二进制代码
        arg, // 参数
    };

    if (ic::call(
        Principal::management_canister(), // 这个返回管理 canister 的 id，调用它的方法进行安装
        "install_code",
        (install_config,),
    )
    .await as Result<(), (RejectionCode, std::string::String)>)
        .is_err()
    {
        return Err(FactoryError::InstallCodeError);
    }

    Ok(canister_id)
}

// 初始化时直接设置所有者
#[init]
pub fn init(owner: Principal) {
    ic_kit::ic::store(owner);
}

#[pre_upgrade]
pub fn pre_upgrade() {
    // 把所有者临时保存
    ic_kit::ic::stable_store((owner(),)).expect("unable to store data in stable storage")
}

#[post_upgrade]
pub fn post_upgrade() {
    let (owner,) = ic_kit::ic::stable_restore::<(Principal,)>()
        .expect("unable to restore data in stable storage");
    // 恢复所有者
    ic_kit::ic::store(owner);
}

查询所有者
#[query]
#[candid_method(query)]
pub fn owner() -> Principal {
    *ic_kit::ic::get_maybe::<Principal>().expect("owner not set")
}

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    candid::export_service!();
    std::print!("{}", __export_service());
}
