use alloc::vec;
use core::ops::Range;

use super::*;
use crate::{
    interpreter::InterpreterParams,
    prelude::*,
};
use fuel_asm::op;
use fuel_tx::ConsensusParameters;
use test_case::test_case;

#[cfg(feature = "random")]
#[test]
fn memcopy() {
    let tx_params = TxParameters::default().with_max_gas_per_tx(Word::MAX / 2);

    let consensus_params = ConsensusParameters {
        tx_params,
        ..Default::default()
    };

    let mut vm = Interpreter::with_storage(
        MemoryStorage::default(),
        InterpreterParams::from(&consensus_params),
    );
    let tx = TransactionBuilder::script(op::ret(0x10).to_bytes().to_vec(), vec![])
        .gas_limit(tx_params.max_gas_per_tx)
        .add_random_fee_input()
        .finalize();

    let tx = tx
        .into_checked(Default::default(), &consensus_params)
        .expect("default tx should produce a valid checked transaction");

    vm.init_script(tx).expect("Failed to init VM");

    let alloc = 1024;

    // r[0x10] := 1024
    vm.instruction(op::addi(0x10, RegId::ZERO, alloc)).unwrap();
    vm.instruction(op::aloc(0x10)).unwrap();

    // r[0x20] := 128
    vm.instruction(op::addi(0x20, RegId::ZERO, 128)).unwrap();

    for i in 0..alloc {
        vm.instruction(op::addi(0x21, RegId::ZERO, i)).unwrap();
        vm.instruction(op::sb(RegId::HP, 0x21, i as Immediate12))
            .unwrap();
    }

    // r[0x23] := m[$hp, 0x20] == m[$zero, 0x20]
    vm.instruction(op::meq(0x23, RegId::HP, RegId::ZERO, 0x20))
        .unwrap();

    assert_eq!(0, vm.registers()[0x23]);

    // r[0x12] := $hp + r[0x20]
    vm.instruction(op::add(0x12, RegId::HP, 0x20)).unwrap();

    // Test ownership
    vm.instruction(op::mcp(RegId::HP, 0x12, 0x20)).unwrap();

    // r[0x23] := m[0x30, 0x20] == m[0x12, 0x20]
    vm.instruction(op::meq(0x23, RegId::HP, 0x12, 0x20))
        .unwrap();

    assert_eq!(1, vm.registers()[0x23]);

    // Assert ownership
    vm.instruction(op::subi(0x24, RegId::HP, 1)).unwrap(); // TODO: look into this
    let ownership_violated = vm.instruction(op::mcp(0x24, 0x12, 0x20));

    assert!(ownership_violated.is_err());

    // Assert no panic on overlapping
    vm.instruction(op::subi(0x25, 0x12, 1)).unwrap();
    let overlapping = vm.instruction(op::mcp(RegId::HP, 0x25, 0x20));

    assert!(overlapping.is_err());
}

#[test]
fn memrange() {
    let tx = TransactionBuilder::script(vec![], vec![])
        .gas_limit(1000000)
        .add_random_fee_input()
        .finalize()
        .into_checked(Default::default(), &ConsensusParameters::standard())
        .expect("Empty script should be valid");
    let mut vm = Interpreter::with_memory_storage();
    vm.init_script(tx).expect("Failed to init VM");

    let bytes = 1024;
    vm.instruction(op::addi(0x10, RegId::ZERO, bytes as Immediate12))
        .unwrap();
    vm.instruction(op::aloc(0x10)).unwrap();

    let m = MemoryRange::new(vm.registers()[RegId::HP] - 1, bytes).unwrap();
    assert!(!vm.ownership_registers().has_ownership_range(&m));

    let m = MemoryRange::new(vm.registers()[RegId::HP], bytes).unwrap();
    assert!(vm.ownership_registers().has_ownership_range(&m));

    MemoryRange::new(vm.registers()[RegId::HP], bytes + 1).expect_err("Overflows");

    let m = MemoryRange::new(0, bytes).unwrap().to_heap(&vm);
    assert!(vm.ownership_registers().has_ownership_range(&m));
}

