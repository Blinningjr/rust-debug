# DWARF format version 4 Attributes

## `DW_AT_abstract_origin` TODO
## `DW_AT_accessibility` TODO
## `DW_AT_address_class` TODO
## `DW_AT_allocated` TODO
## `DW_AT_artificial` TODO
## `DW_AT_associated` TODO
## `DW_AT_base_types` TODO
## `DW_AT_binary_scale` TODO
## `DW_AT_bit_offset` TODO
## `DW_AT_bit_size` TODO
## `DW_AT_bit_stride` TODO
## `DW_AT_byte_size` TODO
## `DW_AT_byte_stride` TODO
## `DW_AT_call_column` TODO
## `DW_AT_call_file` TODO


## `DW_AT_call_line` TODO
## `DW_AT_calling_convention` TODO
## `DW_AT_common_reference` TODO
## `DW_AT_comp_dir` TODO
## `DW_AT_const_value` TODO
## `DW_AT_const_expr` TODO
## `DW_AT_containing_type` TODO
## `DW_AT_count` TODO
## `DW_AT_data_bit_offset` TODO
## `DW_AT_data_location` TODO
## `DW_AT_data_member_location` TODO
## `DW_AT_decimal_scale` TODO
## `DW_AT_decimal_sign` TODO
## `DW_AT_decl_column` TODO
## `DW_AT_decl_file` TODO
## `DW_AT_decl_line` TODO
## `DW_AT_declaration` TODO
## `DW_AT_default_value` TODO
## `DW_AT_description` TODO


## `DW_AT_digit_count` TODO
## `DW_AT_discr` TODO
## `DW_AT_discr_list` TODO
## `DW_AT_discr_value` TODO
## `DW_AT_elemental` TODO
## `DW_AT_encoding` TODO
## `DW_AT_endianity` TODO
## `DW_AT_entry_pc` TODO
## `DW_AT_enum_class` TODO
## `DW_AT_explicit` TODO
## `DW_AT_extension` TODO


## `DW_AT_external` <!--- TODO: Use this to show visibility of functions and variables -->
This attributes is to show if a function or variable is visible outside of its compilation unit.
It can be found in dies with the tags `DW_TAG_subprogram` and `DW_TAG_variable`.

This is how it is described for Subroutines in DWARF 4, page 53:

"If the name of the subroutine described by an entry with the tag `DW_TAG_subprogram` is visible
outside of its containing compilation unit, that entry has a `DW_AT_external` attribute, which is a
flag."

This is how it is described for Variables in DWARF 4, page 70:

"If the variable entry represents a non-defining declaration, `DW_AT_specification` may be
used to reference the defining declaration of the variable. If no `DW_AT_specification`
attribute is present, the defining declaration may be found as a global definition either in the
current compilation unit or in another compilation unit with the `DW_AT_external` attribute."


## `DW_AT_frame_base` TODO
## `DW_AT_friend` TODO
## `DW_AT_high_pc` TODO
## `DW_AT_identifier_case` TODO
## `DW_AT_import` TODO


## `DW_AT_inline` <!--- TODO: Used this to show what functions got inlined -->
This attribute can be used in to ways, the first is to declare an instance abstract, the other way so to describe if a function got inlined or not. 

This is partly how Abstract Instances is described in DWARF 4, page 59:

"Any debugging information entry that is owned (either directly or indirectly) by a debugging
information entry that contains the `DW_AT_inline` attribute is referred to as an “abstract instance
entry.” Any subroutine entry that contains a `DW_AT_inline` attribute whose value is other than
`DW_INL_not_inlined` is known as an “abstract instance root.”"

This is how  Inlined Subroutines is described in DWARF 4, page 58:

"A declaration or a definition of an inlinable subroutine is represented by a debugging information
entry with the tag `DW_TAG_subprogram`. The entry for a subroutine that is explicitly declared
to be available for inline expansion or that was expanded inline implicitly by the compiler has a
`DW_AT_inline` attribute whose value is an integer constant. The set of values for the
`DW_AT_inline` attribute is given in Figure 11."


## `DW_AT_is_optional` TODO
## `DW_AT_language` TODO
## `DW_AT_linkage_name` TODO
## `DW_AT_location` TODO
## `DW_AT_low_pc` TODO
## `DW_AT_lower_bound` TODO
## `DW_AT_macro_info` TODO
## `DW_AT_main_subprogram` TODO
## `DW_AT_mutable` TODO
## `DW_AT_name` TODO
## `DW_AT_namelist_item` TODO
## `DW_AT_object_pointer` TODO
## `DW_AT_ordering` TODO
## `DW_AT_picture_string` TODO
## `DW_AT_priority` TODO


## `DW_AT_producer` <!--- TODO: Use this attribute in my debugger somehow. -->
Describes which compiler that generated this DWARF compilation unit and which version of the compiler that was used.
This attributes is only found in the dies with the tag `DW_TAG_compile_unit`.

This is how it is described in DWARF 4, page 46:

"A `DW_AT_producer` attribute whose value is a null-terminated string containing information
about the compiler that produced the compilation unit. The actual contents of the string will
be specific to each producer, but should begin with the name of the compiler vendor or some
other identifying character sequence that should avoid confusion with other producer values."


## `DW_AT_prototyped` <!--- NOTE: Not applicable to my debugger -->
This seams to be a thing in `C` functions and thus is not used in rust.

This is how it is described in DWARF 4, page 54:

"A subroutine entry declared with a function prototype style declaration may have a
`DW_AT_prototyped` attribute, which is a flag."

This is how it is described in DWARF 4, page 97:

"A subroutine entry declared with a function prototype style declaration may have a
`DW_AT_prototyped` attribute, which is a flag."


## `DW_AT_pure` TODO
## `DW_AT_ranges` TODO


## `DW_AT_recursive` TODO
## `DW_AT_return_addr` TODO
## `DW_AT_segment` TODO
## `DW_AT_sibling` TODO
## `DW_AT_small` TODO
## `DW_AT_signature` TODO
## `DW_AT_specification` TODO
## `DW_AT_start_scope` TODO
## `DW_AT_static_link` TODO


## `DW_AT_stmt_list` <!--- NOTE: I use unit.line_program from gimli-rs instead -->
This attribute is only found in dies with the tag `DW_TAG_compile_unit` and its value is a offset to the line number information.

This is how it is described in DWARF 4, page 45:

"A `DW_AT_stmt_list` attribute whose value is a section offset to the line number information
for this compilation unit.
This information is placed in a separate object file section from the debugging information
entries themselves. The value of the statement list attribute is the offset in the .debug\_line
section of the first byte of the line number information for this compilation unit (see
Section 6.2)."


## `DW_AT_string_length` <!--- NOTE: Not applicable to my debugger -->
Doesn't seem to be used by Rust.

This is how it is described in DWARF 4, page 98:

"The string type entry may have a `DW_AT_string_length` attribute whose value is a location
description yielding the location where the length of the string is stored in the program."


## `DW_AT_threads_scaled` TODO
## `DW_AT_trampoline` TODO
## `DW_AT_type` TODO
## `DW_AT_upper_bound` TODO
## `DW_AT_use_location` TODO
## `DW_AT_use_UTF8` TODO
## `DW_AT_variable_parameter` TODO
## `DW_AT_virtuality` TODO


## `DW_AT_visibility` TODO
## `DW_AT_vtable_elem_location` TODO

