//! Yaoi Macros

/// Helper macro to execute a system call
macro_rules! syscall {
    ($fn: ident ( $($arg: expr),* $(,)* ) ) => {{
        #[allow(unused_unsafe)]
        let res = unsafe { libc::$fn($($arg, )*) };
        if res == -1 {
            Err(YaoiError::StdIo(std::io::Error::last_os_error()))
        } else {
            Ok(res)
        }
    }};
}
