macro_rules! unwrap_or_return {
    ($res:expr) => {
        match $res {
            Ok(val) => val,
            Err(_) => {
                return;
            }
        }
    };
}

pub(crate) use unwrap_or_return;
