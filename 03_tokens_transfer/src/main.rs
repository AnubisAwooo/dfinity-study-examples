mod lib;

#[allow(unused_imports)]
use crate::lib::Conf;
#[allow(unused_imports)]
use crate::lib::TransferArgs;
#[allow(unused_imports)]
use candid::Principal;
#[allow(unused_imports)]
use ic_ledger_types::BlockIndex;

#[cfg(any(target_arch = "wasm32", test))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", test)))]
fn main() {
    ic_cdk::export::candid::export_service!();
    std::println!("{}", __export_service());
}
