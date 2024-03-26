mod error;
pub use crate::error::{CatBoostError, CatBoostResult};

mod model;
pub use crate::model::Model;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_model() {
    }
}
