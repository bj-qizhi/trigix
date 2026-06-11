// Copyright © 2026 北京祺智科技有限公司. All rights reserved.
// https://www.qzso.com/ · managecode@gmail.com

//! The execution contracts and the workflow runtime driver now live in the
//! `execution-core` crate so the abstraction sits in a foundational crate that
//! both this executor and `platform-rs` depend on. Re-exported here so existing
//! `crate::runtime::*` / `trigix_executor::runtime::*` paths keep resolving.

pub use execution_core::*;
