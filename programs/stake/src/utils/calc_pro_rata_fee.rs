use anchor_lang::prelude::*;

use crate::StakeError;

pub fn calc_pro_rata_fee(next_payment_time: i64, fee: u64) -> Result<u64> {
    if fee == 0 {
        return Ok(0);
    }
    let current_time = Clock::get().unwrap().unix_timestamp;
    let thirty_days: i64 = 60 * 60 * 24 * 30;
    let payable_time: i64 = next_payment_time - current_time;
    let factor: i64 = thirty_days / payable_time;

    let factor_u64 = match u64::try_from(factor) {
        Ok(time) => time,
        _ => {
            return err!(StakeError::FailedTimeConversion);
        }
    };

    Ok(fee * factor_u64)
}
