use super::{
    Reader,
    Debugger,
    DebuggerValue,
};


impl<'a, R: Reader<Offset = usize>> Debugger<'a, R> {
    pub fn eval_basetype(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }

    pub fn eval_pointer_type(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }

    pub fn eval_array_type(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }

    pub fn eval_structured_type(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }

    pub fn eval_union_type(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }

    pub fn eval_member(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }

    pub fn eval_enumeration_type(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }

    pub fn eval_enumerator(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }

    pub fn eval_string_type(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }

    pub fn eval_subrange_type(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }

    pub fn eval_generic_subrange_type(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }

    pub fn eval_template_type_parameter(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }

    pub fn eval_variant_part(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }

    pub fn eval_variant(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }

    pub fn eval_subroutune_type(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }

    pub fn eval_subprogram(&mut self) -> DebuggerValue<R>
    {
        unimplemented!();
    }
}

