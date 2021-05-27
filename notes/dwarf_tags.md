# DWARF format version 4 Tags

## DW\_TAG\_access\_declaration <!--- NOTE: Not applicable to my debugger -->
Doesn't seem to be used by Rust.

This is partly how `DW_TAG_access_declaration` is described in DWARF 4, page 87:

> "If a derived class or structure contains access declarations, each such declaration may be represented by a debugging information entry with the tag `DW_TAG_access_declaration`.
> Each such entry is a child of the class or structure type entry."


## DW\_TAG\_array\_type <!--- NOTE: Used in my debugger -->
The tag `DW_TAG_array_type` describes an array composed of one data type, which is described by the die referenced in the attribute `DW_AT_type`.
The length of the array is found in a child die with the tag `DW_TAG_subrange_type` and can be used in combination of the size of the type to read the data in the array from the memory.

This is partly how `DW_TAG_array_type` is described in DWARF 4, page 83:

> "An array type is represented by a debugging information entry with the tag `DW_TAG_array_type`.
> If a name has been given to the array type in the source program, then the corresponding array type entry has a `DW_AT_name` attribute whose value is a null-terminated string containing the array type name as it appears in the source program."


## DW\_TAG\_base\_type <!--- NOTE: Used in my debugger -->
This tag is one of the most common tags that describes the type of a value.
The attribute `DW_AT_encoding` is always present in the base type dies because it is used in combination with the attribute `DW_AT_byte_size` to decode the binary value.
Then there is also the attribute `DW_AT_name` which holds the name of the base type.

This is partly how `DW_TAG_base_type` is described in DWARF 4, page 75:

> "A base type is represented by a debugging information entry with the tag `DW_TAG_base_type`.
> <br></br>
> A base type entry has a `DW_AT_name` attribute whose value is a null-terminated string containing the name of the base type as recognized by the programming language of the compilation unit containing the base type entry."


## DW\_TAG\_catch\_block <!--- NOTE: Not applicable to my debugger -->
Doesn't seem to be used by Rust.

This is partly how `DW_TAG_catch_block` is described in DWARF 4, page 66:

> "A try block is represented by a debugging information entry with the tag `DW_TAG_try_block`.
> A catch block is represented by a debugging information entry with the tag `DW_TAG_catch_block`."


## DW\_TAG\_class\_type <!--- NOTE: Not applicable to my debugger -->
Doesn't seem to be used by Rust.

This is partly how `DW_TAG_class_type` is described in DWARF 4, page 84:

> "Structure, union, and class types are represented by debugging information entries with the tags `DW_TAG_structure_type`, `DW_TAG_union_type`, and `DW_TAG_class_type`, respectively.
> If a name has been given to the structure, union, or class in the source program, then the corresponding structure type, union type, or class type entry has a `DW_AT_name` attribute whose value is a null-terminated string containing the type name as it appears in the source program."


## DW\_TAG\_common\_block <!--- TODO: Confirm, seems like a Fortran thing-->
Doesn't seem to be used by Rust.

This is partly how `DW_TAG_common_block` is described in DWARF 4, page 73:

> "A Fortran common block may be described by a debugging information entry with the tag `DW_TAG_common_block`.
> The common block entry has a `DW_AT_name` attribute whose value is a null-terminated string containing the common block name as it appears in the source program.
> It may also have a `DW_AT_linkage_name` attribute as described in Section 2.22. It also has a `DW_AT_location` attribute whose value describes the location of the beginning of the common block.
> The common block entry owns debugging information entries describing the variables contained within the common block."


## DW\_TAG\_common\_inclusion <!--- TODO: Confirm, seems like a Fortran thing-->
Doesn't seem to be used by Rust.

This is partly how `DW_TAG_common_inclusion` is described in DWARF 4, page 56:

> "The entry for a subroutine that includes a Fortran common block has a child entry with the tag `DW_TAG_common_inclusion`.
> The common inclusion entry has a `DW_AT_common_reference` attribute whose value is a reference to the debugging information entry for the common block being included (see Section 4.2)."


