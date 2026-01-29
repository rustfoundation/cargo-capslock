use std::{fmt::Debug, path::PathBuf};

use inkwell::{
    context::Context,
    llvm_sys::core::{LLVMGetCalledValue, LLVMIsAConstant},
    module::Module,
    values::{AsValueRef, GlobalValue, InstructionOpcode},
};
use petgraph::graphmap::DiGraphMap;
use thiserror::Error;

use crate::{
    caps::FunctionCaps,
    function::{FunctionMap, ToFunction},
    location::IntoOptionLocation,
    r#static::bitcode::Bitcode,
};

pub fn from_bc_path(
    path: impl Into<PathBuf> + Debug,
    function_caps: &FunctionCaps,
) -> Result<Bitcode, Error> {
    let path = path.into();

    let context = Context::create();
    let module =
        Module::parse_bitcode_from_path(&path, &context).map_err(|e| Error::BitcodeParse {
            e: e.to_string(),
            path: path.clone(),
        })?;

    let mut map = FunctionMap::default();
    for function in module.get_functions() {
        map.upsert_with_caps(function_caps, function)?;
    }

    let mut graph = DiGraphMap::with_capacity(map.len(), 0);
    for function in module.get_functions() {
        let caller = map.get_index(function.mangled_name()).unwrap();

        for block in function.get_basic_block_iter() {
            for instr in block.get_instructions() {
                match instr.get_opcode() {
                    InstructionOpcode::Call | InstructionOpcode::Invoke => {
                        let cv = unsafe { LLVMGetCalledValue(instr.as_value_ref()) };
                        assert!(!cv.is_null());

                        // We'll ignore the other cases for now. They are:
                        //
                        // 1. Inline asm.
                        // 2. Metadata operand.
                        // 3. Local pointer.
                        //
                        // See CallInfo::from_llvm_ref() in llvm-ir for the gory details.
                        let con = unsafe { LLVMIsAConstant(cv) };
                        if !con.is_null() {
                            let callee_value = unsafe { GlobalValue::new(con) };
                            let callee_name = callee_value.get_name().to_str().unwrap();
                            let callee = map.get_index(callee_name).unwrap();

                            let loc = instr.into_option_location();

                            graph.add_edge(caller, callee, loc);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(Bitcode {
        path,
        functions: map,
        call_graph: graph.into(),
    })
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("parsing bitcode from {path:?}: {e}")]
    BitcodeParse { e: String, path: PathBuf },

    #[error(transparent)]
    Function(#[from] crate::function::Error),
}