#[test]
fn stack_alloc_ownership() {
    let mut vm = Interpreter::with_memory_storage();

    let tx = TransactionBuilder::script(vec![], vec![])
        .gas_limit(1000000)
        .add_random_fee_input()
        .finalize()
        .into_checked(Default::default(), &ConsensusParameters::standard())
        .expect("Empty script should be valid");
    vm.init_script(tx).expect("Failed to init VM");

    vm.instruction(op::move_(0x10, RegId::SP)).unwrap();
    vm.instruction(op::cfei(2)).unwrap();

    // Assert allocated stack is writable
    vm.instruction(op::mcli(0x10, 2)).unwrap();
}

#[test_case(
    OwnershipRegisters::test(0..0, 0..0, Context::Call{ block_height: Default::default()}), 0..0
    => true ; "empty mem range"
)]
#[test_case(
    OwnershipRegisters::test(0..0, 0..0, Context::Script{ block_height: Default::default()}), 0..0
    => true ; "empty mem range (external)"
)]
#[test_case(
    OwnershipRegisters::test(0..0, 0..0, Context::Call{ block_height: Default::default()}), 0..1
    => false ; "empty stack and heap"
)]
#[test_case(
    OwnershipRegisters::test(0..0, VM_MAX_RAM..VM_MAX_RAM, Context::Script{ block_height: Default::default() }), 0..1
    => false ; "empty stack and heap (external)"
)]
#[test_case(
    OwnershipRegisters::test(0..1, 0..0, Context::Call{ block_height: Default::default()}), 0..1
    => true ; "in range for stack"
)]
#[test_case(
    OwnershipRegisters::test(0..1, 0..0, Context::Call{ block_height: Default::default()}), 0..2
    => false; "above stack range"
)]
#[test_case(
    OwnershipRegisters::test(0..0, 0..2, Context::Call{ block_height: Default::default()}), 1..2
    => true ; "in range for heap"
)]
#[test_case(
    OwnershipRegisters::test(0..2, 1..2, Context::Call{ block_height: Default::default()}), 0..2
    => true ; "crosses stack and heap"
)]
#[test_case(
    OwnershipRegisters::test(0..0, 0..0, Context::Script{ block_height: Default::default()}), 1..2
    => true ; "in heap range (external)"
)]
#[test_case(
    OwnershipRegisters::test(0..19, 31..100, Context::Script{ block_height: Default::default()}), 20..30
    => false; "between ranges (external)"
)]
#[test_case(
    OwnershipRegisters::test(0..19, 31..100, Context::Script{ block_height: Default::default() }), 0..1
    => true; "in stack range (external)"
)]
#[test_case(
    OwnershipRegisters::test(0..0, 9..10, Context::Script { block_height: 10.into() }), 1..9
    => false; "not owned in Script context"
)]
#[test_case(
    OwnershipRegisters::test(0..0, 9..10, Context::Call { block_height: 15.into() }), 1..9
    => false; "not owned in Call context"
)]
#[test_case(
    OwnershipRegisters::test(1_000_000..1_100_000, 5_900_000..6_300_000, Context::Script { block_height: 0.into() }),
    999_000..7_100_200 => false; "crosses heap and stack range"
)]
#[test_case(
    OwnershipRegisters::test(0..20, 40..50, Context::Script { block_height: 0.into() }),
    0..20 => true; "start inclusive and end exclusive"
)]
#[test_case(
    OwnershipRegisters::test(0..20, 40..50, Context::Script { block_height: 0.into() }),
    20..41 => false; "start exclusive and end inclusive"
)]
fn test_ownership(reg: OwnershipRegisters, range: Range<u64>) -> bool {
    let range =
        MemoryRange::new(range.start, range.end - range.start).expect("Invalid range");
    reg.verify_ownership(&range).is_ok()
}

fn set_index(index: usize, val: u8, mut array: [u8; 100]) -> [u8; 100] {
    array[index] = val;
    array
}

