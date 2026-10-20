//! What Arbiter needs from an exchange, apart from any one exchange. Blocking calls: they run on
//! the runner's thread or in a staged handler's prepare step, never under a lock.

use crate::model::{Balance, Candle, ExchangeFill, ExchangeOrder, Fees, Granularity, OrderRequest, Permissions, Preview, Product, Quote};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum ExchangeError {
    /// No key is saved, or the exchange refused it (401/403).
    Auth(String),
    /// The exchange answered with a refusal for this request (400, 404, a failure reason).
    Refused(String),
    /// Too many requests (429).
    RateLimited,
    /// The network or the exchange is down: nothing is known about the request's effect.
    Unreachable(String),
    /// An answer this client could not read.
    Malformed(String),
    /// Create refused a `client_order_id` the exchange already has: the order exists, though this
    /// answer does not say what became of it.
    Duplicate(String),
}

impl fmt::Display for ExchangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExchangeError::Auth(m) => write!(f, "The exchange refused the key: {m}"),
            ExchangeError::Refused(m) => write!(f, "The exchange refused: {m}"),
            ExchangeError::RateLimited => write!(f, "The exchange asked to slow down"),
            ExchangeError::Unreachable(m) => write!(f, "The exchange could not be reached: {m}"),
            ExchangeError::Malformed(m) => write!(f, "The exchange's answer could not be read: {m}"),
            ExchangeError::Duplicate(m) => write!(f, "The exchange already has this order: {m}"),
        }
    }
}

impl std::error::Error for ExchangeError {}

pub type Result<T> = std::result::Result<T, ExchangeError>;

/// Public market data: needs no key.
pub trait Market: Send + Sync {
    /// Every spot product.
    fn products(&self) -> Result<Vec<Product>>;
    fn product(&self, id: &str) -> Result<Product>;
    /// Candles with `start` in `[start, end)`, Unix seconds, oldest first. At most
    /// [`MAX_CANDLES`] per call.
    fn candles(&self, product: &str, granularity: Granularity, start: i64, end: i64) -> Result<Vec<Candle>>;
    fn quote(&self, product: &str) -> Result<Quote>;
}

/// The most candles one request returns.
pub const MAX_CANDLES: i64 = 350;

/// The person's own account: needs a key.
pub trait Account: Send + Sync {
    fn permissions(&self) -> Result<Permissions>;
    fn balances(&self) -> Result<Vec<Balance>>;
    fn fees(&self) -> Result<Fees>;
    fn preview(&self, order: &OrderRequest) -> Result<Preview>;
    /// Places the order. A repeated `client_order_id` returns the order already placed, or
    /// [`ExchangeError::Duplicate`] when the exchange only says it has one.
    fn place(&self, order: &OrderRequest, preview_id: Option<&str>) -> Result<ExchangeOrder>;
    fn order(&self, order_id: &str) -> Result<ExchangeOrder>;
    /// The order placed with `client_order_id`, among `product`'s orders created in `[from, to)`
    /// (Unix seconds), or `None` when the exchange has none. Reads only: what an order sent with
    /// no answer is looked up by before it is sent again, failed or cancelled.
    fn find_order(&self, product: &str, client_order_id: &str, from: i64, to: i64) -> Result<Option<ExchangeOrder>>;
    fn fills(&self, order_id: &str) -> Result<Vec<ExchangeFill>>;
    fn cancel(&self, order_ids: &[String]) -> Result<()>;
}

/// Candles for `[start, end)`, fetched in as many requests as it takes, oldest first, with no
/// duplicates.
pub fn candles_between(m: &dyn Market, product: &str, g: Granularity, start: i64, end: i64) -> Result<Vec<Candle>> {
    let step = g.seconds() * MAX_CANDLES;
    let mut out: Vec<Candle> = Vec::new();
    let mut from = start;
    while from < end {
        let to = (from + step).min(end);
        let mut page = m.candles(product, g, from, to)?;
        page.sort_by_key(|c| c.start);
        for c in page {
            if out.last().is_none_or(|l| c.start > l.start) {
                out.push(c);
            }
        }
        from = to;
    }
    Ok(out)
}
