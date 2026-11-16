//! Controls that know nothing about notes. A component that needs `ipc`
//! belongs in `windows`, not here.

mod kbd;

pub use kbd::Kbd;
