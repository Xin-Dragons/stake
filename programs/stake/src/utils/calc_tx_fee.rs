use anchor_lang::prelude::*;

use crate::state::{Staker, Subscription};

pub fn calc_tx_fee(staker: &Staker, fee: u64) -> u64 {
    let clock: Clock = Clock::get().unwrap();
    let current_time: i64 = clock.unix_timestamp;
    let next_payment_time: i64 = staker.next_payment_time;
    let staker_subscription: &Subscription = &staker.get_subscription();
    let has_bolt_ons: bool =
        staker.own_domain || staker.remove_branding || staker.collections.len() > 1;
    let grace_period: i64 = 60 * 60 * 24 * 7;
    let cut_off: i64 = next_payment_time + grace_period;

    // if payment has lapsed, subscription is basic, if they have bolt ons, penalty.
    let subscription: &Subscription = if current_time > cut_off {
        if has_bolt_ons {
            &Subscription::Penalty
        } else {
            &Subscription::Free
        }
    } else {
        staker_subscription
    };

    let fee: u64 = match subscription {
        Subscription::Penalty => fee * 2,
        Subscription::Advanced => fee / 2,
        Subscription::Pro => fee / 5,
        Subscription::Ultimate => 0,
        _ => fee,
    };

    fee
}
