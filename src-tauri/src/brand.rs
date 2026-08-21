//! Product display name injected at build time from src/brand.json.

pub const PRODUCT_NAME: &str = env!("APP_PRODUCT_NAME");
