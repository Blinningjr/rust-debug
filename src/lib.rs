//! Easy to use DWARF abstraction for Rust.
//!
//! Provides an abstraction over dwarf. The abstraction provides a one function solution for
//! retrieving debug information from DWARF.
//! Here are some of the  advantages:
//! - Easy to use
//!

/// Provides one function solutions for doing a stack trace
pub mod call_stack;

/// Provides one function solutions for handling evaluation the DWARF location attribute.
pub mod evaluate;

/// Defines a struct containing information about the registers
pub mod registers;

/// Provides one function solutions for retrieving the source location declaration information.
pub mod source_information;

/// Provides some useful functions for reading the DWARF format.
pub mod utils;

/// Provides one function solutions for retrieving information about a variable.
pub mod variable;
