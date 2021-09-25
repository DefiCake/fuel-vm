use super::Interpreter;
use crate::consts::*;
use crate::error::InterpreterError;

use fuel_data::bytes::SizedBytes;
use fuel_data::{RegisterId, Word};

impl<S> Interpreter<S> {
    pub(crate) fn transaction_input_length(&mut self, ra: RegisterId, b: Word) -> Result<(), InterpreterError> {
        self.registers[ra] = self
            .tx
            .inputs()
            .get(b as usize)
            .ok_or(InterpreterError::InputNotFound)
            .map(|input| input.serialized_size() as Word)?;

        Ok(())
    }

    pub(crate) fn transaction_input_start(&mut self, ra: RegisterId, b: Word) -> Result<(), InterpreterError> {
        self.registers[ra] = (VM_TX_MEMORY
            + self
                .tx
                .input_offset(b as usize)
                .ok_or(InterpreterError::InputNotFound)?) as Word;

        Ok(())
    }

    pub(crate) fn transaction_output_length(&mut self, ra: RegisterId, b: Word) -> Result<(), InterpreterError> {
        self.registers[ra] = self
            .tx
            .outputs()
            .get(b as usize)
            .ok_or(InterpreterError::OutputNotFound)
            .map(|output| output.serialized_size() as Word)?;

        Ok(())
    }

    pub(crate) fn transaction_output_start(&mut self, ra: RegisterId, b: Word) -> Result<(), InterpreterError> {
        self.registers[ra] = (VM_TX_MEMORY
            + self
                .tx
                .output_offset(b as usize)
                .ok_or(InterpreterError::OutputNotFound)?) as Word;

        Ok(())
    }

    pub(crate) fn transaction_witness_length(&mut self, ra: RegisterId, b: Word) -> Result<(), InterpreterError> {
        self.registers[ra] = self
            .tx
            .witnesses()
            .get(b as usize)
            .ok_or(InterpreterError::OutputNotFound)
            .map(|witness| witness.serialized_size() as Word)?;

        Ok(())
    }

    pub(crate) fn transaction_witness_start(&mut self, ra: RegisterId, b: Word) -> Result<(), InterpreterError> {
        self.registers[ra] = (VM_TX_MEMORY
            + self
                .tx
                .witness_offset(b as usize)
                .ok_or(InterpreterError::WitnessNotFound)?) as Word;

        Ok(())
    }
}
