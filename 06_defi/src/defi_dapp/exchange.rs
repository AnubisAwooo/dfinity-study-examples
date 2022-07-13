use std::collections::HashMap;

use candid::{Nat, Principal};
use ic_cdk::caller;

use crate::types::*;
use crate::{utils, OrderId};

// 余额模型 某个所属人 对每个币种的余额
#[derive(Default)]
pub struct Balances(pub HashMap<Principal, HashMap<Principal, Nat>>); // owner -> token_canister_id -> amount
type Orders = HashMap<OrderId, Order>; // 所有订单列表

// 交易所模型
#[derive(Default)]
pub struct Exchange {
    pub next_id: OrderId,   // 下一个订单号
    pub balances: Balances, // 每个人的账户余额
    pub orders: Orders,     // 所有订单
}

// 给账户增加方法
impl Balances {
    // 增加余额方法  所有人 目标币种 变更数量
    pub fn add_balance(&mut self, owner: &Principal, token_canister_id: &Principal, delta: Nat) {
        let balances = self.0.entry(*owner).or_insert_with(HashMap::new);

        if let Some(x) = balances.get_mut(token_canister_id) {
            *x += delta; // 如果之前有余额，修改变动
        } else {
            balances.insert(*token_canister_id, delta); // 如果之前没有余额，新增余额
        }
    }

    // 减去余额的方法  返回成功或失败
    // Tries to subtract balance from user account. Checks for overflows
    pub fn subtract_balance(
        &mut self,
        owner: &Principal,
        token_canister_id: &Principal,
        delta: Nat,
    ) -> bool {
        if let Some(balances) = self.0.get_mut(owner) {
            if let Some(x) = balances.get_mut(token_canister_id) {
                if *x >= delta {
                    // 余额大于变动数量则减去
                    *x -= delta;
                    // no need to keep an empty token record
                    if *x == utils::zero() {
                        // 如果减为 0 的，则删除该余额
                        balances.remove(token_canister_id);
                    }
                    return true;
                } else {
                    return false;
                }
            }
        }

        false
    }
}

// 为交易所添加方法
impl Exchange {
    // 获取当前调用者某个币种的余额方法
    pub fn get_balance(&self, token_canister_id: Principal) -> Nat {
        self.balances
            .0
            .get(&caller())
            .and_then(|v| v.get(&token_canister_id))
            .map_or(utils::zero(), |v| v.to_owned())
    }

    // 获取当前调用者所有余额
    pub fn get_balances(&self) -> Vec<Balance> {
        match self.balances.0.get(&caller()) {
            None => Vec::new(),
            Some(v) => v
                .iter()
                .map(|(token_canister_id, amount)| Balance {
                    owner: caller(),
                    token: *token_canister_id,
                    amount: amount.to_owned(),
                })
                .collect(),
        }
    }

    // 获取所有余额
    pub fn get_all_balances(&self) -> Vec<Balance> {
        self.balances
            .0
            .iter()
            .flat_map(|(owner, balances)| {
                balances.iter().map(move |(token, amount)| Balance {
                    owner: *owner,
                    token: *token,
                    amount: amount.to_owned(),
                })
            })
            .collect()
    }

    // 查询订单
    pub fn get_order(&self, order: OrderId) -> Option<Order> {
        self.orders.get(&order).cloned()
    }

    // 获取所有订单
    pub fn get_all_orders(&self) -> Vec<Order> {
        self.orders.iter().map(|(_, o)| o.clone()).collect()
    }

    // 下单
    pub fn place_order(
        &mut self,
        from_token_canister_id: Principal,
        from_amount: Nat,
        to_token_canister_id: Principal,
        to_amount: Nat,
    ) -> OrderPlacementReceipt {
        ic_cdk::println!("place order");
        if from_amount <= utils::zero() || to_amount <= utils::zero() {
            // 交换数量不能小于等于 0
            return OrderPlacementReceipt::Err(OrderPlacementErr::InvalidOrder);
        }

        if self.check_for_sell_orders(from_token_canister_id) {
            // 当前币种已经有卖单了，不能再下单
            return OrderPlacementReceipt::Err(OrderPlacementErr::InvalidOrder);
        }

        let balance = self.get_balance(from_token_canister_id);
        if balance < from_amount {
            // 余额不足
            return OrderPlacementReceipt::Err(OrderPlacementErr::InvalidOrder);
        }
        let id = self.next_id(); // 取得 id
        self.orders.insert(
            id,
            Order {
                id,
                owner: caller(),
                from: from_token_canister_id,
                fromAmount: from_amount,
                to: to_token_canister_id,
                toAmount: to_amount,
            },
        );
        self.resolve_order(id)?; // ? 估计是进行订单处理的方法

        if let Some(o) = self.orders.get(&id) {
            OrderPlacementReceipt::Ok(Some(o.clone()))
        } else {
            OrderPlacementReceipt::Ok(None)
        }
    }

    // 检查当前币种的卖单
    pub fn check_for_sell_orders(&self, from_token_canister_id: Principal) -> bool {
        self.orders
            .values()
            .any(|v| (v.from == from_token_canister_id) && (v.owner == caller()))
    }

    // 取消订单
    pub fn cancel_order(&mut self, order: OrderId) -> CancelOrderReceipt {
        if let Some(o) = self.orders.get(&order) {
            if o.owner == caller() {
                self.orders.remove(&order); // 直接移除就算取消了
                CancelOrderReceipt::Ok(order)
            } else {
                CancelOrderReceipt::Err(CancelOrderErr::NotAllowed)
            }
        } else {
            CancelOrderReceipt::Err(CancelOrderErr::NotExistingOrder)
        }
    }

