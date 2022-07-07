use candid::{candid_method, CandidType, Principal};
use ic_cdk_macros::*;
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct Subscriber {
    topic: String,
}

type SubscriberStore = BTreeMap<Principal, Subscriber>;

thread_local! {
    static SUBSCRIBERS: RefCell<SubscriberStore> = RefCell::default();
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct Counter {
    topic: String,
    value: u64,
}

// 给订阅者调用，需提供订阅的主题 caller 就是订阅者的 id
#[update]
#[candid_method(update)]
fn subscribe(subscriber: Subscriber) {
    let subscriber_principal_id = ic_cdk::caller(); // 调用者的 principal id
    SUBSCRIBERS.with(|subscribers| {
        subscribers
            .borrow_mut()
            .insert(subscriber_principal_id, subscriber)
    });
}

// 发布新的消息
#[update]
#[candid_method(update)]
fn publish(counter: Counter) {
    SUBSCRIBERS.with(|subscribers| {
        for (k, v) in subscribers.borrow().iter() {
            if v.topic == counter.topic {
                // 遍历每一个订阅了对应主题的订阅者，通知消息
                let _call_result: Result<(), _> = ic_cdk::notify(*k, "update_count", (&counter,));
            }
        }
    });
}
