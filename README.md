# rust-debug
Is a library for retrieving debug information from the DWARF format.

The library provides a abstraction over the DWARF debug format, simplifies the process of retrieving information form DWARF.
The goal is to provide one function solution for complicated task like preforming a stack trace.


## Features
* One function solution for doing a stack trace
* One function solution for retrieving the call stack.
* One function solution for evaluating a variable.
* One Function solution for retrieving the source code location where a `DIE` was declared.
* One Function solution for finding a breakpoint location.


## Example
Check out this debugger for embedded Rust code I made using this library.
[https://github.com/Blinningjr/embedded-rust-debugger](https://github.com/Blinningjr/embedded-rust-debugger)


## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

