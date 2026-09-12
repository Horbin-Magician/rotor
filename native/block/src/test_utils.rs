// Exercise the original block 0.1 tests across the block2 ABI boundary without
// the unpublished upstream Objective-C test-helper path dependency.
extern crate block2;

use {Block, RcBlock};

pub fn get_int_block_with(i: i32) -> RcBlock<(), i32> {
    let block = block2::RcBlock::new(move || i);
    unsafe { RcBlock::copy((&*block as *const block2::Block<dyn Fn() -> i32>).cast_mut().cast()) }
}

pub fn get_add_block_with(i: i32) -> RcBlock<(i32,), i32> {
    let block = block2::RcBlock::new(move |a: i32| a + i);
    unsafe { RcBlock::copy((&*block as *const block2::Block<dyn Fn(i32) -> i32>).cast_mut().cast()) }
}

pub fn invoke_int_block(block: &Block<(), i32>) -> i32 {
    unsafe { (&*(block as *const _ as *const block2::Block<dyn Fn() -> i32>)).call(()) }
}

pub fn invoke_add_block(block: &Block<(i32,), i32>, a: i32) -> i32 {
    unsafe { (&*(block as *const _ as *const block2::Block<dyn Fn(i32) -> i32>)).call((a,)) }
}