    // 处理订单？
    fn resolve_order(&mut self, id: OrderId) -> Result<(), OrderPlacementErr> {
        ic_cdk::println!("resolve order");
        let mut matches = Vec::new(); // 匹配的订单
        let a = self.orders.get(&id).unwrap(); // 本订单
        for (order, b) in self.orders.iter() {
            if *order == id {
                // 本订单不继续
                continue;
            }

            if a.from == b.to && a.to == b.from {
                // 订单类型能和本订单匹配
                // Simplified to use multiplication from
                // (a.fromAmount / a.toAmount) * (b.fromAmount / b.toAmount) >= 1 // 对方的买价高于我的卖价
                // which checks that this pair of trades is profitable.
                if a.fromAmount.to_owned() * b.fromAmount.to_owned()
                    >= a.toAmount.to_owned() * b.toAmount.to_owned()
                {
                    ic_cdk::println!(
                        "match {}: {} -> {}, {}: {} -> {}",
                        id,
                        a.fromAmount,
                        a.toAmount,
                        *order,
                        b.fromAmount,
                        b.toAmount
                    );
                    matches.push((a.to_owned(), b.to_owned())); // 找到的订单
                }
            }
        }
        for m in matches {
            let mut a_to_amount: Nat = utils::zero();
            let mut b_to_amount: Nat = utils::zero();
            let (a, b) = m;
            // Check if some orders can be completed in their entirety.
            if b.fromAmount >= a.toAmount {
                // 买单给的数量大于卖单需要的数量
                a_to_amount = a.toAmount.to_owned(); // 卖单需要的数量就是卖单指定的数量
            }
            if a.fromAmount >= b.toAmount {
                // 卖单给的数量大于买单需要的数量
                b_to_amount = b.toAmount.to_owned(); // 买单需要的数量就是买单指定的数量
            }
            // ? 看不明白这 2 个比较啥意思？
            // Check if some orders can be completed partially.
            if check_orders(
                a.to_owned(),
                b.to_owned(),
                &mut a_to_amount,
                b_to_amount.to_owned(),
            ) {
                continue;
            }
            if check_orders(
                b.to_owned(),
                a.to_owned(),
                &mut b_to_amount,
                a_to_amount.to_owned(),
            ) {
                continue;
            }

            if a_to_amount > utils::zero() && b_to_amount > utils::zero() {
                self.process_trade(a.id, b.id, a_to_amount, b_to_amount)?;
            }
        }

        Ok(())
    }

    fn process_trade(
        &mut self,
        a: OrderId,
        b: OrderId,
        a_to_amount: Nat,
        b_to_amount: Nat,
    ) -> Result<(), OrderPlacementErr> {
        ic_cdk::println!("process trade {} {} {} {}", a, b, a_to_amount, b_to_amount);

        let Exchange {
            orders, balances, ..
        } = self;

        let mut order_a = orders.remove(&a).unwrap();
        let mut order_b = orders.remove(&b).unwrap();

        // Calculate "cost" to each
        let a_from_amount =
            (a_to_amount.to_owned() * order_a.fromAmount.to_owned()) / order_a.toAmount.to_owned();
        let b_from_amount =
            (b_to_amount.to_owned() * order_b.fromAmount.to_owned()) / order_b.toAmount.to_owned();

        // Update order with remaining tokens
        order_a.fromAmount -= a_from_amount.to_owned();
        order_a.toAmount -= a_to_amount.to_owned();

        order_b.fromAmount -= b_from_amount.to_owned();
        order_b.toAmount -= b_to_amount.to_owned();

        // Update DEX balances
        balances.subtract_balance(&order_a.owner, &order_a.from, a_from_amount.to_owned());
        balances.add_balance(&order_a.owner, &order_a.to, a_to_amount.to_owned());

        balances.subtract_balance(&order_b.owner, &order_b.from, b_from_amount.to_owned());
        balances.add_balance(&order_b.owner, &order_b.to, b_to_amount.to_owned());

        // The DEX keeps any tokens not required to satisfy the parties.
        let dex_amount_a = a_from_amount - b_to_amount;
        if dex_amount_a > utils::zero() {
            balances.add_balance(&ic_cdk::id(), &order_a.from, dex_amount_a);
        }

        let dex_amount_b = b_from_amount - a_to_amount;
        if dex_amount_b > utils::zero() {
            balances.add_balance(&ic_cdk::id(), &order_b.from, dex_amount_b);
        }

        // Maintain the order only if not empty
        if order_a.fromAmount != utils::zero() {
            orders.insert(order_a.id, order_a);
        }

        if order_b.fromAmount != utils::zero() {
            orders.insert(order_b.id, order_b);
        }

        Ok(())
    }

    fn next_id(&mut self) -> OrderId {
        self.next_id += 1;
        self.next_id
    }
}

// ? 检查订单 不知道怎么使用的
fn check_orders(
    first: Order,              // 卖单
    second: Order,             // 买单
    first_to_amount: &mut Nat, // 卖单想要的数量
    second_to_amount: Nat,     // 买单想要的数量
) -> bool {
    if *first_to_amount == utils::zero() && second_to_amount > utils::zero() {
        // 如果卖单没有想要的数量了，但买单还有想给的数量
        *first_to_amount = second.fromAmount;
        // Verify that we can complete the partial order with natural number tokens remaining.
        if ((first_to_amount.to_owned() * first.fromAmount) % first.toAmount) != utils::zero() {
            // 能成交的比例
            // 卖单想给的数量 % 买单想要的数量
            return true;
        }
    }

    false
}
