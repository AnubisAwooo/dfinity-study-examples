use std::collections::HashMap;
use std::iter::FromIterator;

use ic_cdk::api::{self, call};
use ic_certified_map::AsHashTree;
use percent_encoding::percent_decode_str;
use serde::Serialize;
use serde_cbor::Serializer;
use sha2::{Digest, Sha256};

use crate::stable::HASHES;
use crate::types::{HttpResponse, MetadataPurpose, MetadataVal};
use crate::{stable::STATE, types::HttpRequest};

// 请求 nft 数据
// This could reply with a lot of data. To return this data from the function would require it to be cloned,
// because the thread_local! closure prevents us from returning data borrowed from inside it.
// Luckily, it doesn't actually get returned from the exported WASM function, that's just an abstraction.
// What happens is it gets fed to call::reply, and we can do that explicitly to save the cost of cloning the data.
// #[query] calls call::reply unconditionally, and calling it twice would trap, so we use #[export_name] directly.
// This requires duplicating the rest of the abstraction #[query] provides for us, like setting up the panic handler with
// ic_cdk::setup() and fetching the function parameters via call::arg_data.
// cdk 0.5 makes this unnecessary, but it has not been released at the time of writing this example.
#[export_name = "canister_query http_request"]
fn http_request(/* req: HttpRequest */) /* -> HttpResponse */
{
    ic_cdk::setup();
    let req = call::arg_data::<(HttpRequest,)>().0; // 取得请求参数，也就是请求体
    STATE.with(|state| {
        let state = state.borrow();
        let url = req.url.split('?').next().unwrap_or("/"); // 分割出 url，默认是 /
        let cert = format!(
            "certificate=:{}:, tree=:{}:",
            base64::encode(api::data_certificate().unwrap()), // 认证信息写上
            witness(&url)                                     // 一般来说会找到对应的证明
        )
        .into();
        let mut path =
            url[1..] // 第 1 个斜线不要
                .split('/') // 按斜线分割
                .map(|segment| percent_decode_str(segment).decode_utf8().unwrap()); // ? 不知道干嘛的

        let mut headers = HashMap::from_iter([
            (
                "Content-Security-Policy",
                "default-src 'self' ; script-src 'none' ; frame-src 'none' ; object-src 'none'"
                    .into(),
            ),
            ("IC-Certificate", cert), // 默认的响应头
        ]);
        if cfg!(mainnet) {
            // 如果是主网，要求必须通过 https 访问
            headers.insert(
                "Strict-Transport-Security",
                "max-age=31536000; includeSubDomains".into(),
            );
        }
        let root = path.next().unwrap_or_else(|| "".into()); // 这里取得的应该是第一个参数，是 token id
        let body;
        let mut code = 200;
        if root == "" {
            // 没有路径返回默认的总数信息
            body = format!("Total NFTs: {}", state.nfts.len())
                .into_bytes()
                .into();
        } else {
            if let Ok(num) = root.parse::<usize>() {
                // 目前一直路径是 /something
                // /:something
                if let Some(nft) = state.nfts.get(num) {
                    // /:nft
                    let img = path.next().unwrap_or_else(|| "".into()); // 下一个路径
                    if img == "" {
                        // 如果是空，则表示只指明了正确的 nft 的 token id
                        // /:nft/
                        // 找到第一个渲染属性的元数据，找不到则是第一个数据
                        let part = nft
                            .metadata
                            .iter()
                            .find(|x| x.purpose == MetadataPurpose::Rendered)
                            .or_else(|| nft.metadata.get(0));
                        if let Some(part) = part {
                            // default metadata: first non-preview metadata, or if there is none, first metadata
                            body = part.data.as_slice().into(); // 设置响应体为元数据的数据
                            if let Some(MetadataVal::TextContent(mime)) =
                                part.key_val_data.get("contentType")
                            // 如果有设置 contentType，则需要设置响应头
                            {
                                headers.insert("Content-Type", mime.as_str().into());
                            }
                        } else {
                            // 没有任何元数据
                            // no metadata to be found
                            body = b"No metadata for this NFT"[..].into();
                        }
                    } else {
                        // 第 2 个路径也有东西
                        // /:nft/:something
                        if let Ok(num) = img.parse::<usize>() {
                            // /:nft/:number
                            if let Some(part) = nft.metadata.get(num) {
                                // /:nft/:id
                                body = part.data.as_slice().into(); // 设置对应的元数据的数据
                                if let Some(MetadataVal::TextContent(mime)) =
                                    part.key_val_data.get("contentType")
                                // 如果有设置 contentType，则需要设置响应头
                                {
                                    headers.insert("Content-Type", mime.as_str().into());
                                }
                            } else {
                                // 找不到第 2 个数字对应的元数据
                                code = 404;
                                body = b"No such metadata part"[..].into();
                            }
                        } else {
                            // 第 2 个路径不是数字
                            code = 400;
                            body = format!("Invalid metadata ID {}", img).into_bytes().into();
                        }
                    }
                } else {
                    // 转换成数字了，但是没有对应的 nft
                    code = 404;
                    body = b"No such NFT"[..].into();
                }
            } else {
                // 第一个路径无法转换成数字，就是无效的 token id
                code = 400;
                body = format!("Invalid NFT ID {}", root).into_bytes().into();
            }
        }
        call::reply((HttpResponse {
            status_code: code,
            headers,
            body,
        },));
    });
}

// 增加新的证明  每次生成新的 nft 时调用
pub fn add_hash(tkid: u64) {
    crate::STATE.with(|state| {
        HASHES.with(|hashes| {
            let state = state.borrow(); // 取得当前系统状态数据
            let mut hashes = hashes.borrow_mut(); // 取得 hash 数据
            let nft = state.nfts.get(tkid as usize)?; // 获取 NFT 数据
            let mut default = false;
            for (i, metadata) in nft.metadata.iter().enumerate() {
                let hash = Sha256::digest(&metadata.data);
                hashes.insert(format!("/{}/{}", tkid, i), hash.into()); // 把该 nft 对应的每一个元数据内容的 hash 值插入
                if !default && matches!(metadata.purpose, MetadataPurpose::Rendered) {
                    // 如果匹配到了渲染的元数据，则当成默认的 hash 值
                    default = true;
                    hashes.insert(format!("/{}", tkid), hash.into());
                }
            }
            // 更新单独 / 的 hash 值
            hashes.insert(
                "/".to_string(),
                Sha256::digest(format!("Total NFTs: {}", state.nfts.len())).into(),
            );
            let cert = ic_certified_map::labeled_hash(b"http_assets", &hashes.root_hash());
            api::set_certified_data(&cert); // ? 不知道干嘛的  应该是设置 canister 的证明？
            Some(())
        })
    });
}

// ? 见证？不知道啥意思
fn witness(name: &str) -> String {
    HASHES.with(|hashes| {
        let hashes = hashes.borrow();
        let witness = hashes.witness(name.as_bytes()); // 如果 key 不在 hashes 中，则返回一个证明？
        let tree = ic_certified_map::labeled(b"http_assets", witness); // ? 不知道干啥的
        let mut data = vec![];
        let mut serializer = Serializer::new(&mut data); // 序列化器？把数据写到 data 里面？
        serializer.self_describe().unwrap(); // ? 不知道干啥的
        tree.serialize(&mut serializer).unwrap(); // 把 tree 里面的内容序列化出来
        base64::encode(data) // 返回编码后的结果
    })
}
