# DWARF Format Notes
Source: http://dwarfstd.org/doc/DWARF5.pdf

## Notations
* DIE - Debugging Information Entry
* TLS - thread-local storage
* CFA -	Canonical Frame Address
	An area of memory that is allocated on a stack called a “call frame.” The call frame is identified by an address on the stack. We refer to this address as the Canonical Frame Address or CFA. Typically, the CFA is defined to be the value of the stack pointer at the call site in the previous frame (which may be different from its value11on entry to the current frame).

