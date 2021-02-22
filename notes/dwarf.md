# DWARF Format Notes
Source: http://dwarfstd.org/doc/DWARF5.pdf

## DWARF Sections

### DWARF .debug\_info section
This section contains the debug information, the information is stored in units and these units contain a tree of DIE:s. 

## DWARF Unit
The information stored in the section .debug\_info is divided into compilation units, also known as units. These units are the different files the compiler has compiled for this program. These units contains a lot for DIE:s that are stored in a tree structure where the root DIE has the tag DW\_TAG\_compile\_unit. They also contain a range of program counter start and end values, which can be used to determine which unit the program is currently running.

## DWARF DIE
The debugging information is stored in DIE:s, DIE stands for Debugging Information Entry. DIE:s has a tag which tells the debugger what information it contains, it also contains key value pairs. These key value pairs are stored as attributes of the die and contain all the information about that entry. Some of the DIE:s have the attributes DW\_AT\_range, DW\_at\_high\_pc and DW\_AT\_low\_pc. These attributes can be used to determine if the DIE is currently running on the code by comparing there values to the current program counter(pc)

## Notations
* DIE - Debugging Information Entry
* TLS - thread-local storage
* CFA -	Canonical Frame Address - An area of memory that is allocated on a stack called a “call frame.” The call frame is identified by an address on the stack. We refer to this address as the Canonical Frame Address or CFA. Typically, the CFA is defined to be the value of the stack pointer at the call site in the previous frame (which may be different from its value on entry to the current frame).

# Optimization problems
* Can use `#[inline(never)]` to ensure that the location information is there, but removes some of the optimization.

