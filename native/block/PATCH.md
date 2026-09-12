# block 0.1.6 compatibility patch

Source: the MIT-licensed `block` 0.1.6 crate by Steven Sheldon,
<https://github.com/SSheldon/rust-block>. The published README and package
attribution are retained. This local copy must not be published.

`_NSConcreteStackBlock` was declared with an uninhabited empty-enum type,
which triggers Rust's `uninhabited_static` future-incompatibility lint.
Represent the class as an opaque inhabited C struct and take only its raw
address. All formerly implicit C ABI declarations now explicitly use `extern "C"`.
Block layout, copying and release semantics are unchanged; see the
[Clang Block ABI](https://clang.llvm.org/docs/Block-ABI-Apple.html).

This dependency is still used transitively by Cocoa, Metal and GPUI. A local
Cargo patch fixes all callers without changing their dependency versions or
editing the registry cache. Remove it once the dependency graph no longer
uses block 0.1.

The original six unit tests are retained. Their helper calls cross the block2
ABI boundary in place of the unpublished upstream `test_utils` path dependency.
Run `cargo test --manifest-path native/block/Cargo.toml --target-dir target/block-tests --locked` on macOS to check calls, heap copying and
callbacks that outlive their original stack frame.
