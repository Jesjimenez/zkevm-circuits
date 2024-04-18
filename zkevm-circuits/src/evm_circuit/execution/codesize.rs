use bus_mapping::evm::OpcodeId;
use eth_types::Field;
use halo2_proofs::{circuit::Value, plonk::Error};

use crate::{
    evm_circuit::{
        step::ExecutionState,
        util::{
            common_gadget::SameContextGadget,
            constraint_builder::{
                ConstrainBuilderCommon, EVMConstraintBuilder, StepStateTransition, Transition,
            },
            CachedRegion, Cell, U64Cell,
        },
        witness::{Block, Call, Chunk, ExecStep, Transaction},
    },
    util::{word::WordExpr, Expr},
};

use super::ExecutionGadget;

#[derive(Clone, Debug)]
pub(crate) struct CodesizeGadget<F> {
    same_context: SameContextGadget<F>,
    codesize_bytes: U64Cell<F>,
    codesize: Cell<F>,
}

impl<F: Field> ExecutionGadget<F> for CodesizeGadget<F> {
    const NAME: &'static str = "CODESIZE";

    const EXECUTION_STATE: ExecutionState = ExecutionState::CODESIZE;

    fn configure(cb: &mut EVMConstraintBuilder<F>) -> Self {
        let opcode = cb.query_cell();

        let codesize_bytes = cb.query_u64();

        let code_hash = cb.curr.state.code_hash.clone();
        let codesize = cb.query_cell();
        cb.bytecode_length(code_hash.to_word(), codesize.expr());

        cb.require_equal(
            "Constraint: bytecode length lookup == codesize",
            codesize_bytes.expr(),
            codesize.expr(),
        );

        cb.stack_push(codesize_bytes.to_word());

        let step_state_transition = StepStateTransition {
            gas_left: Transition::Delta(-OpcodeId::CODESIZE.constant_gas_cost().expr()),
            rw_counter: Transition::Delta(1.expr()),
            program_counter: Transition::Delta(1.expr()),
            stack_pointer: Transition::Delta((-1).expr()),
            ..Default::default()
        };
        let same_context = SameContextGadget::construct(cb, opcode, step_state_transition);

        Self {
            same_context,
            codesize_bytes,
            codesize,
        }
    }

    fn assign_exec_step(
        &self,
        region: &mut CachedRegion<'_, '_, F>,
        offset: usize,
        block: &Block<F>,
        _chunk: &Chunk<F>,
        _transaction: &Transaction,
        _call: &Call,
        step: &ExecStep,
    ) -> Result<(), Error> {
        self.same_context.assign_exec_step(region, offset, step)?;

        let codesize = block.get_rws(step, 0).stack_value().as_u64();

        self.codesize_bytes
            .assign(region, offset, Some(codesize.to_le_bytes()))?;

        self.codesize
            .assign(region, offset, Value::known(F::from(codesize)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util::CircuitTestBuilder;
    use eth_types::{bytecode, Word};
    use mock::TestContext;

    fn test_ok(large: bool) {
        let mut code = bytecode! {};
        if large {
            for _ in 0..128 {
                code.push(1, Word::from(0));
            }
        }
        let tail = bytecode! {
            CODESIZE
            STOP
        };
        code.append(&tail);

        CircuitTestBuilder::new_from_test_ctx(
            TestContext::<2, 1>::simple_ctx_with_bytecode(code).unwrap(),
        )
        .run();
    }

    #[test]
    fn test_codesize_gadget() {
        test_ok(false);
    }

    #[test]
    fn test_codesize_gadget_large() {
        test_ok(true);
    }
}
