# Monday 2021/02/08
* Did nothing


# Tuesday
* Implemented `RequiresParameterRef` enum case in the evaluation function and case `RequiresIndexedAddress`.

* Restructured the code by splitting it up into different files and made a debugger struct. The struct reduces the number of parameters needed.

* Improved on the `eval_piece` function so that it will mask the relevant bits.


# Wednesday
* Added my own value type enum which enables me to return different types of data that is needed.

* Implemented so that more then 32bits can now be read from a address.

* Implemented so that the byte size of the object is sent to `eval_piece` and thus enabling the tool to read the raw value of  structs and enums.


# Thursday
* Implemented a function that searches for a variable and evaluates it.

* Implemented a function that finds all the type die using the type name.

* Had a meeting with Per and the other.


# Friday
* Restructured the code so it is easier to work with.

* Implemented so that the given type offset is used to find the tree node that has all the type info.

* Implemented a set of parsers that parse the type information into a type struct, that is easier to use.

* Added a trait for the `DebuggerType` that allows for easy access to the bytes size of the type.

* Changed so that the `DebuggerType` is used instead of a reference to the die with the type information.


# Saturday
* Implemented a set of parsers that takes the raw data read from some address plus the parsed type information and transforms it into a value struct.


# Sunday
* Make the print function not evaluate the expressions.

* Fixed some of the warnings.

