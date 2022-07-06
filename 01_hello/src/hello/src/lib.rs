#[ic_cdk_macros::query]
#[candid::candid_method(query)]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}