## DW\_TAG\_compile\_unit <!--- NOTE: Used in my debugger -->
A die with the tag `DW_TAG_compile_unit` will always be the root of the unit.
It holds some information about the compilation of one of the source files.

This is partly how `DW_TAG_compile_unit` is described in DWARF 4, page :

> "A normal compilation unit is represented by a debugging information entry with the tag `DW_TAG_compile_unit`.
> A partial compilation unit is represented by a debugging information entry with the tag `DW_TAG_partial_unit`."


## DW\_TAG\_condition <!--- TODO: Confirm, seems like a COBOL thing -->
Doesn't seem to be used by Rust.

This is partly how `DW_TAG_condition` is described in DWARF 4, page 95:

> "The `DW_TAG_condition` debugging information entry describes a logical condition that tests whether a given data item’s value matches one of a set of constant values.
> If a name has been given to the condition, the condition entry has a `DW_AT_name` attribute whose value is a null-terminated string giving the condition name as it appears in the source program."


## DW\_TAG\_const\_type <!--- TODO: Confirm, seems like a C or C++ thing -->
Doesn't seem to be used by Rust.

This is partly how `DW_TAG_const_type` is described in DWARF 4, page 81:

> "C or C++ const qualified type"


## DW\_TAG\_constant <!--- TODO: Confirm -->
Doesn't seem to be used by Rust.

This is partly how `DW_TAG_constant` is described in DWARF 4, page 69:

> "Program variables, formal parameters and constants are represented by debugging information entries with the tags `DW_TAG_variable`, `DW_TAG_formal_parameter` and `DW_TAG_constant`, respectively."


## DW\_TAG\_dwarf\_procedure <!--- TODO: Confirm -->
Doesn't seem to be used by Rust.

This is partly how `DW_TAG_dwarf_procedure` is described in DWARF 4, page 37:

> "A DWARF procedure is represented by any kind of debugging information entry that has a `DW_AT_location` attribute.
> If a suitable entry is not otherwise available, a DWARF procedure can be represented using a debugging information entry with the tag `DW_TAG_dwarf_procedure` together with a `DW_AT_location` attribute."


## DW\_TAG\_entry\_point <!--- TODO: Confirm -->
Doesn't seem to be used by Rust.

This is partly how `DW_TAG_entry_point` is described in DWARF 4, page 53:

> "An alternate entry point."


## `DW_TAG_enumeration_type`
## `DW_TAG_enumerator`
## `DW_TAG_file_type`
## `DW_TAG_formal_parameter`
## `DW_TAG_friend`
## `DW_TAG_imported_declaration`
## `DW_TAG_imported_module`
## `DW_TAG_imported_unit`
## `DW_TAG_inheritance`
## `DW_TAG_inlined_subroutine`
## `DW_TAG_interface_type`
## `DW_TAG_label`
## `DW_TAG_lexical_block`
## `DW_TAG_member`
## `DW_TAG_module`
## `DW_TAG_namelist`
## `DW_TAG_namelist_item`


## `DW_TAG_namespace`
## `DW_TAG_packed_type`
## `DW_TAG_partial_unit`
## `DW_TAG_pointer_type`
## `DW_TAG_ptr_to_member_type`
## `DW_TAG_reference_type`
## `DW_TAG_restrict_type`
## `DW_TAG_rvalue_reference_type`
## `DW_TAG_set_type`
## `DW_TAG_shared_type`
## `DW_TAG_string_type`
## `DW_TAG_structure_type`
## `DW_TAG_subprogram`
## `DW_TAG_subrange_type`
## `DW_TAG_subroutine_type`
## `DW_TAG_template_alias`
## `DW_TAG_template_type_parameter`
## `DW_TAG_template_value_parameter`
## `DW_TAG_thrown_type`
## `DW_TAG_try_block`
## `DW_TAG_typedef`
## `DW_TAG_type_unit`
## `DW_TAG_union_type`
## `DW_TAG_unspecified_paramters`
## `DW_TAG_unspecified_type`
## `DW_TAG_variable`
## `DW_TAG_variant`
## `DW_TAG_variant_part`
## `DW_TAG_volatile_type`
## `DW_TAG_with_stmt`

