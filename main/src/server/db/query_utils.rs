use crate::server::models::HistoryOrder;

pub fn clamp_history_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(100).clamp(1, 500)
}

pub fn clamp_history_offset(offset: Option<u32>) -> u32 {
    offset.unwrap_or(0).min(1_000_000)
}

pub fn history_order_to_sql(order: Option<HistoryOrder>) -> &'static str {
    match order.unwrap_or(HistoryOrder::Desc) {
        HistoryOrder::Asc => "ASC",
        HistoryOrder::Desc => "DESC",
    }
}
