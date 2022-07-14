use candid::{CandidType, Principal};

use ic_certified_map::Hash;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::num::TryFromIntError;
use std::result::Result as StdResult;

// logo数据
#[derive(CandidType, Deserialize, Clone)]
pub struct LogoResult {
    pub logo_type: Cow<'static, str>, // logo类型
    pub data: Cow<'static, str>,      // 数据
}

// 初始化参数
#[derive(CandidType, Deserialize)]
pub struct InitArgs {
    pub custodians: Option<HashSet<Principal>>, // 控制人
    pub logo: Option<LogoResult>,               // logo数据
    pub name: String,                           // 名称
    pub symbol: String,                         // 符号
}

// 元数据的值
#[allow(clippy::enum_variant_names)]
#[derive(CandidType, Deserialize)]
pub enum MetadataVal {
    TextContent(String),  // 文本类型
    BlobContent(Vec<u8>), // 二进制类型
    NatContent(u128),     // 数字类型 rust 居然有 128 位的数字
    Nat8Content(u8),      // u8 类型
    Nat16Content(u16),    // u16 类型
    Nat32Content(u32),    // u32 类型
    Nat64Content(u64),    // u64 类型
}

// 元数据处理方式
#[derive(CandidType, Deserialize, PartialEq)]
pub enum MetadataPurpose {
    Preview,  // ? 预览
    Rendered, // ? 渲染
}

// 元数据的键值对
#[derive(CandidType, Deserialize)]
pub struct MetadataPart {
    pub purpose: MetadataPurpose,                   // ? 处理方式
    pub key_val_data: HashMap<String, MetadataVal>, // 键值对
    pub data: Vec<u8>,                              // 实际数据
}

pub type MetadataDesc = Vec<MetadataPart>; // 多个元数据
pub type MetadataDescRef<'a> = &'a [MetadataPart]; // 多个元数据的引用

// Nft 的结构体
#[derive(CandidType, Deserialize)]
pub struct Nft {
    pub owner: Principal,            // 所属人
    pub approved: Option<Principal>, // 授权人
    pub id: u64,                     // nft 的 id，这个数字才是真正的 nft
    pub metadata: MetadataDesc,      // 该 nft 的元数据
    pub content: Vec<u8>,            // ? nft的内容
}

// 系统状态数据
#[derive(CandidType, Deserialize, Default)]
pub struct State {
    pub nfts: Vec<Nft>,                                    // 所有的 nft
    pub custodians: HashSet<Principal>,                    // 系统的控制人
    pub operators: HashMap<Principal, HashSet<Principal>>, // owner to operators // ? 看不明白啥意思 猜测是可以把某个人的 nft 全部授权出去
    pub logo: Option<LogoResult>,                          // logo 信息
    pub name: String,                                      // NFT 的名称
    pub symbol: String,                                    // NFT 的符号
    pub txid: u128,                                        // 交易 id, 每笔交易应该会递增
}

impl State {
    pub fn next_txid(&mut self) -> u128 {
        let txid = self.txid;
        self.txid += 1; // 保存下一个 id
        txid // 返回当前的
    }
}

// 升级时保存数据的结构体
#[derive(CandidType, Deserialize)]
pub struct StableState {
    pub state: State,                // 系统数据
    pub hashes: Vec<(String, Hash)>, // ? 不知道干啥的
}

// 错误类型
#[derive(CandidType, Deserialize)]
pub enum Error {
    Unauthorized,   // 未授权
    InvalidTokenId, // ? 无效的 token id
    ZeroAddress,    // 空地址
    Other,          // 其他
}

impl From<TryFromIntError> for Error {
    fn from(_: TryFromIntError) -> Self {
        Self::InvalidTokenId // 数字转换错误一律可以自动转换为该错误
    }
}

// 函数响应结果
pub type Result<T = u128, E = Error> = StdResult<T, E>;

// 拓展的元数据结果
#[derive(CandidType)]
pub struct ExtendedMetadataResult<'a> {
    pub metadata_desc: MetadataDescRef<'a>,
    pub token_id: u64,
}

// 铸币结果
#[derive(CandidType, Deserialize)]
pub struct MintResult {
    pub token_id: u64, // 生成的 token 的序号
    pub id: u128,      // 铸币交易的 id
}

// ? 接口 id
#[derive(CandidType, Deserialize)]
pub enum InterfaceId {
    Approval,             // 授权
    TransactionHistory,   // 交易历史
    Mint,                 // 铸币
    Burn,                 // 销毁
    TransferNotification, // 转账通知
}

// ? 约束错误 ？
#[derive(CandidType, Deserialize)]
pub enum ConstrainedError {
    Unauthorized, // 未授权
}

// http 请求的结构体
#[derive(CandidType, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

// http 响应的结构体
#[derive(CandidType)]
pub struct HttpResponse<'a> {
    pub status_code: u16,
    pub headers: HashMap<&'a str, Cow<'a, str>>,
    pub body: Cow<'a, [u8]>,
}
