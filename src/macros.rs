#![allow(unused_macros)]

///Unwraps a given Result<T,E>, returning the interior T or continuing.
#[macro_export]
macro_rules! skip_err {
    ($res:expr) => {
        match $res {
            Ok(val) => val,
            Err(_) => {
                #[cfg(debug_assertions)]
                log::warn!("Returning at line {}", line!());
                continue;
            }
        }
    };
}

///Unwraps a given Option<T>, returning the interior T or continuing.
#[macro_export]
macro_rules! skip_opt {
    ($res:expr) => {
        match $res {
            Some(val) => val,
            None => {
                #[cfg(debug_assertions)]
                log::warn!("Returning at line {}", line!());
                continue;
            }
        }
    };
}

///Unwraps a given Result<T,E>, returning the interior T or returning.
#[macro_export]
macro_rules! ret_err {
    ($res:expr) => {
        match $res {
            Ok(val) => val,
            Err(_) => {
                #[cfg(debug_assertions)]
                log::warn!("Returning at line {}", line!());
                return;
            }
        }
    };
}

///Unwraps a given Option<T>, returning the interior T or returning.
#[macro_export]
macro_rules! ret_opt {
    ($res:expr) => {
        match $res {
            Some(val) => val,
            None => {
                #[cfg(debug_assertions)]
                log::warn!("Returning at line {} in file {}", line!(), file!());
                return;
            }
        }
    };
}
