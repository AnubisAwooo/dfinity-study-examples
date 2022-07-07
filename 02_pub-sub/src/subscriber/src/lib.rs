use candid::{CandidType, Principal};
use ic_cdk_macros::*;
use serde::Deserialize;

use std::cell::Cell;

thread_local! {
    static COUNTER: Cell<u64> = Cell::new(0);
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct Counter {
    topic: String,
    value: u64,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
struct Subscriber {
    topic: String,
}

// 设置订阅，告诉发布者 principal id 和要订阅的主题
#[update]
#[candid::candid_method(update)]
async fn setup_subscribe(publisher_id: Principal, topic: String) {
    let subscriber = Subscriber { topic };
    let _call_result: Result<(), _> = ic_cdk::call(publisher_id, "subscribe", (subscriber,)).await;
}

// 更新计数器
#[update]
#[candid::candid_method(update)]
fn update_count(counter: Counter) {
    COUNTER.with(|c| {
        c.set(c.get() + counter.value);
    });
}

// 查询 counter 的值
#[query]
#[candid::candid_method(query)]
fn get_count() -> u64 {
    COUNTER.with(|c| c.get())
}