#[test_case(
    1, &[],
    OwnershipRegisters::test(0..1, 100..100, Context::Script{ block_height: Default::default()})
    => (false, [0u8; 100]); "External errors when write is empty"
)]
#[test_case(
    1, &[],
    OwnershipRegisters::test(0..1, 100..100, Context::Call{ block_height: Default::default()})
    => (false, [0u8; 100]); "Internal errors when write is empty"
)]
#[test_case(
    1, &[2],
    OwnershipRegisters::test(0..2, 100..100, Context::Script{ block_height: Default::default()})
    => (true, set_index(1, 2, [0u8; 100])); "External writes to stack"
)]
#[test_case(
    98, &[2],
    OwnershipRegisters::test(0..2, 97..100, Context::Script{ block_height: Default::default()})
    => (true, set_index(98, 2, [0u8; 100])); "External writes to heap"
)]
#[test_case(
    1, &[2],
    OwnershipRegisters::test(0..2, 100..100, Context::Call { block_height: Default::default()})
    => (true, set_index(1, 2, [0u8; 100])); "Internal writes to stack"
)]
#[test_case(
    98, &[2],
    OwnershipRegisters::test(0..2, 97..100, Context::Call { block_height: Default::default()})
    => (true, set_index(98, 2, [0u8; 100])); "Internal writes to heap"
)]
#[test_case(
    1, &[2; 50],
    OwnershipRegisters::test(0..40, 100..100, Context::Script{ block_height: Default::default()})
    => (false, [0u8; 100]); "External too large for stack"
)]
#[test_case(
    1, &[2; 50],
    OwnershipRegisters::test(0..40, 100..100, Context::Call{ block_height: Default::default()})
    => (false, [0u8; 100]); "Internal too large for stack"
)]
#[test_case(
    61, &[2; 50],
    OwnershipRegisters::test(0..0, 60..100, Context::Call{ block_height: Default::default()})
    => (false, [0u8; 100]); "Internal too large for heap"
)]
fn test_mem_write(
    addr: usize,
    data: &[u8],
    registers: OwnershipRegisters,
) -> (bool, [u8; 100]) {
    let mut memory: Memory<MEM_SIZE> = vec![0u8; MEM_SIZE].try_into().unwrap();
    let r = try_mem_write(addr, data, registers, &mut memory).is_ok();
    let memory: [u8; 100] = memory[..100].try_into().unwrap();
    (r, memory)
}

#[test_case(
    1, 0,
    OwnershipRegisters::test(0..1, 100..100, Context::Script{ block_height: Default::default()})
    => (false, [1u8; 100]); "External errors when write is empty"
)]
#[test_case(
    1, 0,
    OwnershipRegisters::test(0..1, 100..100, Context::Call{ block_height: Default::default()})
    => (false, [1u8; 100]); "Internal errors when write is empty"
)]
#[test_case(
    1, 1,
    OwnershipRegisters::test(0..2, 100..100, Context::Script{ block_height: Default::default()})
    => (true, set_index(1, 0, [1u8; 100])); "External writes to stack"
)]
#[test_case(
    98, 1,
    OwnershipRegisters::test(0..2, 97..100, Context::Script{ block_height: Default::default()})
    => (true, set_index(98, 0, [1u8; 100])); "External writes to heap"
)]
#[test_case(
    1, 1,
    OwnershipRegisters::test(0..2, 100..100, Context::Call { block_height: Default::default()})
    => (true, set_index(1, 0, [1u8; 100])); "Internal writes to stack"
)]
#[test_case(
    98, 1,
    OwnershipRegisters::test(0..2, 97..100, Context::Call { block_height: Default::default()})
    => (true, set_index(98, 0, [1u8; 100])); "Internal writes to heap"
)]
#[test_case(
    1, 50,
    OwnershipRegisters::test(0..40, 100..100, Context::Script{ block_height: Default::default()})
    => (false, [1u8; 100]); "External too large for stack"
)]
#[test_case(
    1, 50,
    OwnershipRegisters::test(0..40, 100..100, Context::Call{ block_height: Default::default()})
    => (false, [1u8; 100]); "Internal too large for stack"
)]
#[test_case(
    61, 50,
    OwnershipRegisters::test(0..0, 60..100, Context::Call{ block_height: Default::default()})
    => (false, [1u8; 100]); "Internal too large for heap"
)]
fn test_try_zeroize(
    addr: usize,
    len: usize,
    registers: OwnershipRegisters,
) -> (bool, [u8; 100]) {
    let mut memory: Memory<MEM_SIZE> = vec![1u8; MEM_SIZE].try_into().unwrap();
    let r = try_zeroize(addr, len, registers, &mut memory).is_ok();
    let memory: [u8; 100] = memory[..100].try_into().unwrap();
    (r, memory)
}

