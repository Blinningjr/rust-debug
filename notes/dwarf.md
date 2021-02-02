# DWARF Format Notes
Source: http://dwarfstd.org/doc/DWARF5.pdf

## DWARF Sections

### DWARF Units
The information stored in the section .debug\_info is divided into compilation units, also known as units. These units are the different files the compiler has compiled for this program. These units contains a lot for DIE:s that are stored in a tree structure where the root die has the tag DW\_TAG\_compile\_unit. They also contain a range of program counter start and end values, which can be used to determine which unit the program is currently running.

## Notations
* DIE - Debugging Information Entry
* TLS - thread-local storage
* CFA -	Canonical Frame Address - An area of memory that is allocated on a stack called a “call frame.” The call frame is identified by an address on the stack. We refer to this address as the Canonical Frame Address or CFA. Typically, the CFA is defined to be the value of the stack pointer at the call site in the previous frame (which may be different from its value on entry to the current frame).