// Zero-sized write
#[test_case(0, 0, 0, &[1, 2, 3, 4] => (true, [0xff, 0xff, 0xff, 0xff, 0xff]))]
#[test_case(1, 0, 0, &[1, 2, 3, 4] => (true, [0xff, 0xff, 0xff, 0xff, 0xff]))]
#[test_case(0, 0, 1, &[1, 2, 3, 4] => (true, [0xff, 0xff, 0xff, 0xff, 0xff]))]
// Dst address checks
#[test_case(0, 4, 0, &[1, 2, 3, 4] => (true, [1, 2, 3, 4, 0xff]))]
#[test_case(1, 4, 0, &[1, 2, 3, 4] => (true, [0xff, 1, 2, 3, 4]))]
#[test_case(2, 4, 0, &[1, 2, 3, 4] => (true, [0xff, 0xff, 1, 2, 3]))]
#[test_case(2, 2, 0, &[1, 2, 3, 4] => (true, [0xff, 0xff, 1, 2, 0xff]))]
// Zero padding when exceeding slice size
#[test_case(0, 2, 2, &[1, 2, 3, 4] => (true, [3, 4, 0xff, 0xff, 0xff]))]
#[test_case(0, 2, 3, &[1, 2, 3, 4] => (true, [4, 0, 0xff, 0xff, 0xff]))]
#[test_case(0, 2, 4, &[1, 2, 3, 4] => (true, [0, 0, 0xff, 0xff, 0xff]))]
#[test_case(0, 2, 5, &[1, 2, 3, 4] => (true, [0, 0, 0xff, 0xff, 0xff]))]
// Zero padding when exceeding slice size, but with nonzero dst address
#[test_case(1, 2, 2, &[1, 2, 3, 4] => (true, [0xff, 3, 4, 0xff, 0xff]))]
#[test_case(1, 2, 3, &[1, 2, 3, 4] => (true, [0xff, 4, 0, 0xff, 0xff]))]
#[test_case(1, 2, 4, &[1, 2, 3, 4] => (true, [0xff, 0, 0, 0xff, 0xff]))]
#[test_case(1, 2, 5, &[1, 2, 3, 4] => (true, [0xff, 0, 0, 0xff, 0xff]))]
// Zero-sized src slice
#[test_case(1, 0, 0, &[] => (true, [0xff, 0xff, 0xff, 0xff, 0xff]))]
#[test_case(1, 2, 0, &[] => (true, [0xff, 0, 0, 0xff, 0xff]))]
#[test_case(1, 2, 1, &[] => (true, [0xff, 0, 0, 0xff, 0xff]))]
#[test_case(1, 2, 2, &[] => (true, [0xff, 0, 0, 0xff, 0xff]))]
#[test_case(1, 2, 3, &[] => (true, [0xff, 0, 0, 0xff, 0xff]))]
fn test_copy_from_slice_zero_fill_noownerchecks(
    addr: usize,
    len: usize,
    src_offset: usize,
    src_data: &[u8],
) -> (bool, [u8; 5]) {
    let mut memory: Memory<MEM_SIZE> = vec![0xffu8; MEM_SIZE].try_into().unwrap();
    let r = copy_from_slice_zero_fill_noownerchecks(
        &mut memory,
        src_data,
        addr,
        src_offset,
        len,
    )
    .is_ok();
    let memory: [u8; 5] = memory[..5].try_into().unwrap();
    (r, memory)
}
